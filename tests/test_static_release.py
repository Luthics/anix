"""Inspect the release ELF files produced by the x86_64 musl CI job."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


def verify_static_elf(path):
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError(f"{path}: expected an executable file")
    result = subprocess.run(["readelf", "--wide", "--file-header", "--program-headers", "--dynamic", path],
                            capture_output=True, text=True, env={**os.environ, "LC_ALL": "C"}, check=False)
    if result.returncode or result.stderr:
        raise ValueError(f"{path}: readelf could not validate the ELF: {result.stderr}")
    for header, pattern in [("Class", "ELF64"), ("Type", r"(?:EXEC|DYN)\b.*"),
                            ("Machine", "Advanced Micro Devices X86-64")]:
        if not re.search(rf"^\s*{header}:\s+{pattern}$", result.stdout, re.MULTILINE):
            raise ValueError(f"{path}: unexpected ELF {header}")
    if not re.search(r"^\s*LOAD\s", result.stdout, re.MULTILINE):
        raise ValueError(f"{path}: ELF has no loadable segment")
    if re.search(r"^\s*INTERP\s", result.stdout, re.MULTILINE):
        raise ValueError(f"{path}: ELF requests an interpreter")
    if "(NEEDED)" in result.stdout:
        raise ValueError(f"{path}: ELF needs a shared library")


@unittest.skipUnless("ANIX_STATIC_BIN_DIR" in os.environ, "requires the x86_64 musl release artifacts")
class StaticRelease(unittest.TestCase):
    # RFC §2-05 — 核对五个发行程序均是可执行的 x86_64 ELF，并逐个排除动态解释器和共享库依赖。
    def test_every_musl_release_executable_has_no_interpreter_or_needed_libraries(self):
        with tempfile.TemporaryDirectory() as folder:
            invalid = Path(folder) / "invalid"
            invalid.write_text("#!/bin/sh\nexit 0\n")
            invalid.chmod(0o700)
            with self.assertRaisesRegex(ValueError, "could not validate the ELF"):
                verify_static_elf(invalid)
            # An interpreter without libc, then a shared object with libc and no interpreter.
            # These fixtures exercise the program-header and dynamic-table checks independently.
            subprocess.run(["cc", "-nostdlib", "-pie", "-Wl,-e,main", "-x", "c", "-", "-o", invalid],
                           input=b"int main(void) { return 0; }", check=True)
            with self.assertRaisesRegex(ValueError, "requests an interpreter"):
                verify_static_elf(invalid)
            subprocess.run(["cc", "-shared", "-fPIC", "-x", "c", "-", "-o", invalid],
                           input=b'#include <stdio.h>\nvoid fixture(void) { puts("hello"); }', check=True)
            with self.assertRaisesRegex(ValueError, "needs a shared library"):
                verify_static_elf(invalid)
        for name in ("anix", "msh", "anix-provider", "aview", "aclr"):
            with self.subTest(binary=name):
                verify_static_elf(Path(os.environ["ANIX_STATIC_BIN_DIR"]) / name)
