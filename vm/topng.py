#!/usr/bin/env python3
"""Save the VM's framebuffer (32-bit BGRA, portrait) as a PNG, without
third-party modules.

    topng.py FB0_RAW OUTPUT.png [WIDTH HEIGHT]
"""

import struct
import sys
import zlib

src, out = sys.argv[1], sys.argv[2]
w = int(sys.argv[3]) if len(sys.argv) > 3 else 1072
h = int(sys.argv[4]) if len(sys.argv) > 4 else 1448

with open(src, 'rb') as f:
    data = f.read(w * h * 4)
rgb = bytearray(w * h * 3)
rgb[0::3] = data[2::4]
rgb[1::3] = data[1::4]
rgb[2::3] = data[0::4]
stride = w * 3
rows = b''.join(b'\0' + bytes(rgb[y * stride:(y + 1) * stride]) for y in range(h))


def chunk(kind, body):
    return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))


with open(out, 'wb') as f:
    f.write(b'\x89PNG\r\n\x1a\n')
    f.write(chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)))
    f.write(chunk(b'IDAT', zlib.compress(rows, 6)))
    f.write(chunk(b'IEND', b''))
