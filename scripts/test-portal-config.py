#!/usr/bin/env python3
"""Check portal selection without modifying the session or compiling applications."""
import configparser
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'scripts/portal-config.py'
spec = importlib.util.spec_from_file_location('portal_config', SCRIPT)
config = importlib.util.module_from_spec(spec)
spec.loader.exec_module(config)
P = config.PREFIX


def values(text):
    parser = configparser.ConfigParser(interpolation=None, strict=False)
    parser.optionxform = str
    parser.read_string(text)
    return dict(parser['preferred'])


class PortalConfig(unittest.TestCase):
    def test_selected_routes_keep_explicit_and_default_fallbacks(self):
        base = '# My Niri setup\n[preferred]\ndefault=gtk;gnome;\n' + P + 'FileChooser=gnome;gtk;\n' + P + 'ScreenCast=gnome\n' + P + 'Print=gtk\n'
        result = config.merge(base, {P + 'FileChooser': 'canopa', P + 'Settings': 'canopa'})
        actual = values(result)
        self.assertEqual(actual[P + 'FileChooser'], 'canopa;gnome;gtk;')
        self.assertEqual(actual[P + 'Settings'], 'canopa;gtk;gnome;')
        for key in ['default', P + 'ScreenCast', P + 'Print']:
            self.assertEqual(actual[key], values(base)[key])
        self.assertTrue(result.startswith('# My Niri setup\n'))

    def test_unselected_lines_comments_and_other_sections_are_preserved(self):
        base = '# before\n[preferred]\n  ' + P + 'Settings = gnome\n# after\n[unrelated]\nvalue=100% # literal text\n'
        result = config.merge(base, {P + 'Settings': 'canopa'})
        self.assertEqual(result, base.replace('Settings = gnome', 'Settings = canopa;gnome;'))

    def test_new_routes_are_inserted_in_the_preferred_group(self):
        base = '[preferred]\ndefault=gtk\n[other]\nvalue=unchanged\n'
        result = config.merge(base, {P + 'Settings': 'canopa'})
        self.assertIn(P + 'Settings=canopa;gtk;\n[other]', result)
        self.assertEqual(values(result)['default'], 'gtk')

    def test_repeated_groups_and_keys_use_the_effective_last_value(self):
        base = '[preferred]\n' + P + 'Settings=gtk\n[other]\na=b\n[preferred]\n' + P + 'Settings=gnome\n'
        result = config.merge(base, {P + 'Settings': 'canopa'})
        self.assertEqual(result, base.replace('Settings=gnome', 'Settings=canopa;gnome;'))
        self.assertEqual(values(result)[P + 'Settings'], 'canopa;gnome;')

    def test_crlf_and_existing_no_final_newline_are_preserved(self):
        base = '[preferred]\r\ndefault=gtk\r\n' + P + 'Settings=gnome'
        self.assertEqual(config.merge(base, {P + 'Settings': 'canopa'}), base.replace('Settings=gnome', 'Settings=canopa;gnome;'))
        result = config.merge('[preferred]\r\ndefault=gtk', {P + 'Settings': 'canopa'})
        self.assertNotIn('\n', result.replace('\r\n', ''))

    def test_missing_preferred_group_is_created_without_losing_other_data(self):
        base = '# keep\n[other]\nkey=value'
        result = config.merge(base, {P + 'Settings': 'canopa'})
        self.assertTrue(result.startswith(base + '\n[preferred]\n'))
        self.assertEqual(values(result)[P + 'Settings'], 'canopa;')

    def test_special_fallbacks_and_duplicates_keep_their_order(self):
        for old, expected in [('gnome;canopa;gnome;*;', 'canopa;gnome;*;'), ('none', 'canopa;'), ('', 'canopa;')]:
            with self.subTest(old=old):
                result = config.merge('[preferred]\ndefault=' + old, {P + 'Settings': 'canopa'})
                self.assertEqual(values(result)[P + 'Settings'], expected)

    def test_selecting_an_interface_removes_its_none_veto_without_reviving_disabled_backends(self):
        base = '[preferred]\ndefault=gnome;none;gtk;\n' + P + 'FileChooser=none;\n' + P + 'Print=none;\n'
        result = values(config.merge(base, {P + 'Settings': 'canopa', P + 'FileChooser': 'canopa'}))
        self.assertEqual(result[P + 'Settings'], 'canopa;')
        self.assertEqual(result[P + 'FileChooser'], 'canopa;')
        self.assertEqual(result['default'], 'gnome;none;gtk;')
        self.assertEqual(result[P + 'Print'], 'none;')

    def test_malformed_or_unsupported_backend_syntax_is_refused(self):
        for base in ['default=gtk', '[preferred]\nbroken', '[preferred]\ndefault=gtk\\;unexpected', '[preferred]\ndefault=gtk;;gnome', '[preferred]\ndefault=gtk\0', 'x' * (config.MAX_CONFIG + 1)]:
            with self.subTest(base=base[:60]):
                with self.assertRaises(ValueError):
                    config.merge(base, {P + 'Settings': 'canopa'})

    def test_real_metadata_uses_the_separate_notify_backend_and_omits_print(self):
        routes = config.available(ROOT / 'packaging/portal')
        self.assertEqual(routes[P + 'FileChooser'], 'canopa')
        self.assertEqual(routes[P + 'Notification'], 'canopa-notify')
        for name in ['Print', 'ScreenCast', 'InputCapture']:
            self.assertNotIn(P + name, routes)

    def test_cli_refuses_unadvertised_interfaces_and_output_replacement(self):
        with tempfile.TemporaryDirectory(prefix='canopa-portal-config-') as temporary:
            root = Path(temporary);base = root / 'base';output = root / 'candidate'
            original = b'[preferred]\ndefault=gnome\n';base.write_bytes(original)
            def run(*args):
                return subprocess.run([sys.executable, str(SCRIPT), '--base', str(base), *args], capture_output=True, timeout=10)
            for name in ['Print', 'ScreenCast', 'NotAPortal']:
                result = run('--interface', name, '--output', str(output))
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(output.exists())
            result = run('--interface', 'Notification', '--output', str(output))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(values(output.read_text())[P + 'Notification'], 'canopa-notify;gnome;')
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            before = output.read_bytes()
            self.assertEqual(run('--output', str(output)).returncode, 2)
            self.assertEqual(output.read_bytes(), before)
            output.unlink();output.symlink_to(base)
            self.assertEqual(run('--output', str(output)).returncode, 2)
            self.assertEqual(base.read_bytes(), original)
            self.assertEqual(run('--output', str(base)).returncode, 2)

    def test_cli_defaults_and_relocation_use_bundle_descriptors(self):
        with tempfile.TemporaryDirectory(prefix='canopa-portal-config-relocated-') as temporary:
            import shutil
            root = Path(temporary) / 'prefix with spaces';program = root / 'bin/canopa-portal-config'
            program.parent.mkdir(parents=True);shutil.copyfile(SCRIPT, program)
            descriptors = root / 'share/xdg-desktop-portal/portals';descriptors.mkdir(parents=True)
            for name in ['canopa', 'canopa-notify']:
                shutil.copyfile(ROOT / 'packaging/portal' / (name + '.portal'), descriptors / (name + '.portal'))
            descriptor = descriptors / 'canopa.portal'
            descriptor.write_text(descriptor.read_text().replace('Interfaces=', 'Interfaces=' + P + 'ScreenCast;'))
            base = root / 'base';base.write_text('[preferred]\ndefault=gnome\n')
            def run(*args):
                return subprocess.run([sys.executable, str(program), *args], capture_output=True, text=True, cwd='/', timeout=10)
            result = run('--base', str(base))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(set(values(result.stdout)), {'default', P + 'Settings', P + 'FileChooser'})
            result = run('--base', str(base), '--interface', 'ScreenCast')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(values(result.stdout)[P + 'ScreenCast'], 'canopa;gnome;')
            result = run('--list')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('ScreenCast\tcanopa', result.stdout)
            self.assertIn('Notification\tcanopa-notify', result.stdout)
            self.assertNotIn('Print', result.stdout)


if __name__ == '__main__':
    unittest.main()
