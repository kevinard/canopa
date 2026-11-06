#!/usr/bin/env python3
"""Verify staged package paths and session lifecycle without host service changes."""
import configparser
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]


def module(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT / 'scripts' / f'{name}.py')
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


install = module('install-system')
session = module('canopa-session')
attributions = module('dependency-attributions')
documentation = module('package-documentation')
STUB = '''#!/usr/bin/python3
import json, os, pathlib, sys
name=pathlib.Path(sys.argv[0]).name
with open(os.environ['CANOPA_TEST_CALLS'],'a') as output:
 output.write(json.dumps({'program':name,'args':sys.argv[1:],'config':os.environ.get('NIRI_CONFIG'),'desktop':os.environ.get('XDG_CURRENT_DESKTOP'),'session_pid':os.environ.get('CANOPA_SESSION_PID')})+'\\n')
if name=='systemctl':
 if 'is-active' in sys.argv: sys.exit(0 if os.environ.get('CANOPA_TEST_ACTIVE') else 3)
 if '--wait' in sys.argv and os.environ.get('CANOPA_TEST_WAIT'): __import__('time').sleep(120)
 if '--wait' in sys.argv: sys.exit(int(os.environ.get('CANOPA_TEST_START_STATUS','0')))
if name=='canopa-settings':
 path=pathlib.Path(os.environ['NIRI_CONFIG']); path.parent.mkdir(parents=True,exist_ok=True); path.write_text('// private initialized session\\n')
'''


class SystemInstallTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='canopa-system-test-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        for name in install.PROGRAMS:
            path = self.bin / name
            path.write_text('#!/bin/sh\nexit 0\n')
            path.chmod(0o755)

    def test_desktop_staging_uses_system_paths_and_does_not_enable_units(self):
        destination = self.root / 'desktop'
        install.stage(destination, self.bin)
        for name in install.PROGRAMS:
            self.assertTrue(os.access(destination / 'usr/bin' / name, os.X_OK))
        units = destination / 'usr/lib/systemd/user'
        for path in units.glob('*.service'):
            text = path.read_text()
            self.assertNotIn('%h/.local/bin', text)
            self.assertNotIn('[Install]', text)
        self.assertEqual(list(destination.rglob('*.wants')), [])
        self.assertIn('CANOPA_SESSION=1', (units / 'canopa-session.target').read_text())
        self.assertIn('/usr/bin/stasis', (units / 'canopa-idle.service').read_text())
        self.assertIn('Exec=/usr/bin/canopa-files', (destination / 'usr/share/dbus-1/services/org.freedesktop.FileManager1.service').read_text())
        self.assertTrue((destination / 'etc/pam.d/canopa-lock').is_file())
        self.assertFalse((destination / 'etc/greetd/config.toml').exists())
        self.assertIn('Settings=canopa;', (destination / 'usr/share/xdg-desktop-portal/canopa-portals.conf').read_text())
        for path in (destination / 'usr/share/applications').glob('*.desktop'):
            subprocess.run(['desktop-file-validate', str(path)], check=True)

    def test_login_and_compositor_packages_preserve_ownership_and_provenance(self):
        login = self.root / 'login'
        install.stage(login, self.bin, component='login')
        self.assertFalse((login / 'etc/greetd/config.toml').exists())
        self.assertIn('canopa-greeter', (login / 'usr/lib/sysusers.d/canopa-login.conf').read_text())
        compositor = self.root / 'compositor'
        install.stage(compositor, self.bin, self.bin / 'canopa-shell', component='compositor')
        self.assertTrue((compositor / 'usr/lib/canopa/niri').is_file())
        self.assertFalse((compositor / 'usr/bin/niri').exists())
        record = json.loads((compositor / 'usr/share/canopa/compositor-provenance.json').read_text())
        self.assertEqual(record['source']['sources']['niri']['revision'], '8ed0da44d974c32c6877d2f4630c314da0717ecb')
        self.assertEqual(len(record['binary_sha256']), 64)

    def test_oo7_package_is_separate_and_preserves_other_portal_choices(self):
        destination = self.root / 'oo7'
        (self.bin / 'canopa-keyring-prompter').write_text('#!/bin/sh\nexit 0\n')
        install.stage(destination, self.bin, component='oo7')
        self.assertFalse((destination / 'etc/pam.d/greetd').exists())
        pam = (destination / 'etc/pam.d/canopa-oo7').read_text().splitlines()
        hooks = [line.split() for line in pam if line and not line.startswith('#')]
        self.assertEqual(hooks, [[phase, 'optional', 'pam_oo7.so']
                                 for phase in ('auth', 'session')])
        portal = configparser.ConfigParser(interpolation=None)
        portal.optionxform = str
        portal.read(ROOT / 'packaging/system/canopa-portals.conf')
        expected = dict(portal['preferred'])
        expected['org.freedesktop.impl.portal.Secret'] = 'oo7-portal;'
        portal.clear()
        portal.read(destination / 'etc/xdg/xdg-desktop-portal/canopa-portals.conf')
        self.assertEqual(dict(portal['preferred']), expected)
        self.assertTrue(os.access(destination / 'usr/lib/canopa/oo7-wait-for-pam', os.X_OK))
        self.assertTrue(os.access(destination / 'usr/lib/canopa/canopa-keyring-prompter', os.X_OK))
        service = 'org.gnome.keyring.SystemPrompter.service'
        self.assertFalse((destination / 'usr/share/dbus-1/services' / service).exists())
        self.assertIn('Exec=/usr/lib/canopa/canopa-keyring-prompter',
                      (destination / 'usr/share/canopa/dbus-1/services' / service).read_text())
        self.assertEqual(list(destination.rglob('*.wants')), [])

    def test_system_unit_graph_has_no_ordering_cycle(self):
        destination = self.root / 'desktop'
        install.stage(destination, self.bin)
        units = destination / 'usr/lib/systemd/user'
        for name in ('basic', 'shutdown', 'sockets', 'graphical-session-pre', 'graphical-session', 'xdg-desktop-autostart'):
            (units / f'{name}.target').write_text('[Unit]\nDescription=Private fixture\n')
        (units / 'dbus.socket').write_text('[Socket]\nListenStream=%t/bus\n')
        (units / 'dbus.service').write_text('[Service]\nExecStart=/usr/bin/true\n')
        for path in units.glob('*.service'):
            path.write_text(re.sub(r'^(Exec(?:Start|Reload))=.*$', lambda match: match[1]+'=/usr/bin/true', path.read_text(), flags=re.M))
        runtime = self.root / 'runtime'
        runtime.mkdir(mode=0o700)
        result = subprocess.run(['systemd-analyze', '--user', '--man=no', '--recursive-errors=yes', 'verify', str(units / 'canopa-compositor.service'), str(units / 'canopa-session.target')],
                                env={**os.environ, 'SYSTEMD_UNIT_PATH': str(units), 'XDG_RUNTIME_DIR': str(runtime)}, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_refuses_root_destination_and_missing_builds(self):
        with self.assertRaises(ValueError): install.stage(Path('/'), self.bin)
        (self.bin / 'canopa-settings').unlink()
        with self.assertRaisesRegex(ValueError, 'canopa-settings'): install.stage(self.root / 'missing', self.bin)

    def invoke_session(self, *, active=False, status=0, print_command=False):
        tools = self.root / 'tools'
        tools.mkdir()
        for name in ('systemctl', 'dbus-update-activation-environment', 'canopa-settings'):
            path = tools / name; path.write_text(STUB); path.chmod(0o755)
        calls = self.root / 'calls.jsonl'
        environment = {'PATH': str(tools) + os.pathsep + os.environ['PATH'], 'XDG_CONFIG_HOME': str(self.root / 'config'),
                       'CANOPA_TEST_CALLS': str(calls), 'CANOPA_TEST_START_STATUS': str(status)}
        if active: environment['CANOPA_TEST_ACTIVE'] = '1'
        with mock.patch.dict(os.environ, environment), mock.patch.object(session, 'COMPOSITOR', self.bin / 'canopa-shell'), \
                mock.patch.object(sys, 'argv', ['canopa-session', *(['--print-command'] if print_command else [])]), contextlib.redirect_stdout(io.StringIO()) as output:
            try: result = session.main()
            except RuntimeError as error: result = str(error)
        events = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
        return result, events, output.getvalue()

    def test_session_environment_start_and_failure_cleanup_use_real_child_boundaries(self):
        result, calls, _ = self.invoke_session(status=27)
        self.assertEqual(result, 27)
        settings = next(item for item in calls if item['program'] == 'canopa-settings')
        self.assertEqual(settings['desktop'], 'canopa:niri')
        self.assertEqual(settings['config'], str(self.root / 'config/canopa/niri.kdl'))
        self.assertTrue(any('canopa-shutdown.target' in item['args'] for item in calls))
        imported = next(item for item in calls if 'import-environment' in item['args'])
        self.assertIn('CANOPA_SESSION_PID', imported['args'])
        self.assertEqual(imported['session_pid'], str(os.getpid()))
        self.assertIn('CANOPA_SESSION_PID', calls[-2]['args'])
        self.assertIn('unset-environment', calls[-2]['args'])
        self.assertEqual(calls[-1]['program'], 'dbus-update-activation-environment')
        self.assertFalse(any('enable' in item['args'] for item in calls))

    def test_active_session_is_refused_before_initialization(self):
        result, calls, _ = self.invoke_session(active=True)
        self.assertIn('already running', result)
        self.assertFalse(any(item['program'] == 'canopa-settings' for item in calls))
        self.assertFalse((self.root / 'config').exists())

    def test_print_command_has_no_side_effects(self):
        result, calls, output = self.invoke_session(print_command=True)
        self.assertEqual(result, 0)
        self.assertEqual(calls, [])
        self.assertEqual(json.loads(output)['command'][-1], 'canopa-compositor.service')

    def test_upstream_members_inherit_only_matching_workspace_notices(self):
        upstream = self.root / 'upstream'
        member = upstream / 'member'
        member.mkdir(parents=True)
        (upstream / 'Cargo.toml').write_text('[workspace]\n[workspace.package]\nlicense="MIT"\n')
        (upstream / 'LICENSE').write_text('Original upstream copyright and permission notice\n')
        package = dict(name='fixture', version='1.0', id='fixture', source=None,
                       license='MIT', license_file=None, manifest_path=str(member / 'Cargo.toml'))
        destination = self.root / 'notices'
        attributions.collect_notices(package, destination)
        self.assertEqual((destination / 'fixture-1.0/LICENSE').read_text(), (upstream / 'LICENSE').read_text())
        package['license'] = 'Apache-2.0'
        with self.assertRaisesRegex(RuntimeError, 'No notices'):
            attributions.collect_notices(package, self.root / 'rejected-notices')

    def test_source_archive_docs_retain_development_notes_without_scratch_data(self):
        guide = self.root / 'guide.md'
        guide.write_text('[Design](.scratch/feature/PRD.md)\n')
        output = self.root / 'documentation'
        with mock.patch.object(documentation, 'ROOT', self.root), \
                mock.patch.object(documentation, 'FILES', {'guide.md': 'share/guide.md'}):
            documentation.stage(output)
            self.assertIn('development checkout: `.scratch/feature/PRD.md`', (output / 'share/guide.md').read_text())
            guide.write_text('[Broken](missing.md)\n')
            with self.assertRaisesRegex(ValueError, 'missing source reference'):
                documentation.stage(output)
            guide.write_text('![Missing](.scratch/private.png)\n')
            with self.assertRaisesRegex(ValueError, 'unshipped image'):
                documentation.stage(output)

    def test_terminated_session_launcher_stops_its_session_and_clears_environment(self):
        tools = self.root / 'tools'
        tools.mkdir()
        for name in ('systemctl', 'dbus-update-activation-environment', 'canopa-settings'):
            path = tools / name; path.write_text(STUB); path.chmod(0o755)
        calls = self.root / 'calls.jsonl'
        env = {**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'],
               'XDG_CONFIG_HOME': str(self.root / 'config'), 'CANOPA_TEST_CALLS': str(calls),
               'CANOPA_TEST_WAIT': '1'}
        wrapper = (f"import importlib.util,sys;spec=importlib.util.spec_from_file_location('session', {str(ROOT / 'scripts/canopa-session.py')!r});"
                   f"module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);"
                   f"module.COMPOSITOR=__import__('pathlib').Path({str(self.bin / 'canopa-shell')!r});sys.argv=['canopa-session'];sys.exit(module.main())")
        for interruption in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            with self.subTest(signal=interruption):
                calls.unlink(missing_ok=True)
                process = subprocess.Popen([sys.executable, '-c', wrapper], env=env,
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
                try:
                    deadline = time.monotonic() + 5
                    while time.monotonic() < deadline:
                        if calls.exists() and any('--wait' in json.loads(line)['args'] for line in calls.read_text().splitlines()):
                            break
                        time.sleep(.02)
                    else: self.fail('private session did not reach its compositor wait')
                    process.send_signal(interruption)
                    self.assertEqual(process.wait(timeout=5), 128 + interruption)
                    events = [json.loads(line) for line in calls.read_text().splitlines()]
                    self.assertTrue(any('canopa-shutdown.target' in event['args'] for event in events))
                    self.assertIn('unset-environment', events[-2]['args'])
                    self.assertEqual(events[-1]['program'], 'dbus-update-activation-environment')
                finally:
                    try: os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError: pass
                    process.wait()


if __name__ == '__main__':
    unittest.main()
