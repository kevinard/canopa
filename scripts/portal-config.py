#!/usr/bin/env python3
"""Generate a Canopa portal configuration from an explicitly selected base file.

Prints the result by default. Never installs files or restarts session services.
"""
import argparse
import configparser
import os
from pathlib import Path
import re
import sys

PREFIX = 'org.freedesktop.impl.portal.'
MAX_CONFIG = 256 * 1024
SECTION = re.compile(r'^\s*\[([^\[\]]+)\]\s*$')
ENTRY = re.compile(r'^(\s*([^=\s]+)\s*=\s*)(.*)$')
BACKEND = re.compile(r'(?:[A-Za-z0-9_.-]+|\*)\Z')
INTERFACE = re.compile(re.escape(PREFIX) + r'[A-Za-z][A-Za-z0-9]*\Z')


def read_text(path):
    with path.open('rb') as stream:
        data = stream.read(MAX_CONFIG + 1)
    if len(data) > MAX_CONFIG or b'\0' in data:
        raise ValueError(f'Invalid or oversized portal file: {path}')
    return data.decode('utf-8')


def portal_directory():
    prefix = Path(__file__).resolve().parents[1]
    source = prefix / 'packaging/portal'
    return source if source.is_dir() else prefix / 'share/xdg-desktop-portal/portals'


def available(directory):
    """Use backend IDs from shipped filenames, including Notify's separate ID."""
    routes = {}
    for backend in ['canopa', 'canopa-notify']:
        path = directory / (backend + '.portal')
        if not path.exists() and backend == 'canopa-notify':
            continue
        descriptor = configparser.ConfigParser(interpolation=None)
        descriptor.optionxform = str
        descriptor.read_string(read_text(path))
        if descriptor['portal']['DBusName'] != {
            'canopa': 'org.freedesktop.impl.portal.desktop.canopa',
            'canopa-notify': 'org.freedesktop.impl.portal.desktop.canopa.notify',
        }[backend]:
            raise ValueError(f'Unexpected Canopa backend identity: {path}')
        for interface in descriptor['portal']['Interfaces'].split(';'):
            if not interface:
                continue
            if not INTERFACE.fullmatch(interface) or interface in routes:
                raise ValueError(f'Invalid or ambiguous Canopa interface: {interface}')
            routes[interface] = backend
    return routes


def backend_list(value):
    values = [part.strip() for part in value.split(';')]
    while values and not values[-1]:
        values.pop()
    if any(not BACKEND.fullmatch(part) for part in values):
        raise ValueError('Cannot preserve this backend list; expected backend IDs separated by semicolons')
    return values


def merge(text, selections):
    """Change only selected effective keys; retain all other lines verbatim.

    Key-file groups and keys can repeat: the last value is effective. There are
    no multiline values or inline comments in GLib's key-file syntax.
    """
    if '\0' in text or len(text.encode('utf-8')) > MAX_CONFIG:
        raise ValueError('Invalid or oversized portal configuration')
    parts = text.split('\n')
    lines = [part + '\n' for part in parts[:-1]]
    if parts[-1]:
        lines.append(parts[-1])
    newline = '\r\n' if next((line for line in lines if line.endswith('\n')), '').endswith('\r\n') else '\n'
    group = None
    entries = {}
    preferred_end = None
    for index, line in enumerate(lines):
        raw = line.rstrip('\r\n')
        if index == 0:
            raw = raw.removeprefix('\ufeff')
        if not raw.strip() or raw.lstrip().startswith('#'):
            continue
        section = SECTION.fullmatch(raw)
        if section:
            if group == 'preferred':
                preferred_end = index
            group = section[1]
            if group == 'preferred':
                preferred_end = len(lines)
            continue
        entry = ENTRY.fullmatch(raw)
        if group is None or entry is None:
            raise ValueError(f'Invalid key-file syntax at line {index + 1}')
        if group == 'preferred':
            entries[entry[2]] = (index, entry[1], entry[3])

    additions = []
    for interface, backend in selections.items():
        if not INTERFACE.fullmatch(interface) or not BACKEND.fullmatch(backend):
            raise ValueError('Invalid interface selection')
        current = entries.get(interface)
        fallback = current or entries.get('default')
        old = backend_list(fallback[2]) if fallback else []
        # 'none' vetoes the entire list, even after a valid backend. Explicit
        # selection replaces a disabled route; its inactive names are not fallbacks.
        if 'none' in old:
            old = []
        value = ';'.join(dict.fromkeys([backend, *old])) + ';'
        if current:
            index, prefix, _ = current
            ending = '\r\n' if lines[index].endswith('\r\n') else '\n' if lines[index].endswith('\n') else ''
            lines[index] = prefix + value + ending
        else:
            additions.append(interface + '=' + value + newline)

    if additions:
        if preferred_end is None:
            if lines and not lines[-1].endswith('\n'):
                lines[-1] += newline
            lines += ['[preferred]' + newline, *additions]
        else:
            if preferred_end and not lines[preferred_end - 1].endswith('\n'):
                lines[preferred_end - 1] += newline
            lines[preferred_end:preferred_end] = additions
    return ''.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', type=Path, help='Complete configuration currently selected for this session; never edited')
    parser.add_argument('--interface', action='append', help='Short or full interface name; repeat to select more (default: Settings and FileChooser)')
    parser.add_argument('--portal-dir', type=Path, default=portal_directory(), help='Canopa descriptor directory (default: this checkout or bundle)')
    parser.add_argument('--output', type=Path, help='Create a new review file; refuse existing files and symlinks')
    parser.add_argument('--list', action='store_true', help='List interfaces advertised by these Canopa descriptors')
    args = parser.parse_args()
    try:
        routes = available(args.portal_dir)
        if args.list:
            if args.base or args.interface or args.output:
                raise ValueError('--list cannot be combined with --base, --interface or --output')
            for name, backend in sorted(routes.items()):
                print(name.removeprefix(PREFIX) + '\t' + backend)
            return
        if not args.base:
            raise ValueError('--base is required; select the complete configuration for your session')
        selections = {}
        for name in args.interface or ['Settings', 'FileChooser']:
            interface = name if name.startswith(PREFIX) else PREFIX + name
            if interface not in routes:
                raise ValueError(f'Interface is not advertised by these Canopa descriptors: {name}')
            selections[interface] = routes[interface]
        result = merge(read_text(args.base), selections)
        if args.output:
            fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
            with os.fdopen(fd, 'w', encoding='utf-8', newline='') as stream:
                stream.write(result)
            print(f'Created {args.output}; review it before installation. No session services changed.', file=sys.stderr)
        else:
            sys.stdout.buffer.write(result.encode('utf-8'))
    except (OSError, ValueError, KeyError, configparser.Error) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
