"""Linux RFC checks using I/O accounting and faults confined to test processes."""
import json
import os
from pathlib import Path
import re
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

    # RFC §6.2, §9.1
    def test_archived_wal_growth_does_not_require_reading_the_archive_on_resume(self):
        # Sample the core's actual read-byte counter while it waits on the mock.
        # Logical reads remain visible even when the filesystem cache is warm.
        mock = self.root / "mock"
        script = mock.read_text().replace("/bin/cat > request", '/bin/cat /proc/"$PPID"/io > reads\n/bin/cat > request')
        mock.write_text(script)
        ts = "2026-01-02T03:04:05Z"
        cfg = {"stderr": "explicit", "approval": "ask"}
        init = {"type": "session_init", "ir_version": 1, "ts": ts, "config": cfg}
        checkpoint = {"type": "checkpoint", "ir_version": 1, "turn_id": 3, "ts": ts, "config": cfg}

        def message(role, tid, text):
            return {"type": "message", "role": role, "turn_id": tid, "ts": ts, "text": text}

        active = [checkpoint, message("system", 3, "active prefix"), message("user", 4, "<checkpoint>\ns\n</checkpoint>")]
        counts = []
        requests = []
        sizes = (1 << 20, 32 << 20)
        chunk = b"x" * (1 << 16)
        for size in sizes:
            path = self.root / f"history-{size}"
            with path.open("wb") as f:
                f.write(json.dumps(init).encode() + b"\n")
                # Write a valid, large archived message in bounded memory.
                row = json.dumps(message("system", 0, "ARCHIVE")).encode()
                before, after = row.split(b"ARCHIVE")
                f.write(before)
                for _ in range(size // len(chunk)):
                    f.write(chunk)
                f.write(after + b"\n")
                for record in [message("user", 1, "archived input"), message("assistant", 2, "archived answer"), *active]:
                    f.write(json.dumps(record).encode() + b"\n")
            out = subprocess.run([self.binary, "-q", "-c", path, "case.agent", "--", "resume"],
                                 timeout=20, **self.process)
            self.assertEqual(out.returncode, 0, out.stderr)
            stats = dict(line.split(": ") for line in (self.root / "reads").read_text().splitlines())
            counts.append(int(stats["rchar"]))
            request = [json.loads(line) for line in (self.root / "request").read_bytes().splitlines()]
            self.assertEqual([(r["role"], r["text"]) for r in request[1:]],
                             [("system", "active prefix"), ("user", "<checkpoint>\ns\n</checkpoint>"),
                              ("user", "<args>resume</args>")])
            requests.append(request)
        self.assertEqual(requests[0], requests[1])
        # Detect a scan proportional to the extra 31 MiB without prescribing
        # a particular reverse-reader block size or a wall-clock limit.
        self.assertLess(counts[1] - counts[0], (sizes[1] - sizes[0]) // 2, counts)

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

    def prepare_doctor(self):
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

    # RFC §10.3
    def test_doctor_checks_missing_msh_before_diagnosing_invalid_toml(self):
        self.prepare_doctor()
        missing = self.root / ".local/libexec/agent/msh"
        missing.unlink()
        self.config.joinpath("config.toml").write_text("[broken-table\n")
        result = subprocess.run(["strace", "-f", "-qq", "-s", "512", "-e", "trace=%file,write,writev",
                                 "-o", str(self.trace), self.binary, "--doctor"], timeout=10, **self.process)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(b"msh", result.stderr)
        lines = self.trace.read_text().splitlines()
        # Compare the filesystem observation with the first *published* config
        # failure. This permits preliminary config reads, buffered diagnostics,
        # and returning immediately after the first failed check.
        probes = [i for i, line in enumerate(lines) if 'msh"' in line and "ENOENT" in line]
        self.assertTrue(probes, "no filesystem probe of the missing msh")
        for i, line in enumerate(lines):
            if ("write(2," in line or "writev(2," in line) and any(
                    word in line.lower() for word in ("config", "toml", "broken-table")):
                self.assertLess(probes[0], i, f"configuration failure preceded msh check:\n{line}\n{lines[probes[0]]}")
                break

    # RFC §10.3
    def test_doctor_detects_broken_pipes_and_ineffective_or_unavailable_flock(self):
        self.prepare_doctor()
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
    def test_idle_inbox_polls_the_empty_new_directory_at_200_ms_without_a_watcher(self):
        watcher = "?inotify_init,inotify_init1"
        result = subprocess.run(
            ["strace", "-f", "-qq", "-ttt", "-yy", "-o", str(self.trace),
             "-e", f"trace={watcher},getdents64,clock_nanosleep,nanosleep,ppoll,?poll",
             "-e", f"inject={watcher}:error=ENOSYS", self.binary, "--inbox", "mail",
             "--idle-timeout", "2", "-q", "case.agent"], timeout=12, **self.process)
        self.assertEqual(result.returncode, 0, result.stderr)
        trace = self.trace.read_text()
        self.assertIn("ENOSYS", trace)
        self.assertIn("INJECTED", trace)
        self.assertFalse((self.root / "request").exists())
        # Inspect full idle intervals between actual empty directory reads.
        # Exclude the last interval, which can be shortened at the idle deadline.
        scan = re.compile(r"(\d+\.\d+) getdents64\(\d+<[^>]+/mail/new>,.*\)\s+= 0$")
        scans = [(i, float(match[1])) for i, line in enumerate(trace.splitlines())
                 if (match := scan.search(line))]
        self.assertGreaterEqual(len(scans), 4, trace)
        lines = trace.splitlines()
        for (before, start), (after, end) in zip(scans, scans[1:-1]):
            waits = "\n".join(lines[before + 1:after])
            # Read the nominal timeout when visible, independent of scheduling
            # latency. Use observed scans for alternate wait mechanisms.
            relative = re.search(r"(?:clock_nanosleep\([^,]+, 0, |nanosleep\()"
                                 r"\{tv_sec=0, tv_nsec=(\d+)\}", waits)
            if relative:
                self.assertEqual(int(relative[1]), 200_000_000, waits)
            else:
                self.assertGreaterEqual(end - start, .18, waits)
                self.assertLess(end - start, .40, waits)

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
