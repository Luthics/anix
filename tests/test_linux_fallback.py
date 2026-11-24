"""Linux RFC checks using syscall faults confined to the test-owned process."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest


class KernelFallback(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="agent-kernel-rfc-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.config = self.root / "config/agent"
        self.config.mkdir(parents=True)
        self.config.joinpath("config.toml").write_text(
            "[defaults]\nprovider='fixture'\nmodel='fixture'\n[providers.fixture]\nbin='mock'\n")
        mock = self.root / "mock"
        mock.write_text("#!/bin/sh\n/bin/cat > request\nprintf '%s\\n' "
                        """'{"type":"text_delta","text":"ok"}' '{"type":"done","finish_reason":"stop"}'\n""")
        mock.chmod(0o700)
        (self.root / "case.agent").write_text("local fixture")
        self.binary = os.environ.get("AGENT_BIN", Path(__file__).resolve().parents[1] / "target/debug/agent")
        self.trace = self.root / "probe.trace"
        self.process = dict(cwd=self.root, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, env={"HOME": tmp.name, "PATH": f"{tmp.name}:/usr/bin:/bin",
                            "XDG_CONFIG_HOME": str(self.root / "config"), "XDG_RUNTIME_DIR": str(self.root / "run")})

    def command(self, syscalls, fault, *args):
        return ["strace", "-f", "-qq", "-o", str(self.trace), "-e", f"trace={syscalls}",
                "-e", f"inject={syscalls}:{fault}", str(self.binary), *args]

    # RFC §10.2
    def test_unavailable_landlock_warns_or_fails_before_inference(self):
        for policy, status in (("strict", 1), ("best-effort", 0)):
            with self.subTest(policy=policy):
                out = subprocess.run(self.command("landlock_create_ruleset", "error=ENOSYS",
                                     f"--sandbox={policy}", "-q", "case.agent", "--", "input"),
                                     timeout=25, **self.process)
                self.assertRegex(self.trace.read_text(), r"ENOSYS.*INJECTED")
                self.assertEqual(out.returncode, status, out.stderr)
                self.assertTrue(out.stderr.strip())
                self.assertEqual(out.stdout, b"")
                self.assertEqual((self.root / "request").exists(), policy == "best-effort")

    # RFC §10.3
    def test_doctor_detects_broken_pipes_and_ineffective_or_unavailable_flock(self):
        for name in ("bin/agent", "libexec/agent/msh", "libexec/agent/agent-provider-openai-responses",
                     "libexec/agent/tools/aview", "libexec/agent/tools/aclr"):
            path = self.root / ".local" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("#!/bin/sh\nexit 0\n")
            path.chmod(0o700)
        self.config.joinpath("config.toml").write_text(
            "[providers.openai-responses]\napi_key_env='LOCAL_KEY'\n")
        self.process["env"]["LOCAL_KEY"] = "offline-doctor-fixture"
        baseline = subprocess.run([self.binary, "--doctor"], timeout=8, **self.process)
        self.assertEqual(baseline.returncode, 0, baseline.stderr)
        for syscalls, fault, affected, remedy in [
            ("?pipe,pipe2", "error=EMFILE", rb"pipe|/dev/fd", rb"/dev/fd|mount"),
            ("flock", "error=ENOSYS", rb"flock", rb"XDG_RUNTIME_DIR|filesystem"),
            # An implementation that only obtains the first lock would miss a
            # filesystem that claims to grant both independent exclusive locks.
            ("flock", "retval=0", rb"flock", rb"XDG_RUNTIME_DIR|filesystem"),
        ]:
            with self.subTest(syscalls=syscalls, fault=fault):
                run = subprocess.run(self.command(syscalls, fault, "--doctor"), timeout=8, **self.process)
                self.assertIn("INJECTED", self.trace.read_text())
                self.assertEqual(run.returncode, 1, (run.stdout, run.stderr))
                self.assertRegex(run.stderr, affected)
                self.assertRegex(run.stderr, remedy)

    # RFC §4.2
    def test_inbox_polls_new_publications_when_kernel_notifications_are_unavailable(self):
        cmd = self.command("?inotify_init,inotify_init1", "error=ENOSYS",
                           "-q", "--inbox", "mail", "--idle-timeout", "2", "-c", "history", "case.agent")
        with subprocess.Popen(cmd, **self.process) as child:
            try:
                deadline = time.monotonic() + 8
                while not (self.trace.exists() and "INJECTED" in self.trace.read_text()):
                    self.assertIsNone(child.poll(), "agent exited before watcher setup")
                    self.assertLess(time.monotonic(), deadline, "no inotify setup observed")
                    time.sleep(.01)
                self.assertIn("ENOSYS", self.trace.read_text())
                name = "100-23-abc"
                body = "complete after watcher setup"
                pending = self.root / "mail/tmp" / name
                pending.write_text(body)
                pending.rename(self.root / "mail/new" / name)
                stdout, stderr = child.communicate(timeout=12)
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
        self.assertEqual(child.returncode, 0, stderr)
        self.assertEqual(stdout, b"")
        self.assertEqual((self.root / "mail/done" / name).read_text(), body)
        for path in ["request", "history"]:
            rows = [json.loads(line) for line in (self.root / path).read_text().splitlines()]
            inputs = [r["text"] for r in rows if r.get("role") == "user"]
            self.assertEqual(len(inputs), 1)
            self.assertRegex(inputs[0], rf'^<input source="{name}">\n?{body}\n?</input>$')


if __name__ == "__main__":
    unittest.main()
