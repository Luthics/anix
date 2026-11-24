"""RFC §7.2. Render a four-page local PDF through aview's public fd 6 interface.

Run cargo build -p agent-tools, then python3 tests/test_pdf_pages.py. Install
Poppler's pdftoppm, mutool, or Ghostscript. Set AVIEW_BIN for another target.
"""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest


def page_shapes_pdf():
    # The three selected pages have unique aspect ratios. A renderer returning
    # page one repeatedly or including the excluded third page is detectable.
    sizes = [(72, 72), (144, 72), (216, 72), (72, 144)]
    kids = " ".join(f"{3 + i} 0 R" for i in range(len(sizes)))
    objects = ["<< /Type /Catalog /Pages 2 0 R >>",
               f"<< /Type /Pages /Count 4 /Kids [{kids}] >>"]
    for width, height in sizes:
        objects.append(f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] "
                       "/Resources << >> /Contents 7 0 R >>")
    objects.append("<< /Length 0 >>\nstream\nendstream")
    buf = bytearray(b"%PDF-1.4\n")
    offsets = []
    for number, obj in enumerate(objects, 1):
        offsets.append(len(buf))
        buf.extend(f"{number} 0 obj\n{obj}\nendobj\n".encode())
    xref = len(buf)
    buf.extend(f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode())
    for offset in offsets:
        buf.extend(f"{offset:010} 00000 n \n".encode())
    buf.extend(f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\n"
               f"startxref\n{xref}\n%%EOF\n".encode())
    return buf


class PdfPages(unittest.TestCase):
    def test_single_file_and_stdin_render_the_selected_pages(self):
        self.assertTrue(any(shutil.which(name) for name in ("pdftoppm", "mutool", "gs")),
                        "install poppler-utils, mutool or Ghostscript for this suite")
        binary = os.environ.get("AVIEW_BIN", Path(__file__).resolve().parents[1] / "target/debug/aview")
        with tempfile.TemporaryDirectory(prefix="agent-pdf-") as tmp:
            path = Path(tmp) / "page shapes.pdf"
            data = page_shapes_pdf()
            path.write_bytes(data)
            for source in (str(path), "-"):
                with self.subTest(source=source):
                    proc = subprocess.run(
                        ["/bin/sh", "-c", 'exec 6>&1; exec "$@" >/dev/null', "pdf-test",
                         str(binary), "--max-dim", "48", "--pages", "1-2,4", source],
                        input=data if source == "-" else b"", cwd=tmp, capture_output=True,
                        env={"PATH": os.environ["PATH"], "HOME": tmp, "AGENT_CTL": "/dev/fd/6"},
                        timeout=45)
                    rows = [json.loads(line) for line in proc.stdout.splitlines()]
                    paths = [Path(frame["path"]) for frame in rows]
                    try:
                        self.assertEqual(proc.returncode, 0, proc.stderr)
                        self.assertEqual(len(rows), 3)
                        self.assertEqual(len(set(paths)), 3)
                        for frame, output, size in zip(rows, paths, [(48, 48), (48, 24), (24, 48)]):
                            self.assertEqual((frame["op"], frame["mime"]), ("media", "image/png"))
                            self.assertTrue(output.is_absolute())
                            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
                            image = output.read_bytes()
                            self.assertTrue(image.startswith(b"\x89PNG\r\n\x1a\n"))
                            self.assertEqual(image[12:16], b"IHDR")
                            self.assertEqual(struct.unpack(">II", image[16:24]), size)
                    finally:
                        for output in paths:
                            if output.parent == Path("/tmp") and output.name.startswith("aview_"):
                                output.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
