#!/usr/bin/env python3
"""Check printable annotation appearances through the actual isolated worker.

No printer or scheduler is contacted. Copy-only conversion must preserve the
printable stamp, omit a screen-only stamp, and print a NoView/Print stamp. Both
one-copy and collated two-copy requests preserve source geometry and page order.
"""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import traceback
from PIL import Image


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main(args):
    binary, out = args.binary.resolve(), args.output.resolve()
    out.mkdir(parents=True)
    (out / 'probe-source.py').write_bytes(Path(__file__).read_bytes())
    fixture = Path(__file__).with_name('desktop-fixture') / 'print-paper-document.py'
    spec = importlib.util.spec_from_file_location('paper_document', fixture)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    result = dict(status='running', scope=__doc__, binary_sha256=sha(binary),
                  source_sha256=sha(Path(__file__)), fixture_sha256=sha(fixture), cases=[])
    def save():
        (out / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-annotation-check-') as directory:
            env = dict(os.environ, HOME=directory, TMPDIR=directory, XDG_CACHE_HOME=directory+'/cache',
                       FONTCONFIG_FILE='/etc/fonts/fonts.conf', LC_ALL='C', CUPS_SERVER=directory+'/no-scheduler.sock')
            for copies in [1, 2]:
                for annotation in ['print', 'screen', 'print-hidden']:
                    case = out / f'{annotation}-{copies}'
                    case.mkdir()
                    source = case / 'input.pdf'
                    source.write_bytes(module.document(annotation, args.width, args.height))
                    qpdf = subprocess.run(['qpdf', '--check', source], env=env, capture_output=True, timeout=5)
                    assert qpdf.returncode == 0, qpdf.stderr
                    row = dict(annotation=annotation, copies=copies, expected_printed=annotation != 'screen', status='running')
                    result['cases'].append(row); save()
                    document = os.memfd_create('annotation-source', os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING)
                    target = os.memfd_create('annotation-output', os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING)
                    read, write = os.pipe2(os.O_CLOEXEC)
                    try:
                        data = source.read_bytes()
                        assert os.write(document, data) == len(data)
                        seals = fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL
                        fcntl.fcntl(document, fcntl.F_ADD_SEALS, seals)
                        layout = dict(orientation='portrait', copies=copies, collate=True, reverse=False, scale=100.,
                                      pages='all', ranges=[], page_set='all', number_up=1, number_up_layout='lrtb')
                        packet = dict(cancel=read, operation=dict(operation='render', source=document, target=target,
                                      request=dict(layout=layout, page_setup={}, format='pdf', mode='copies', duplex=False)))
                        p = subprocess.Popen([binary, '--print-worker'], env=env, stdin=subprocess.PIPE,
                                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, pass_fds=(read, document, target))
                        row['pid'] = p.pid
                        row['start_ticks'] = int(Path(f'/proc/{p.pid}/stat').read_text().rsplit(')', 1)[1].split()[19])
                        try:
                            output, errors = p.communicate((json.dumps(packet)+'\n').encode(), timeout=45)
                        finally:
                            if p.poll() is None:
                                p.kill(); p.communicate()
                            row['terminal'] = p.poll() is not None
                        row.update(exit_code=p.returncode, frames=[json.loads(line) for line in output.splitlines()], stderr=errors.decode())
                        assert p.returncode == 0 and row['frames'][-1]['event'] == 'rendered', row
                        assert fcntl.fcntl(target, fcntl.F_GET_SEALS) & seals == seals
                        rendered = case / 'output.pdf'
                        rendered.write_bytes(os.pread(target, os.fstat(target).st_size, 0))
                    finally:
                        for fd in [document, target, read, write]:
                            os.close(fd)
                    count = copies * 4
                    info = subprocess.run(['pdfinfo', '-f', '1', '-l', str(count), rendered], env=env,
                                          capture_output=True, text=True, check=True, timeout=5).stdout
                    (case / 'pdfinfo.out').write_text(info)
                    objects = json.loads(subprocess.check_output(['qpdf', '--json', rendered], env=env, timeout=5))['qpdf'][1]
                    dictionaries = [value['value'] for value in objects.values() if isinstance(value, dict) and isinstance(value.get('value'), dict)]
                    boxes = [value.get('/MediaBox') for value in dictionaries if value.get('/Type') == '/Page']
                    row['page_boxes'] = boxes
                    row['geometry_preserved'] = len(boxes) == count and all(len(box) == 4 and all(abs(actual - expected) < 0.000001 for actual, expected in zip(box, [0, 0, args.width, args.height])) for box in boxes)
                    text = subprocess.run(['pdftotext', rendered, '-'], env=env, capture_output=True,
                                          text=True, check=True, timeout=5).stdout
                    row['labels'] = re.findall(r'PAGE-[1-4]', text)
                    subprocess.run(['pdftoppm', '-r', '144', '-gray', '-png', rendered, case / 'page'],
                                   env=env, capture_output=True, check=True, timeout=15)
                    row['stamp_ink_pixels'] = []
                    for page in range(1, count+1):
                        with Image.open(case / f'page-{page}.png') as frame:
                            image = frame.convert('L')
                            cx, cy = round(267.637795*2), round((args.height-390.944882)*2)
                            ink = sum(image.getpixel((x,y)) < 32 for x in range(cx-8,cx+8) for y in range(cy-8,cy+8))
                            row['stamp_ink_pixels'].append(ink)
                    expected = [row['expected_printed'] and (page-1) % 4 == 0 for page in range(1,count+1)]
                    actual = [ink > 250 for ink in row['stamp_ink_pixels']]
                    row['status'] = 'passed' if actual == expected and row['geometry_preserved'] and row['labels'] == ['PAGE-1','PAGE-2','PAGE-3','PAGE-4'] * copies else 'failed'
                    save()
            result['status'] = 'passed' if all(row['status'] == 'passed' for row in result['cases']) else 'failed'
        result['private_data_removed'] = not Path(directory).exists()
        assert sha(binary) == result['binary_sha256']
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        save()
    print(json.dumps(dict(status=result['status'], cases=[{k:row.get(k) for k in ['annotation','copies','status','stamp_ink_pixels']} for row in result['cases']])))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--width', type=float, default=595.275591, help='Source width in PDF points')
    parser.add_argument('--height', type=float, default=841.889764, help='Source height in PDF points')
    raise SystemExit(main(parser.parse_args()))
