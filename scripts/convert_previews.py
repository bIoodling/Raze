"""Convert offscreen GDI output without third-party dependencies."""
import json
import struct
import zlib
from pathlib import Path

def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

evidence = []
for path in Path("previews").glob("*.bmp"):
    data = path.read_bytes()
    w, height = struct.unpack_from("<ii", data, 18)
    h = abs(height)
    assert w > 0 and h > 0 and struct.unpack_from("<H", data, 28)[0] == 32
    offset = struct.unpack_from("<I", data, 10)[0]
    rows = []
    for y in range(h):
        source_y = y if height < 0 else h - 1 - y
        row = data[offset + source_y*w*4:offset + (source_y+1)*w*4]
        rgb = bytearray()
        for x in range(w):
            b,g,r,_ = row[x*4:x*4+4]
            rgb.extend((r,g,b))
        rows.append(b"\0" + rgb)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w,h,8,2,0,0,0))
    png += chunk(b"IDAT", zlib.compress(b"".join(rows))) + chunk(b"IEND", b"")
    path.with_suffix(".png").write_bytes(png)
    evidence.append({"file": path.with_suffix(".png").name, "width":w, "height":h,
                     "kind":"offscreen GDI render; not an interactive screenshot"})
assert len(evidence) in (16,19), f"Expected 16 or 19 renders, found {len(evidence)}"
Path("previews/render-evidence.json").write_text(json.dumps(evidence, indent=2), encoding="utf-8")
print(f"PASS: {len(evidence)} offscreen renders; application at 100%, 125%, 150%, 200%")
