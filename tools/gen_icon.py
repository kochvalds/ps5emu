#!/usr/bin/env python3
"""Генерирует assets/ps5emu.png. Геометрия совпадает с src/icon.rs и assets/ps5emu.svg."""
import math, struct, sys, zlib

def mix(a, b, t):
    t = max(0.0, min(1.0, t))
    return [a[i] + (b[i] - a[i]) * t for i in range(3)]

def edge(ax, ay, bx, by, px, py):
    l = math.hypot(bx - ax, by - ay)
    return ((bx - ax) * (py - ay) - (by - ay) * (px - ax)) / l

def render(n):
    s = float(n)
    rows = []
    for y in range(n):
        row = bytearray([0])  # фильтр PNG: None
        for x in range(n):
            u, v = (x + 0.5) / s, (y + 0.5) / s
            px, py = abs(u - 0.5) - 0.32, abs(v - 0.5) - 0.32
            d = math.hypot(max(px, 0.0), max(py, 0.0)) + min(max(px, py), 0.0) - 0.18
            alpha = max(0.0, min(1.0, 0.5 - d * s))
            if alpha <= 0.0:
                row += bytes([0, 0, 0, 0]); continue
            c = mix([24, 36, 92], [88, 40, 160], (u + v) / 2)
            dist = math.hypot(u - 0.5, v - 0.5)
            c = mix(c, [90, 170, 255], math.exp(-abs(dist - 0.30) * 9.0) * 0.35)
            ring = max(0.0, min(1.0, 0.5 - (abs(dist - 0.30) - 0.028) * s))
            c = mix(c, [155, 224, 255], ring)
            inside = min(-edge(0.43, 0.38, 0.43, 0.62, u, v),
                         -edge(0.43, 0.62, 0.64, 0.50, u, v),
                         -edge(0.64, 0.50, 0.43, 0.38, u, v))
            c = mix(c, [255, 255, 255], max(0.0, min(1.0, 0.5 + inside * s)))
            row += bytes([int(c[0]), int(c[1]), int(c[2]), int(alpha * 255)])
        rows.append(bytes(row))
    return b"".join(rows)

def png(n, raw):
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 256
    out = sys.argv[2] if len(sys.argv) > 2 else "assets/ps5emu.png"
    open(out, "wb").write(png(n, render(n)))
    print("записано", out)
