#!/usr/bin/env python3
"""Check custom paper through the original PDF filter using owned fixture files.

No scheduler or print job is created. The fixed cases use Canopa's 125x225mm
custom-range fixture and a four-page PDF with PAGE-1 through PAGE-4 labels.
Select cases explicitly to distinguish the adapter from the separate PWG-only
producer mapping issue. An optional libppd applies only to the filter process.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import traceback



def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main(args):
    out = args.output.resolve()
    library = args.library.resolve() if args.library else None
    filter_binary, ppd, source_pdf = [p.resolve() for p in [args.filter, args.ppd, args.source]]
    out.mkdir(parents=True)
    (out / 'probe-source.py').write_bytes(Path(__file__).read_bytes())
    pins = [filter_binary, ppd, source_pdf, Path('/usr/lib/libcups.so.2'), Path('/usr/lib/libppd.so.2'),
            Path('/usr/lib/libcupsfilters.so.2'), Path('/usr/lib/libpdfio.so.1')]
    if library:
        pins.append(library)
    result = dict(status='running', scope=__doc__, source_sha256=sha(Path(__file__)), pins={str(p): sha(p) for p in pins}, cases=[])
    def save():
        (out / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-filter-paper-') as directory:
            base = Path(directory)
            result['private_path'] = str(base)
            env = dict(PATH='/usr/bin:/bin', HOME=directory, TMPDIR=directory, LC_ALL='C',
                       USER='canopa-fixture', PRINTER='CanopaFixture', PPD=str(ppd),
                       CONTENT_TYPE='application/pdf', FINAL_CONTENT_TYPE='application/pdf',
                       CUPS_SERVER=str(base / 'no-scheduler.sock'),
                       DBUS_SYSTEM_BUS_ADDRESS='unix:path=' + str(base / 'no-bus.sock'),
                       DBUS_SESSION_BUS_ADDRESS='unix:path=' + str(base / 'no-bus.sock'))
            cases = [
                ('named-control', 'PageSize=125x225mm media=custom_canopa_125x225mm', (12500, 22500)),
                ('ppd-custom', 'PageSize=Custom.125.01x225.02mm', (12501, 22502)),
                ('ppd-near-a4', 'PageSize=Custom.209.99x297.01mm', (20999, 29701)),
                ('pwg-custom', 'media=custom_canopa_125.01x225.02mm', (12501, 22502)),
                ('numeric-collection', 'media-col={media-size={x-dimension=12501 y-dimension=22502} media-left-margin=635 media-right-margin=635 media-top-margin=1270 media-bottom-margin=1270}', (12501, 22502)),
            ]
            if args.case:
                cases = [case for case in cases if case[0] in args.case]
            for name, paper, dimensions in cases:
                row = dict(name=name, options=paper, expected_hundredths_mm=dimensions)
                result['cases'].append(row); save()
                start = time.monotonic()
                target = out / (name + '.pdf')
                filter_env = dict(env)
                if library:
                    filter_env.update(LD_LIBRARY_PATH=str(library.parent), LD_DEBUG='libs')
                with target.open('wb') as output, (out / (name + '.err')).open('wb') as errors:
                    p = subprocess.Popen([str(filter_binary), '1', 'canopa-fixture', 'Canopa owned PDF', '1',
                                          paper + ' sides=one-sided print-color-mode=monochrome', str(source_pdf)],
                                         env=filter_env, stdout=output, stderr=errors)
                    row['pid'] = p.pid
                    try:
                        row['start_ticks'] = int(Path(f'/proc/{p.pid}/stat').read_text().rsplit(')', 1)[1].split()[19])
                        row['exit_code'] = p.wait(timeout=15)
                    finally:
                        if p.poll() is None:
                            p.kill(); p.wait()
                        row['terminal'] = p.poll() is not None
                assert row['exit_code'] == 0, row
                if library:
                    init = 'calling init: ' + str(library)
                    row['selected_library_initialized'] = init in (out / (name + '.err')).read_text()
                    assert row['selected_library_initialized'], row
                info = subprocess.run(['pdfinfo', '-f', '1', '-l', '4', target], env=env,
                                      capture_output=True, text=True, check=True, timeout=5).stdout
                (out / (name + '.pdfinfo')).write_text(info)
                sizes = re.findall(r'^Page\s+\d+ size:\s+([\d.]+) x ([\d.]+) pts(?: \([^\n]*\))?$', info, re.M)
                assert re.search(r'^Pages:\s+4$', info, re.M) and len(sizes) == 4, info
                expected = [value * 72 / 2540 for value in dimensions]
                row.update(output_sha256=sha(target), actual_page_points=sizes,
                           exact_dimensions=all(abs(float(x) - expected[0]) < .002 and abs(float(y) - expected[1]) < .002 for x, y in sizes),
                           seconds=round(time.monotonic() - start, 3))
                text = subprocess.run(['pdftotext', target, '-'], env=env, capture_output=True,
                                      text=True, check=True, timeout=5).stdout
                row['page_labels'] = re.findall(r'PAGE-[1-4]', text)
                row['status'] = 'passed' if row['exact_dimensions'] and row['page_labels'] == ['PAGE-1', 'PAGE-2', 'PAGE-3', 'PAGE-4'] else 'failed'
                save()
        result['private_data_removed'] = not base.exists()
        result['status'] = 'passed' if all(row['status'] == 'passed' for row in result['cases']) else 'failed'
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        if 'private_path' in result:
            result['private_data_removed'] = not Path(result['private_path']).exists()
        assert all(sha(Path(path)) == digest for path, digest in result['pins'].items())
        save()
    print(json.dumps(dict(status=result['status'], cases=[{k: row.get(k) for k in ['name', 'status', 'actual_page_points']} for row in result['cases']])))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--filter', type=Path, default=Path('/usr/lib/cups/filter/pdftopdf'))
    parser.add_argument('--ppd', type=Path, required=True, help='Owned custom-range driverless fixture PPD')
    parser.add_argument('--source', type=Path, required=True, help='Owned four-page PDF containing PAGE-1 through PAGE-4')
    parser.add_argument('--library', type=Path, help='Select this libppd only for the filter process')
    parser.add_argument('--case', action='append', choices=['named-control', 'ppd-custom', 'ppd-near-a4', 'pwg-custom', 'numeric-collection'])
    raise SystemExit(main(parser.parse_args()))
