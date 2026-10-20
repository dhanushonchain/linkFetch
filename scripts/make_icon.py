"""Build the macOS app icon from the flat logo PNG.

The source is a black square with the artwork in the middle. macOS wants a
squircle with transparent padding, so: crop to the artwork, drop it on a
rounded black plate and export a 1024x1024 RGBA PNG.
"""

import struct
import sys
import zlib

SRC = "assets/logo-source.png"
OUT = "assets/logo.png"
SIZE = 1024
PLATE = 824  # 80% of the canvas, like Apple's icon grid
RADIUS = 185  # Big Sur corner radius


def read_png(path):
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos = 8
    idat = b""
    w = h = None
    bitdepth = colortype = None
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        ctype = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        if ctype == b"IHDR":
            w, h, bitdepth, colortype = struct.unpack(">IIBB", chunk[:10])
        elif ctype == b"IDAT":
            idat += chunk
        elif ctype == b"IEND":
            break
        pos += 12 + length
    raw = zlib.decompress(idat)
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[colortype]
    stride = w * channels
    out = bytearray(h * stride)
    prev = bytearray(stride)
    p = 0
    for y in range(h):
        f = raw[p]
        p += 1
        line = bytearray(raw[p : p + stride])
        p += stride
        if f == 1:
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif f == 3:
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif f == 4:
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                c = prev[i - channels] if i >= channels else 0
                b = prev[i]
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 0xFF
        out[y * stride : (y + 1) * stride] = line
        prev = line
    return w, h, channels, out


def write_png(path, w, h, rgba):
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        raw += rgba[y * w * 4 : (y + 1) * w * 4]

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    open(path, "wb").write(png)


def artwork_bbox(w, h, ch, buf):
    minx, miny, maxx, maxy = w, h, -1, -1
    for y in range(h):
        for x in range(w):
            i = (y * w + x) * ch
            r, g, b = buf[i], buf[i + 1], buf[i + 2]
            if r + g + b > 90:  # not the black background
                if x < minx:
                    minx = x
                if x > maxx:
                    maxx = x
                if y < miny:
                    miny = y
                if y > maxy:
                    maxy = y
    return minx, miny, maxx, maxy


def main():
    w, h, ch, buf = read_png(SRC)
    minx, miny, maxx, maxy = artwork_bbox(w, h, ch, buf)
    print("artwork bbox", minx, miny, maxx, maxy, "of", w, h)

    # Crop the artwork with a little padding around it.
    pad = 10
    minx = max(0, minx - pad)
    miny = max(0, miny - pad)
    maxx = min(w - 1, maxx + pad)
    maxy = min(h - 1, maxy + pad)
    cw, chh = maxx - minx + 1, maxy - miny + 1

    # Scale the artwork to fit the plate with margin.
    inner = int(PLATE * 0.86)
    scale = min(inner / cw, inner / chh)
    tw, th = max(1, int(cw * scale)), max(1, int(chh * scale))
    ox = (SIZE - tw) // 2
    oy = (SIZE - th) // 2

    # Use the logo's own background colour so the pasted artwork blends in.
    bg = buf[0], buf[1], buf[2]

    canvas = bytearray(SIZE * SIZE * 4)
    plate_x0 = (SIZE - PLATE) // 2
    plate_y0 = (SIZE - PLATE) // 2
    plate_x1 = plate_x0 + PLATE
    plate_y1 = plate_y0 + PLATE

    # Rounded plate (black) with alpha, drawn with a simple distance test.
    for y in range(plate_y0, plate_y1):
        for x in range(plate_x0, plate_x1):
            dx = max(plate_x0 + RADIUS - x, 0, x - (plate_x1 - RADIUS - 1))
            dy = max(plate_y0 + RADIUS - y, 0, y - (plate_y1 - RADIUS - 1))
            if dx * dx + dy * dy > RADIUS * RADIUS:
                continue
            i = (y * SIZE + x) * 4
            canvas[i : i + 4] = bytes((bg[0], bg[1], bg[2], 255))

    # Paste the artwork (area averaged downscale).
    for ty in range(th):
        sy0 = miny + int(ty / scale)
        sy1 = max(sy0 + 1, min(h, miny + int((ty + 1) / scale)))
        for tx in range(tw):
            sx0 = minx + int(tx / scale)
            sx1 = max(sx0 + 1, min(w, minx + int((tx + 1) / scale)))
            r = g = b = n = 0
            for sy in range(sy0, sy1):
                base = sy * w
                for sx in range(sx0, sx1):
                    i = (base + sx) * ch
                    r += buf[i]
                    g += buf[i + 1]
                    b += buf[i + 2]
                    n += 1
            if not n:
                continue
            px = ox + tx
            py = oy + ty
            if 0 <= px < SIZE and 0 <= py < SIZE:
                i = (py * SIZE + px) * 4
                canvas[i] = r // n
                canvas[i + 1] = g // n
                canvas[i + 2] = b // n
                canvas[i + 3] = 255

    write_png(OUT, SIZE, SIZE, canvas)
    print("wrote", OUT)


main()
