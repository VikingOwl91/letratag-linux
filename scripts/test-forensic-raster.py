import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location("forensic", Path(__file__).with_name("forensic-raster.py"))
forensic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(forensic)


def fixture(modern=False):
    # Two asymmetric columns, drawn by hand: (0,0), (0,9), (1,30).
    packed = bytes.fromhex("00 00 40 80 02 00 00 00")
    body = bytes.fromhex("1b 73 9a 02 00 00")
    if modern:
        body += bytes.fromhex("1b 23 01")
    body += bytes.fromhex("1b 44") + bytes([0x81 if modern else 1, 2])
    body += struct.pack("<II", 2, 32) + packed
    body += bytes.fromhex("1b 70 30" if modern else "1b 45") + bytes.fromhex("1b 41 1b 51")
    header = bytes.fromhex("ff f0 12 34") + struct.pack("<I", len(body))
    header += bytes([sum(header) % 256])
    writes = [header, b"\x00" + body[:15], b"\x01" + body[15:] + bytes.fromhex("12 34")]
    return b"".join(struct.pack("<H", len(w)) + w for w in writes)


class DecoderTests(unittest.TestCase):
    def test_asymmetric_known_pixels_both_profiles(self):
        for modern in (False, True):
            with self.subTest(modern=modern):
                result, rows, _ = forensic.decode(fixture(modern))
                points = {(x, y) for y, row in enumerate(rows) for x, v in enumerate(row) if v == "1"}
                self.assertEqual(points, {(0, 0), (0, 9), (1, 30)})
                self.assertEqual(result["raster_length"], 8)
                self.assertEqual(result["esc_d_offset"], 9 if modern else 6)

    def test_corrupt_header(self):
        data = bytearray(fixture())
        data[10] ^= 1
        with self.assertRaisesRegex(ValueError, "checksum"):
            forensic.decode(data)

    def test_truncation(self):
        with self.assertRaisesRegex(ValueError, "truncated"):
            forensic.decode(fixture()[:-1])

    def test_bad_chunk_index(self):
        data = bytearray(fixture())
        data[13] = 4
        with self.assertRaisesRegex(ValueError, "sequence"):
            forensic.decode(data)

    def test_preserved_calibration_evidence(self):
        root = Path(__file__).resolve().parents[1] / "docs/evidence/geometry-2026-10-21"
        direct315, rows315, body315 = forensic.decode((root / "direct-315.ble").read_bytes())
        direct400, rows400, body400 = forensic.decode((root / "direct-400.ble").read_bytes())
        cups400, rows_cups, _ = forensic.decode((root / "cups-400-reproduction.ble").read_bytes())
        self.assertEqual([direct315["width"], direct400["width"], cups400["width"]], [315, 400, 315])
        self.assertEqual(body315[21:1281], body400[21:1281])
        self.assertEqual(body400[1281:1621], bytes(340))
        self.assertEqual([row[:315] for row in rows400], rows315)
        self.assertEqual(direct315["ink_bbox_inclusive"], [10, 1, 262, 30])
        self.assertEqual(cups400["ink_bbox_inclusive"], [7, 4, 206, 27])
        self.assertEqual(rows315[1][10:262], "1" * 252)
        self.assertEqual(rows_cups[4][8:207], "1" * 199)
        # Equal commands/frame sizes do not imply equal raster contents.
        self.assertEqual(direct315["esc_d_hex"], cups400["esc_d_hex"])
        self.assertNotEqual(direct315["raster_sha256"], cups400["raster_sha256"])
        pdf315 = (root / "calibration-315.pdf").read_bytes()
        pdf400 = (root / "calibration-400.pdf").read_bytes()
        stream = lambda pdf: pdf.split(b"stream\n", 1)[1].split(b"endstream", 1)[0]
        self.assertEqual(stream(pdf315), stream(pdf400))


if __name__ == "__main__":
    unittest.main()
