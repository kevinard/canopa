#!/usr/bin/env python3
"""Exercise real libc NSS in a private mount namespace with synthetic records."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/canopa-greeter')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='canopa-nss-') as directory:
        root = Path(directory)
        (root / 'passwd').write_text(
            'root:x:0:0:Root:/root:/bin/sh\n'
            'service:x:42:42:Service:/nonexistent:/usr/bin/nologin\n'
            'alice:x:1000:1000:Alice NSS,Office,555-private:/private/alice:/bin/sh\n'
            'bob:x:1200:1200:Bob NSS:/private/bob:/bin/sh\n'
            'enterprise:x:200000:200000:Enterprise:/private/enterprise:/bin/sh\n'
            'disabled:x:1001:1001:Disabled:/nonexistent:/usr/bin/nologin\n')
        (root / 'nsswitch.conf').write_text('passwd: files\ngroup: files\n')
        (root / 'login.defs').write_text('UID_MIN 1000\nUID_MAX 60000\n')
        command = ['bwrap', '--unshare-user', '--unshare-pid', '--unshare-net',
                   '--unshare-ipc', '--unshare-uts', '--uid', '0', '--gid', '0',
                   '--ro-bind', '/', '/', '--dev', '/dev', '--proc', '/proc',
                   '--tmpfs', '/run', '--die-with-parent', '--clearenv']
        for name in ['passwd', 'nsswitch.conf', 'login.defs']:
            command += ['--ro-bind', str(root / name), '/etc/' + name]
        command += [str(args.binary.resolve()), '--defaults', 'list-users', '--source', 'nss']
        result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=8)
        users = json.loads(result.stdout)
        assert [(u['username'], u['display_name'], u['uid']) for u in users] == [
            ('alice', 'Alice NSS', 1000), ('bob', 'Bob NSS', 1200)]
        assert all(s not in result.stdout for s in ['555-', '/private', 'Office'])
        # Changing only the nameservice switch proves this is NSS, not a passwd-file parser.
        (root / 'nsswitch.conf').write_text('passwd: canopa_test_nonexistent\n')
        result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=8)
        assert json.loads(result.stdout) == []
    print('PASS: real libc NSS, login.defs bounds, nameservice-switch routing, limited GECOS')


if __name__ == '__main__':
    main()
