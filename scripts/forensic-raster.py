#!/usr/bin/env python3
"""Decode a local length-prefixed capture without importing the Rust encoder.

This reads files only. It has no network, printer, or Bluetooth operations.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct


def sha(data):
    return hashlib.sha256(data).hexdigest()


def decode(data):
    writes = []
    offset = 0
    while offset < len(data):
        if offset + 2 > len(data):
            raise ValueError("truncated capture length")
        size = struct.unpack_from("<H", data, offset)[0]
        offset += 2
        if size == 0 or offset + size > len(data):
            raise ValueError("truncated or empty capture write")
        writes.append(data[offset:offset + size])
        offset += size
    if len(writes) < 2:
        raise ValueError("missing header/body")
    header = writes[0]
    if len(header) != 9 or header[:4] != bytes.fromhex("ff f0 12 34"):
        raise ValueError("invalid header")
    if sum(header[:8]) % 256 != header[8]:
        raise ValueError("invalid header checksum")
    for index, write in enumerate(writes[1:]):
        if write[0] != index + (index >= 27):
            raise ValueError("invalid chunk sequence")
    if writes[-1][-2:] != bytes.fromhex("12 34"):
        raise ValueError("invalid final magic")
    body = b"".join(w[1:] for w in writes[1:-1]) + writes[-1][1:-2]
    if len(body) != struct.unpack_from("<I", header, 4)[0]:
        raise ValueError("body length mismatch")
    if body[:6] != bytes.fromhex("1b 73 9a 02 00 00"):
        raise ValueError("invalid START")
    pos = 6
    copies = None
    if body[pos:pos + 2] == b"\x1b#":
        copies = body[pos + 2]
        pos += 3
    if body[pos:pos + 2] != b"\x1bD":
        raise ValueError("missing ESC D")
    mode, alignment, width, height = struct.unpack_from("<BBII", body, pos + 2)
    if height != 32 or not 0 < width <= 8000:
        raise ValueError("unsupported raster geometry")
    start = pos + 12
    packed = body[start:start + 4 * width]
    suffix = body[start + 4 * width:]
    if len(packed) != 4 * width or suffix not in (
        bytes.fromhex("1b 45 1b 41 1b 51"),
        bytes.fromhex("1b 70 30 1b 41 1b 51"),
    ):
        raise ValueError("raster size or command suffix mismatch")
    # S1 Canvas and S2 encodeBitmap specify reversed byte order within
    # each four-byte column, with MSB-first rows inside each byte.
    columns = ["".join(f"{b:08b}" for b in packed[x:x + 4][::-1])
               for x in range(0, len(packed), 4)]
    rows = ["".join(column[y] for column in columns) for y in range(32)]
    points = [(x, y) for y, row in enumerate(rows)
              for x, pixel in enumerate(row) if pixel == "1"]
    box = None
    if points:
        xs, ys = zip(*points)
        box = [min(xs), min(ys), max(xs), max(ys)]
    result = {
        "capture_sha256": sha(data),
        "write_lengths": [len(w) for w in writes],
        "header_hex": header.hex(" "),
        "body_length": len(body),
        "body_sha256": sha(body),
        "esc_d_offset": pos,
        "esc_d_hex": body[pos:start].hex(" "),
        "mode_bpp_byte": mode,
        "alignment": alignment,
        "width": width,
        "height": height,
        "copies": copies,
        "raster_length": len(packed),
        "raster_sha256": sha(packed),
        "suffix_hex": suffix.hex(" "),
        "ink_bbox_inclusive": box,
        "ink_occupied_size": [box[2] - box[0] + 1, box[3] - box[1] + 1] if box else None,
        "ink_center_span": [box[2] - box[0], box[3] - box[1]] if box else None,
        "black_pixels": len(points),
        "ink_count_per_row": [row.count("1") for row in rows],
    }
    return result, rows, body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("captures", nargs="+", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--expect-width", type=int)
    parser.add_argument("--expect-ink-size", nargs=2, type=int)
    args = parser.parse_args()
    results = {}
    if args.output:
        args.output.mkdir(parents=True, exist_ok=True)
    for path in args.captures:
        result, rows, body = decode(path.read_bytes())
        results[str(path)] = result
        if args.expect_width is not None and result["width"] != args.expect_width:
            parser.error(f"{path}: expected width {args.expect_width}, got {result['width']}")
        if args.expect_ink_size is not None and result["ink_occupied_size"] != args.expect_ink_size:
            parser.error(f"{path}: expected ink size {args.expect_ink_size}, got {result['ink_occupied_size']}")
        if args.output:
            stem = path.stem
            # Plain PBM is inspectable without an imaging library.
            (args.output / (stem + ".pbm")).write_text(
                f"P1\n{result['width']} 32\n" +
                "\n".join(" ".join(row) for row in rows) + "\n")
            (args.output / (stem + ".body.bin")).write_bytes(body)
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
