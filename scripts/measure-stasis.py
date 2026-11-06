#!/usr/bin/env python3
"""Compare Stasis worker counts/detectors in a private idle stack. Build first.

No compilation or other acceptance/measurement fixtures may run concurrently.
Context switches measure observed scheduler sleeps, not hardware wakeups.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import socket
import statistics
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]

def module(name,file):
    spec=importlib.util.spec_from_file_location(name,ROOT/'scripts'/file)
    result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result

desktop=module('desktop','validate-desktop.py')
sampler=module('idle_sampler','measure-idle.py')

def tree(pid):
    result=[pid]
    for child in Path(f'/proc/{pid}/task/{pid}/children').read_text().split():result.extend(tree(int(child)))
    return result

def sample(pid):
    processes=tree(pid)
    samples=[sampler.sample(p) for p in processes]
    result=dict(monotonic_seconds=time.monotonic(),start_ticks=[(p,s['start_ticks']) for p,s in zip(processes,samples)],
                cpu_ticks=sum(s['cpu_ticks'] for s in samples))
    for key in ('rss_kib','pss_kib','private_kib','threads'):result[key]=sum(s[key] for s in samples)
    for key in ('thread_voluntary_context_switches','thread_cpu'):
        result[key]={f'{p}:{tid}':value for p,s in zip(processes,samples) for tid,value in s[key].items()}
    return result

def measure(args,session):
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
    session.env['DBUS_SYSTEM_BUS_ADDRESS']=session.env['DBUS_SESSION_BUS_ADDRESS']
    session.env.pop('XDG_SESSION_ID',None)
    root=session.runtime
    session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/lock-logind.py'),str(root)],'logind.log')
    session.wait_for(lambda:(root/'logind-ready').exists(),'private logind')
    pam=root/'pam';pam.mkdir()
    (pam/'canopa-lock').write_text('auth required pam_deny.so\naccount required pam_deny.so\n')
    prefix=['bwrap','--ro-bind','/','/','--dev','/dev','--bind',str(root),str(root),'--ro-bind',str(pam),'/etc/pam.d','--die-with-parent']
    listener=session.start([*prefix,str(args.bin_dir/'canopa-lock'),'--defaults','daemon'],'listener.log')
    def ready():
        try:return json.loads(session.run([str(args.bin_dir/'canopa-lock'),'status']))['sleep_coordinator']
        except Exception:return False
    session.wait_for(ready,'lock coordinator')
    base=(ROOT/'packaging/stasis/stasis.rune').read_text()
    # No physical or fixture power action is possible during sampling.
    base=re.sub(r'timeout \d+', 'timeout 86400', base)
    runs=[]
    for repeat in range(args.runs):
        cases=[('off',None),('off','1'),('on',None),('on','1')]
        if repeat%2:cases.reverse()
        for detectors,workers in cases:
            label=f'{repeat}-{detectors}-{workers or "stock"}'
            config=root/(label+'.rune')
            config.write_text(base if detectors=='off' else base.replace('monitor_gamepad false','monitor_gamepad true').replace('monitor_games false','monitor_games true').replace('monitor_media false','monitor_media true'))
            config.chmod(0o600)
            session.env.pop('TOKIO_WORKER_THREADS',None)
            session.env['TOKIO_WORKER_THREADS']=workers or str(args.stock_workers)
            endpoint=root/'ready.sock';endpoint.unlink(missing_ok=True)
            notify=socket.socket(socket.AF_UNIX,socket.SOCK_DGRAM);notify.bind(str(endpoint));notify.settimeout(12)
            session.env['NOTIFY_SOCKET']=str(endpoint)
            process=session.start([str(args.bin_dir/'canopa-idle'),'--stasis',str(args.stasis),'run','--config',str(config)],label+'.log')
            session.env.pop('NOTIFY_SOCKET',None)
            try:
                assert notify.recv(1024)==b'READY=1'
                notify.close()
                time.sleep(args.settle_seconds)
                values=[sample(process.pid)]
                for _ in range(args.sample_seconds):time.sleep(1);values.append(sample(process.pid))
                result=sampler.summarize(values)
                result.update(detectors=detectors,tokio_workers=workers or 'stock',repeat=repeat,
                              process_tree=[dict(pid=p,exe=os.readlink(f'/proc/{p}/exe')) for p in tree(process.pid)])
                runs.append(result)
                (session.output/(label+'.json')).write_text(json.dumps(result,indent=2)+'\n')
                print(label,{key:result[key] for key in ('cpu_percent_one_core','rss_kib','pss_kib','threads','observed_voluntary_context_switches_per_second')},flush=True)
            finally:notify.close();session.retire(process)
    summary=dict(platform=platform.platform(),cpu_count=os.cpu_count(),sample_seconds=args.sample_seconds,
                 settle_seconds=args.settle_seconds,stasis_sha256=hashlib.sha256(args.stasis.read_bytes()).hexdigest(),
                 binaries={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (args.bin_dir/'canopa-idle',args.bin_dir/'canopa-lock')},
                 stock_sized_workers=args.stock_workers,
                 scope='Canopa idle supervisor plus real Stasis backend and readiness watch; listener/compositor excluded; optional detectors use private D-Bus and host process/device visibility',
                 limitations=['debug executable unless separately built optimized','scheduler sleeps are not hardware wakeups','no input or authentication during each measured interval'],
                 groups={})
    for detectors in ('off','on'):
        for workers in ('stock','1'):
            group=[r for r in runs if r['detectors']==detectors and r['tokio_workers']==workers]
            summary['groups'][f'{detectors}-{workers}']={
                key:statistics.median(r[key] if not isinstance(r[key],dict) else r[key]['median'] for r in group)
                for key in ('cpu_percent_one_core','rss_kib','pss_kib','private_kib','threads','observed_voluntary_context_switches_per_second')}
    (session.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2),flush=True)
    session.retire(listener)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir',type=Path,default=ROOT/'target/debug')
    parser.add_argument('--stasis',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--stock-workers',type=int,default=len(os.sched_getaffinity(0)),help='explicit stock-sized worker comparison; choose Rust available_parallelism for the deployment cgroup')
    parser.add_argument('--runs',type=int,default=2)
    parser.add_argument('--settle-seconds',type=int,default=5)
    parser.add_argument('--sample-seconds',type=int,default=30)
    args=parser.parse_args()
    if args.stock_workers<1:parser.error('stock-workers must be positive')
    if not 1<=args.runs<=10 or args.settle_seconds<0 or args.sample_seconds<10:parser.error('use 1..10 runs, nonnegative settle and at least 10 sample seconds')
    args.bin_dir=args.bin_dir.resolve();args.stasis=args.stasis.resolve();args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    (args.output/'sampler.py').write_bytes(Path(__file__).read_bytes())
    with tempfile.TemporaryDirectory(prefix='canopa-stasis-measure-') as temporary:
        session=desktop.Session(Path(temporary),args.output)
        try:measure(args,session)
        finally:session.close()

if __name__=='__main__':main()
