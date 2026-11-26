"""Isolated CLI with a local oneshot Provider and private config, WAL and runtime files."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class OfflineAnixCase(unittest.TestCase):
    def setUp(self):
        area = tempfile.TemporaryDirectory(prefix="anix-rfc-tables-")
        self.addCleanup(area.cleanup)
        self.root = Path(area.name)
        bins = Path(os.environ.get("ANIX_TEST_BIN_DIR", ROOT / "target/debug")).resolve()
        self.anix = bins / "anix"
        self.env = {"HOME": str(self.root), "PATH": f"{bins}:/usr/bin:/bin",
                    "XDG_RUNTIME_DIR": str(self.root / "run"), "LC_ALL": "C"}
        config = self.root / ".config/anix"
        config.mkdir(parents=True)
        self.provider = self.root / "provider"
        self.config = config / "config.toml"
        self.config.write_text(
            f'[defaults]\nprovider="fixture"\nmodel="local"\n[providers.fixture]\n'
            f'bin="{self.provider}"\nmode="oneshot"\n')
        self.script = self.root / "case.agent"
        self.script.write_text("reply")
        self.reply([{"type": "text_delta", "text": "OK"}, {"type": "done", "finish_reason": "stop"}])

    def reply(self, events):
        (self.root / "events").write_text("".join(json.dumps(event) + "\n" for event in events))
        self.provider.write_text("#!/bin/sh\ncat > request\ncat events\n")
        self.provider.chmod(0o700)

    def invoke(self, *args):
        return subprocess.run([self.anix, *args], cwd=self.root, env=self.env,
                              stdin=subprocess.DEVNULL, capture_output=True,
                              start_new_session=True, timeout=10)
