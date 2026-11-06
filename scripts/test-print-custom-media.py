#!/usr/bin/env python3
"""Check exact custom paper through the actual worker and an owned CUPS queue.

The caller provides a private Unix scheduler socket and an already configured
queue. Only CheckPrinter metadata operations are sent: no document or job is
created. Sizes use integer hundredths of a millimetre, matching CUPS/IPP units.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time
import traceback


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def size(value):
    try:
        width, height = map(int, value.split('x'))
        assert 0 < width <= 1_000_000 and 0 < height <= 1_000_000
        return width, height
    except (AssertionError, ValueError):
        raise argparse.ArgumentTypeError('Use WIDTHxHEIGHT in integer hundredths of a millimetre') from None


def margins(value):
    try:
        parsed = tuple(map(int, value.split(',')))
        assert len(parsed) == 4 and all(0 <= item <= 1_000_000 for item in parsed)
        return parsed
    except (AssertionError, ValueError):
        raise argparse.ArgumentTypeError('Use LEFT,BOTTOM,RIGHT,TOP in integer hundredths of a millimetre') from None


def main(args):
    assert args.scheduler.is_socket(), 'An owned private Unix scheduler socket is required'
    assert args.library or not args.expected_library_sha256
    binary = args.binary.resolve()
    result = dict(status='running', scope=__doc__, binary=str(binary), binary_sha256=sha(binary),
                  queue=args.queue, instance=args.instance, cases=[])
    image = None
    read, write = os.pipe2(os.O_CLOEXEC)
    try:
        if args.library:
            library = args.library.resolve()
            assert library.is_file() and 0 < library.stat().st_size <= 16 * 1024 * 1024
            result['library_sha256'] = sha(library)
            if args.expected_library_sha256:
                assert result['library_sha256'] == args.expected_library_sha256
            image = os.memfd_create('canopa-custom-media-client', os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING | 0x10)
            os.write(image, library.read_bytes())
            fcntl.fcntl(image, fcntl.F_ADD_SEALS, fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL)
            inode = os.fstat(image).st_ino
        with tempfile.TemporaryDirectory(prefix='canopa-custom-media-check-') as directory:
            if args.config_home:
                assert args.config_home.is_dir()
            env = dict(os.environ, CUPS_SERVER=str(args.scheduler.resolve()),
                       HOME=str(args.config_home.resolve()) if args.config_home else directory, LC_ALL='C')
            env.pop('LD_LIBRARY_PATH', None)
            env.pop('CANOPA_PRINT_CUPS_LIBRARY', None)
            for expected, requested in [(True, item) for item in args.size] + [(False, item) for item in args.reject]:
                width, height = requested
                options = dict(copies=1, media=None, sides=args.sides, color=False)
                packet = dict(cancel=read, operation=dict(operation='check_printer',
                              destination=dict(queue=args.queue, instance=args.instance),
                              options=options, custom_size=dict(width=width, height=height)))
                descriptors = (read,)
                if image is not None:
                    packet['cups_runtime'] = dict(kind='image', descriptor=image)
                    descriptors += (image,)
                row = dict(width=width, height=height, expected_supported=expected, selected_image_mapped=False)
                result['cases'].append(row)
                p = subprocess.Popen([str(binary), '--print-worker'], env=env, stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, pass_fds=descriptors)
                row['pid'] = p.pid
                row['start_ticks'] = int(Path(f'/proc/{p.pid}/stat').read_text().rsplit(')', 1)[1].split()[19])
                def observe():
                    while p.poll() is None:
                        try:
                            lines = [line for line in Path(f'/proc/{p.pid}/maps').read_text().splitlines()
                                     if 'memfd:canopa-custom-media-client' in line or '/libcups.so' in line]
                            if lines:
                                assert all(int(line.split()[4]) == inode for line in lines)
                                row['selected_image_mapped'] = True
                                return
                        except FileNotFoundError:
                            return
                        except BaseException as error:
                            row['mapping_error'] = str(error)
                            return
                        time.sleep(.001)
                observer = threading.Thread(target=observe, daemon=True) if image is not None else None
                if observer:
                    observer.start()
                try:
                    output, errors = p.communicate((json.dumps(packet) + '\n').encode(), timeout=30)
                    row.update(exit_code=p.returncode, frames=[json.loads(line) for line in output.splitlines()], stderr=errors.decode())
                finally:
                    if p.poll() is None:
                        p.kill(); p.communicate()
                    row['terminal'] = p.poll() is not None
                    if observer:
                        observer.join(timeout=2)
                        assert not observer.is_alive()
                assert row['exit_code'] == 0 and len(row['frames']) == 1, row
                if image is not None:
                    assert row['selected_image_mapped'] and 'mapping_error' not in row, row
                frame = row['frames'][0]
                if expected:
                    assert frame['event'] == 'checked_printer', row
                    media = frame['options']['custom_media']
                    assert abs(media['width_mm'] * 100 - width) < .000001
                    assert abs(media['height_mm'] * 100 - height) < .000001
                    assert all(media[field] >= 0 for field in ['top_mm', 'bottom_mm', 'left_mm', 'right_mm'])
                    assert media['left_mm'] + media['right_mm'] < media['width_mm']
                    assert media['top_mm'] + media['bottom_mm'] < media['height_mm']
                    if args.expected_margins is not None:
                        actual = tuple(round(media[field] * 100) for field in ['left_mm', 'bottom_mm', 'right_mm', 'top_mm'])
                        assert actual == args.expected_margins, (actual, args.expected_margins)
                else:
                    assert frame == dict(event='failed', message='Printer does not support this exact custom paper size'), row
                row['status'] = 'passed'
        assert sha(binary) == result['binary_sha256'], 'Worker executable changed during the test'
        result.update(status='passed', private_temporary_data_removed=True)
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        result['private_temporary_data_removed'] = 'directory' not in locals() or not Path(directory).exists()
        os.close(read); os.close(write)
        if image is not None:
            os.close(image)
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(status=result['status'], cases=len(result['cases']), receipt=str(args.receipt), error=result.get('error'))))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--scheduler', type=Path, required=True)
    parser.add_argument('--queue', required=True)
    parser.add_argument('--instance')
    parser.add_argument('--config-home', type=Path, help='Owned fixture home containing saved printer instances')
    parser.add_argument('--sides', choices=['one_sided', 'long_edge', 'short_edge'], default='one_sided')
    parser.add_argument('--library', type=Path)
    parser.add_argument('--expected-library-sha256')
    parser.add_argument('--size', type=size, action='append', required=True)
    parser.add_argument('--reject', type=size, action='append', default=[])
    parser.add_argument('--expected-margins', type=margins,
                        help='Assert LEFT,BOTTOM,RIGHT,TOP for each accepted sheet, in hundredths mm')
    parser.add_argument('--receipt', type=Path, required=True)
    raise SystemExit(main(parser.parse_args()))
