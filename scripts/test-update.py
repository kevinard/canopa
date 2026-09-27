#!/usr/bin/env python3
"""Exercise the updater with real Cargo builds and isolated user services."""

import contextlib
import fcntl
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPTS = Path(__file__).resolve().parent
PROGRAMS = ('niribar', 'niribar-wifi-provider', 'canopa-notify', 'canopa')
SERVICES = ('niribar.service', 'canopa-notify.service')
LOCAL_PACKAGES = ('canopa-panel-protocol', 'canopa-config', 'canopa-assets', 'accesskit_unix')
SYSTEMCTL = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

path = Path(os.environ['CANOPA_TEST_SERVICES'])
state = json.loads(path.read_text())
args = sys.argv[1:]
state['calls'].append(args)
services = [arg for arg in args if arg.endswith('.service')]
result = 0

def stop(service):
    pid = service['pid']
    if pid:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        for _ in range(100):
            try:
                if Path(f'/proc/{pid}/stat').read_text().split(') ', 1)[1][0] == 'Z':
                    break
            except FileNotFoundError:
                break
            time.sleep(0.01)
    service.update(active='inactive', pid=0)

if 'show' in args:
    service = state[services[0]]
    values = {'ActiveState': service['active'], 'MainPID': str(service['pid']),
              'LoadState': 'loaded'}
    for arg in args:
        if arg.startswith('--property='):
            print(values[arg.split('=', 1)[1]])
elif 'is-active' in args:
    service = state[services[0]]
    try:
        if Path(f"/proc/{service['pid']}/stat").read_text().split(') ', 1)[1][0] == 'Z':
            service.update(active='failed', pid=0)
    except FileNotFoundError:
        service.update(active='failed', pid=0)
    result = 0 if service['active'] == 'active' else 3
elif 'stop' in args:
    for name in services:
        stop(state[name])
elif 'reset-failed' in args:
    for name in services:
        if state[name]['active'] == 'failed':
            state[name]['active'] = 'inactive'
elif 'start' in args:
    for name in services:
        service = state[name]
        binary = Path.home() / '.local/bin' / name.removesuffix('.service')
        if service['mode'] == 'wrong-binary':
            binary = Path(os.environ['CANOPA_TEST_OLD_BINARY'])
        command = [str(binary)]
        if name == 'canopa-notify.service':
            command.append('daemon')
        version = subprocess.check_output([str(binary), '--version'], text=True)
        if service['mode'] == 'crash' and version.startswith('current'):
            command = [str(binary), '--version']
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        state['pids'].append(process.pid)
        service.update(active='active', pid=process.pid)
path.write_text(json.dumps(state))
sys.exit(result)
'''
RUST_MAIN = r'''
const VERSION: &str = "@VERSION@";
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("{} {} {} {} {}", VERSION, canopa_panel_protocol::VERSION,
            canopa_config::VERSION, canopa_assets::VERSION, accesskit_unix::VERSION);
        return;
    }
    if args.get(1).map(String::as_str) == Some("config") {
        if std::env::var_os("CANOPA_TEST_BAD_CONFIG").is_some() {
            eprintln!("invalid test configuration");
            std::process::exit(1);
        }
        let exe = std::env::current_exe().unwrap();
        for name in ["niribar", "canopa-notify"] {
            let output = std::process::Command::new(exe.parent().unwrap().join(name))
                .arg("--version").output().unwrap();
            assert!(String::from_utf8(output.stdout).unwrap().starts_with(VERSION));
        }
        println!("{{\"component\":\"fixture\",\"source\":{{}}}}");
        return;
    }
    if args.get(1).map(String::as_str) == Some("ctl") {
        if std::env::var_os("CANOPA_TEST_RENDER_ERROR").is_some() && VERSION == "current" {
            println!("{}", r#"{"available":true,"presentation_error":"renderer stopped","sender_publication_error":null,"configuration":{"error":null}}"#);
        } else {
            println!("{}", r#"{"available":true,"presentation_error":null,"sender_publication_error":null,"configuration":{"error":null}}"#);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("daemon") && VERSION == "current" {
        if let Some(path) = std::env::var_os("CANOPA_TEST_MIGRATE") {
            std::fs::write(path, b"new schema").unwrap();
        }
    }
    loop { std::thread::sleep(std::time::Duration::from_secs(60)); }
}
'''


class UpdateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='canopa-update-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / 'checkout with spaces'
        self.write_project(self.repo, 'current')
        (self.repo / 'scripts').mkdir()
        for name in ['update.sh', 'update.py']:
            shutil.copy2(SCRIPTS / name, self.repo / 'scripts' / name)
        self.bin_dir = self.root / 'user/.local/bin'
        self.bin_dir.mkdir(parents=True)
        self.service = self.root / 'services.json'
        self.service.write_text(json.dumps({
            'calls': [], 'pids': [],
            **{name: dict(active='inactive', pid=0, mode='normal') for name in SERVICES}
        }))
        commands = self.root / 'commands'
        commands.mkdir()
        (commands / 'systemctl').write_text(SYSTEMCTL)
        (commands / 'systemctl').chmod(0o755)
        self.config = self.root / 'config/canopa/config.kdl'
        self.config.parent.mkdir(parents=True)
        self.config.write_text('schema-version 1\n')
        self.database = self.root / 'state/canopa-notify/history.db'
        self.database.parent.mkdir(parents=True)
        self.database.write_bytes(b'original history')
        self.env = os.environ.copy()
        self.env.pop('CARGO_TARGET_DIR', None)
        self.env['CARGO_HOME'] = str(Path(os.environ.get('CARGO_HOME', '~/.cargo')).expanduser())
        self.env['RUSTUP_HOME'] = str(Path(os.environ.get('RUSTUP_HOME', '~/.rustup')).expanduser())
        self.env.update(HOME=str(self.root / 'user'), PATH=f"{commands}:{self.env['PATH']}",
                        CARGO_NET_OFFLINE='true', CARGO_BUILD_JOBS='1',
                        XDG_CONFIG_HOME=str(self.root / 'config'),
                        XDG_STATE_HOME=str(self.root / 'state'),
                        XDG_RUNTIME_DIR=str(self.root / 'runtime'),
                        CANOPA_TEST_SERVICES=str(self.service),
                        CANOPA_TEST_OLD_BINARY=str(self.root / 'old-niribar'))
        self.addCleanup(self.stop_services)
        self.run_command(['cargo', 'generate-lockfile', '--offline'], self.repo)

    def write_project(self, root, version):
        root.mkdir(parents=True, exist_ok=True)
        (root / 'Cargo.toml').write_text(
            '[workspace]\nresolver = "2"\nmembers = ["crates/niribar", '
            '"crates/canopa-notify", "crates/canopa-cli"]\n'
            'exclude = ["vendor/accesskit_unix"]\n'
            '[patch.crates-io]\naccesskit_unix = { path = "vendor/accesskit_unix" }\n')
        for name in LOCAL_PACKAGES:
            package = root / 'vendor' / name
            (package / 'src').mkdir(parents=True)
            (package / 'Cargo.toml').write_text(
                f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n')
            (package / 'src/lib.rs').write_text(f'pub const VERSION: &str = "{version}";\n')
        (root / 'vendor/provenance.json').write_text(json.dumps({'crates': {'accesskit_unix': {}}}))
        deps = '\n'.join('accesskit_unix = "0.1.0"' if name == 'accesskit_unix' else
                         f'{name} = {{ path = "../../vendor/{name}" }}' for name in LOCAL_PACKAGES)
        source = RUST_MAIN.replace('@VERSION@', version)
        for name, binary in [('niribar', 'niribar'), ('canopa-notify', 'canopa-notify'),
                             ('canopa-cli', 'canopa')]:
            package = root / 'crates' / name
            (package / 'src').mkdir(parents=True)
            (package / 'Cargo.toml').write_text(
                f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n'
                f'[[bin]]\nname = "{binary}"\npath = "src/main.rs"\n[dependencies]\n{deps}\n')
            (package / 'src/main.rs').write_text(source)
            if name == 'niribar':
                (package / 'src/bin').mkdir()
                (package / 'src/bin/niribar-wifi-provider.rs').write_text(source)

    def run_command(self, args, cwd=None, check=True):
        result = subprocess.run(args, cwd=cwd or self.root, env=self.env,
                                text=True, capture_output=True, timeout=90)
        if check:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def update(self, check=True, *args):
        return self.run_command(['bash', str(self.repo / 'scripts/update.sh'), *args], check=check)

    def state(self, service=None, **changes):
        state = json.loads(self.service.read_text())
        if service:
            state[service].update(changes)
            self.service.write_text(json.dumps(state))
            return state[service]
        return state

    def stop_services(self):
        for pid in self.state()['pids']:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass

    def install_previous(self):
        baseline = self.root / 'old checkout'
        self.write_project(baseline, 'old')
        self.run_command(['cargo', 'build', '--release', '--workspace', '--bins',
                          '--target-dir', str(self.repo / 'target')], baseline)
        for name in PROGRAMS:
            shutil.copy2(self.repo / 'target/release' / name, self.bin_dir / name)
        shutil.copy2(self.bin_dir / 'niribar', self.root / 'old-niribar')

    def assert_versions(self, version):
        for name in PROGRAMS:
            result = self.run_command([str(self.bin_dir / name), '--version'])
            self.assertEqual(result.stdout.strip(), ' '.join([version] * 5))

    def backup(self):
        paths = list((self.root / 'state/canopa-backups').iterdir())
        self.assertEqual(len(paths), 1)
        return paths[0]

    def test_rebuilds_all_local_packages_after_other_checkout_used_cache(self):
        self.install_previous()
        self.update()
        self.assert_versions('current')
        self.assertEqual(self.database.read_bytes(), b'original history')
        self.assertEqual((self.backup() / 'notify-state/history.db').read_bytes(), b'original history')
        self.assertEqual(self.config.read_text(), 'schema-version 1\n')

    def test_unchanged_update_reuses_compiled_packages(self):
        self.update()
        binaries = [self.repo / 'target/release' / name for name in PROGRAMS]
        modified = [binary.stat().st_mtime_ns for binary in binaries]
        result = self.update()
        self.assertNotIn('Compiling ', result.stderr)
        self.assertEqual([binary.stat().st_mtime_ns for binary in binaries], modified)
        self.assert_versions('current')

    def test_cached_update_rebuilds_changed_sources_and_vendor_forks(self):
        self.update()
        for source in [*self.repo.glob('crates/**/src/**/*.rs'),
                       *self.repo.glob('vendor/*/src/*.rs')]:
            source.write_text(source.read_text().replace('"current"', '"edited"'))
        self.update()
        self.assert_versions('edited')
        result = self.update()
        self.assertNotIn('Compiling ', result.stderr)

    def test_shell_edit_reuses_unchanged_packages(self):
        self.update()
        source = self.repo / 'crates/niribar/src/main.rs'
        source.write_text(source.read_text().replace('fn main() {', '''fn main() {
    if std::env::args().any(|arg| arg == "--edited") {
        println!("edited");
        return;
    }
'''))
        result = self.update()
        self.assertIn('Compiling niribar ', result.stderr)
        for package in (*LOCAL_PACKAGES, 'canopa-notify', 'canopa-cli'):
            self.assertNotIn(f'Compiling {package} ', result.stderr)
        edited = self.run_command([str(self.bin_dir / 'niribar'), '--edited'])
        self.assertEqual(edited.stdout.strip(), 'edited')

    def test_rebuilds_after_other_checkout_overwrites_warm_cache(self):
        self.update()
        self.install_previous()
        self.assert_versions('old')
        self.update()
        self.assert_versions('current')
        result = self.update()
        self.assertNotIn('Compiling ', result.stderr)

    def test_explicit_rebuild_invalidates_warm_cache(self):
        self.update()
        result = self.update(True, '--rebuild')
        self.assertIn('Compiling niribar ', result.stderr)
        self.assertIn('Compiling accesskit_unix ', result.stderr)
        self.assert_versions('current')
        result = self.update()
        self.assertNotIn('Compiling ', result.stderr)

    def test_updates_and_restarts_both_services(self):
        for service in SERVICES:
            self.state(service, active='active')
        result = self.update()
        for service in SERVICES:
            state = self.state(service)
            self.assertEqual(state['active'], 'active')
            self.assertTrue(os.path.samefile(f"/proc/{state['pid']}/exe",
                                            self.bin_dir / service.removesuffix('.service')))
            self.assertIn(f"PID {state['pid']}", result.stdout)

    def test_recovers_failed_service_but_leaves_stopped_companion_stopped(self):
        self.state('canopa-notify.service', active='failed')
        result = self.update()
        self.assertEqual(self.state('canopa-notify.service')['active'], 'active')
        self.assertEqual(self.state('niribar.service')['active'], 'inactive')
        self.assertIn('niribar.service is inactive', result.stdout)
        self.assert_versions('current')

    def test_leaves_both_intentionally_stopped_services_stopped(self):
        self.update()
        for service in SERVICES:
            self.assertEqual(self.state(service)['active'], 'inactive')
        self.assertFalse(any('start' in call for call in self.state()['calls']))

    def test_startup_crash_restores_binaries_and_pre_migration_database(self):
        self.install_previous()
        for service in SERVICES:
            self.state(service, active='active')
        self.state('niribar.service', mode='crash')
        self.env['CANOPA_TEST_MIGRATE'] = str(self.database)
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('failed during startup', result.stderr)
        self.assert_versions('old')
        self.assertEqual(self.database.read_bytes(), b'original history')
        self.assertEqual((self.backup() / 'failed-update-notify-state/history.db').read_bytes(), b'new schema')
        for service in SERVICES:
            self.assertEqual(self.state(service)['active'], 'active')

    def test_rejects_different_executable_and_restores_installation(self):
        self.install_previous()
        self.state('niribar.service', active='active', mode='wrong-binary')
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('different executable', result.stderr)
        self.assert_versions('old')

    def test_renderer_failure_restores_installation(self):
        self.install_previous()
        self.state('canopa-notify.service', active='active')
        self.env['CANOPA_TEST_RENDER_ERROR'] = '1'
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('rendering error', result.stderr)
        self.assert_versions('old')

    def test_build_failure_preserves_installation_and_running_services(self):
        self.install_previous()
        (self.repo / 'crates/canopa-notify/src/main.rs').write_text('invalid Rust')
        self.state('canopa-notify.service', active='active')
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assert_versions('old')
        self.assertFalse(any('stop' in call for call in self.state()['calls']))

    def test_invalid_configuration_preserves_installation_and_running_services(self):
        self.install_previous()
        self.env['CANOPA_TEST_BAD_CONFIG'] = '1'
        self.state('canopa-notify.service', active='active')
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('invalid test configuration', result.stderr)
        self.assert_versions('old')
        self.assertFalse(any('stop' in call for call in self.state()['calls']))
        self.assertEqual(self.database.read_bytes(), b'original history')

    def test_help_is_read_only_and_rejects_invalid_jobs(self):
        result = self.update(True, '--help')
        self.assertIn('does not fetch Git updates', ' '.join(result.stdout.split()))
        self.assertEqual(self.state()['calls'], [])
        result = self.update(False, '--jobs', '0')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.state()['calls'], [])

    def test_concurrent_update_is_rejected_before_touching_services(self):
        with (self.bin_dir / '.canopa-update.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('already running', result.stderr)
        self.assertEqual(self.state()['calls'], [])

    def test_cleanup_discovery_does_not_request_registry_dependencies(self):
        self.install_previous()
        # Full metadata resolves/downloads even dependencies for other operating
        # systems. Reject that query at the actual Cargo process boundary; allow
        # local metadata, cleanup and compilation to use real Cargo normally.
        cargo = self.root / 'commands/cargo'
        cargo.write_text('''#!/usr/bin/env python3
import os
import sys
if sys.argv[1] == 'metadata' and '--no-deps' not in sys.argv:
    print('Downloading crates ...', file=sys.stderr)
    sys.exit(42)
os.execv(os.environ['CANOPA_TEST_REAL_CARGO'], ['cargo', *sys.argv[1:]])
''')
        cargo.chmod(0o755)
        self.env['CANOPA_TEST_REAL_CARGO'] = shutil.which('cargo')
        self.update()
        self.assert_versions('current')

    def test_timeout_reports_command_and_decoded_partial_output(self):
        spec = importlib.util.spec_from_file_location('canopa_updater', self.repo / 'scripts/update.py')
        updater = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(updater)
        # subprocess.TimeoutExpired keeps partial stderr as bytes, even when
        # text=True. Exercise that real behavior rather than inventing its shape.
        with self.assertRaises(subprocess.TimeoutExpired) as raised:
            updater.run(sys.executable, '-c',
                        'import sys,time; sys.stderr.write("Downloading crates ...\\n"); '
                        'sys.stderr.flush(); time.sleep(10)', timeout=0.2)
        output = io.StringIO()
        with mock.patch.dict(os.environ, self.env, clear=True), \
                mock.patch.object(sys, 'argv', ['update.sh']), \
                mock.patch.object(updater, 'update', side_effect=raised.exception), \
                contextlib.redirect_stderr(output):
            self.assertEqual(updater.main(), 1)
        diagnostic = output.getvalue()
        self.assertIn('timed out', diagnostic)
        self.assertIn('0.2', diagnostic)
        self.assertIn(sys.executable, diagnostic)
        self.assertIn('Downloading crates ...\n', diagnostic)
        self.assertNotIn("b'", diagnostic)


if __name__ == '__main__':
    unittest.main()
