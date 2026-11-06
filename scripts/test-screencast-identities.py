#!/usr/bin/env python3
"""Verify Rust PipeWire serial lookup using private synthetic video producers."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
CASE='screencast::production::serial::tests::native_identity_fixture'


def verify(args,session):
    session.env.update(PIPEWIRE_RUNTIME_DIR=str(session.runtime),PIPEWIRE_REMOTE='pipewire-0')
    session.start(['pipewire'],'pipewire.log')
    session.wait_for(lambda:(session.runtime/'pipewire-0').is_socket(),'private PipeWire')
    assert CASE+': test' in subprocess.check_output([str(args.harness),'--list'],text=True,timeout=10)
    with (session.output/'resolver.log').open('w') as log:
        helper=subprocess.Popen([str(args.harness),'--ignored','--exact',CASE,'--nocapture'],env={**session.env,'CANOPA_IDENTITY_NATIVE_FIXTURE':'1'},stdin=subprocess.PIPE,stdout=log,stderr=log,text=True,start_new_session=True)
    session.processes.append(helper)
    def responses():return [json.loads(line.split('IDENTITIES ',1)[1]) for line in (session.output/'resolver.log').read_text().splitlines() if 'IDENTITIES ' in line]
    def resolve(nodes):
        before=len(responses());helper.stdin.write(json.dumps({'nodes':nodes})+'\n');helper.stdin.flush()
        return session.wait_for(lambda:responses()[before] if len(responses())>before else None,'Rust identity result',timeout=10)
    def nodes():
        values=json.loads(session.run(['pw-dump']))
        return [value for value in values if value.get('info',{}).get('props',{}).get('node.name')=='canopa-cast-fixture']
    def produce(label):
        producer=session.start(['gst-launch-1.0','-q','videotestsrc','is-live=true','!','video/x-raw,format=RGB,width=64,height=48,framerate=5/1','!','pipewiresink','mode=provide','stream-properties=props,node.name=canopa-cast-fixture,media.class=Video/Source'],label+'.log')
        node=session.wait_for(lambda:next(iter(nodes()),None),'synthetic video node')
        return producer,node['id'],int(node['info']['props']['object.serial'])
    producer,node,serial=produce('first')
    assert resolve([node])=={'identities':{str(node):serial}}
    session.retire(producer);session.wait_for(lambda:not nodes(),'producer removal')
    assert resolve([node])=={'unavailable':True}
    producer,next_node,next_serial=produce('second')
    assert next_serial!=serial
    assert resolve([next_node])=={'identities':{str(next_node):next_serial}}
    helper.stdin.write('{"stop":true}\n');helper.stdin.flush();assert helper.wait(timeout=5)==0;session.processes.remove(helper)
    return {'status':'passed','checks':['actual PipeWire node resolves to its 64-bit object serial','removed producer cannot return a cached identity','replacement producer has a fresh serial and resolves independently'],'numeric_id_reused':node==next_node,'harness_sha256':hashlib.sha256(args.harness.read_bytes()).hexdigest()}


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--harness',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();args.harness=args.harness.resolve(strict=True);args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-cast-identities-') as directory:
        session=desktop.Session(Path(directory),args.output)
        try:result=verify(args,session)
        except Exception as error:
            desktop.write_json(args.output/'result.json',{'status':'failed','error':str(error)});raise
        finally:session.close()
    desktop.write_json(args.output/'result.json',result);print(json.dumps(result))


if __name__=='__main__':main()
