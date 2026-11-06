#!/usr/bin/env python3
"""Create four deterministic A4 pages with centered paper-calibration marks.

The marks fit the minimum custom sheet used by the owned driverless fixture at
100% scale. This avoids treating two identically cropped blank pages as evidence
that a configured printer path preserves visible content.
"""
import argparse
from pathlib import Path


def document(annotation='none', width=595.275591, height=841.889764):
    objects = [
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Kids [4 0 R 6 0 R 8 0 R 10 0 R] /Count 4 >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    ]
    for page in range(1, 5):
        contents = len(objects) + 2
        annot = ' /Annots [12 0 R]' if annotation != 'none' and page == 1 else ''
        objects.append(f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << /Font << /F1 3 0 R >> >> /Contents {contents} 0 R{annot} >>'.encode())
        body = (f'q 0 0 0 RG 1.5 w 247.637795 370.944882 100 100 re S '
                f'0 0 0 RG 287.637795 420.944882 m 307.637795 420.944882 l S '
                f'297.637795 410.944882 m 297.637795 430.944882 l S '
                f'BT /F1 18 Tf 258 446 Td (PAGE-{page}) Tj ET Q\n').encode()
        objects.append(f'<< /Length {len(body)} >>\nstream\n'.encode() + body + b'endstream')
    if annotation != 'none':
        flags = {'print': 4, 'screen': 0, 'print-hidden': 36}[annotation]
        objects.append(f'<< /Type /Annot /Subtype /Stamp /Name /Approved /P 4 0 R /Rect [257.637795 380.944882 277.637795 400.944882] /F {flags} /AP << /N 13 0 R >> >>'.encode())
        body = b'q 0 g 0 0 20 20 re f Q\n'
        objects.append(f'<< /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Resources << >> /Length {len(body)} >>\nstream\n'.encode() + body + b'endstream')
    output = bytearray(b'%PDF-1.7\n%\xe2\xe3\xcf\xd3\n')
    offsets = [0]
    for number, body in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f'{number} 0 obj\n'.encode() + body + b'\nendobj\n')
    start = len(output)
    output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:
        output.extend(f'{offset:010} 00000 n \n'.encode())
    output.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode())
    return bytes(output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--annotation', choices=['none', 'print', 'screen', 'print-hidden'], default='none')
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(document(args.annotation))
