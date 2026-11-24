"""RFC §10.2. Simulate ENOSYS for the Landlock probe in a test-owned process."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class KernelFallback(unittest.TestCase):
    # RFC §10.2
    def test_unavailable_landlock_warns_or_fails_before_inference(self):
        binary = os.environ.get("AGENT_BIN", Path(__file__).resolve().parents[1] / "target/debug/agent")
        with tempfile.TemporaryDirectory(prefix="agent-no-landlock-") as tmp:
            root = Path(tmp)
            config = root / "config/agent"
            config.mkdir(parents=True)
            config.joinpath("config.toml").write_text(
                "[defaults]\nprovider='fixture'\nmodel='fixture'\n[providers.fixture]\nbin='mock'\n")
            mock = root / "mock"
            mock.write_text("#!/bin/sh\n/bin/cat > request\nprintf '%s\\n' "
                            """'{"type":"text_delta","text":"ok"}' '{"type":"done","finish_reason":"stop"}'\n""")
            mock.chmod(0o700)
            (root / "case.agent").write_text("local fixture")
            for policy, status in (("strict", 1), ("best-effort", 0)):
                with self.subTest(policy=policy):
                    trace = root / "probe.trace"
                    out = subprocess.run(
                        ["strace", "-f", "-qq", "-o", str(trace), "-e", "trace=landlock_create_ruleset",
                         "-e", "inject=landlock_create_ruleset:error=ENOSYS", str(binary),
                         f"--sandbox={policy}", "-q", "case.agent", "--", "input"],
                        cwd=tmp, stdin=subprocess.DEVNULL, capture_output=True, timeout=25,
                        env={"HOME": tmp, "PATH": f"{tmp}:/usr/bin:/bin",
                             "XDG_CONFIG_HOME": str(root / "config"), "XDG_RUNTIME_DIR": str(root / "run")})
                    self.assertIn("ENOSYS", trace.read_text())
                    self.assertIn("INJECTED", trace.read_text())
                    self.assertEqual(out.returncode, status, out.stderr)
                    self.assertTrue(out.stderr.strip())
                    self.assertEqual(out.stdout, b"")
                    self.assertEqual((root / "request").exists(), policy == "best-effort")


if __name__ == "__main__":
    unittest.main()
