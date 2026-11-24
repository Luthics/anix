"""POSIX conformance through the real agent and msh, with a local provider.

Build the workspace binaries first. OS_AGENT_BIN_DIR can select another build.
"""
from contextlib import ExitStack
from datetime import datetime
import array
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import resource
import select
import signal
import socket
import subprocess
import sys
import tempfile
import termios
import time
import tty
import unittest


class ShellPipeline(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="agent-shell-rfc-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        binaries = Path(os.environ.get("OS_AGENT_BIN_DIR", Path(__file__).resolve().parents[1] / "target/debug"))
        self.agent = binaries / "agent"
        self.aclr = binaries / "aclr"
        self.libexec = self.root / ".local/libexec/agent"
        self.libexec.mkdir(parents=True)
        (self.libexec / "msh").symlink_to(binaries / "msh")
        config = self.root / "config/agent"
        config.mkdir(parents=True)
        (config / "config.toml").write_text(
            "[defaults]\nprovider='fixture'\nmodel='local'\n[providers.fixture]\nbin='local-provider'\n")
        self.provider = self.libexec / "local-provider"
        self.provider.write_text("""#!/bin/sh
set -eu
if [ -e first.ir ]; then
    /bin/cat > second.ir
    printf '%s\\n' '{"type":"text_delta","text":"model finished"}' '{"type":"done","finish_reason":"stop"}'
else
    /bin/cat > first.ir
    /bin/cat first.reply
fi
""")
        self.provider.chmod(0o700)
        (self.root / "case.agent").write_text("tool fixture")
        (self.root / "trace").touch(mode=0o600)
        self.env = {"HOME": str(self.root), "PATH": "/bin:/usr/bin", "AGENT_TRACE_LEVEL": "wire",
                    "XDG_CONFIG_HOME": str(self.root / "config"), "XDG_RUNTIME_DIR": str(self.root / "run")}

    def invoke(self, script, extra_env=None, shell=None, approval_opts=("-y",)):
        events = [{"type": "tool_call", "id": "fixture-id", "name": "msh", "arguments": {"cmd": script}},
                  {"type": "done", "finish_reason": "tool_calls"}]
        (self.root / "first.reply").write_text("".join(json.dumps(e) + "\n" for e in events))
        args = [str(self.agent), "-q", *approval_opts]
        if shell:
            args += ["--shell", shell]
        args += ["case.agent", "--", "go"]
        result = subprocess.run(["/bin/sh", "-c", 'exec "$@" 3>trace', "fixture", *args],
                                cwd=self.root, env={**self.env, **(extra_env or {})},
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b"")
        reply = json.loads((self.root / "second.ir").read_text().splitlines()[-1])
        self.assertEqual((reply["role"], reply["tool_call_id"]), ("tool", "fixture-id"))
        return result, reply

    # RFC §7.2, §6.1, §6.2
    def test_aclr_and_agent_agree_on_the_serialized_fold_frame_size_limit(self):
        def emit(summary):
            return subprocess.run(["/bin/sh", "-c", 'exec 6>&1; exec "$@" >/dev/null', "fixture",
                                   self.aclr, "-t", "1", summary], cwd=self.root,
                                  env={**self.env, "AGENT_CTL": "/dev/fd/6"},
                                  stdin=subprocess.DEVNULL, capture_output=True, timeout=8)

        # Start with the producer's own serialization, including exactly one LF.
        template = emit("PLACEHOLDER")
        self.assertEqual(template.returncode, 0, template.stderr)
        self.assertEqual(json.loads(template.stdout), {"op": "fold", "from_turn": 1, "summary": "PLACEHOLDER"})
        self.assertEqual(template.stdout.count(b"PLACEHOLDER"), 1)
        self.assertTrue(template.stdout.endswith(b"\n"))
        cmd = 'cat ctl.frame > "$AGENT_CTL"'
        events = [{"type": "tool_call", "id": "boundary", "name": "msh", "arguments": {"cmd": cmd}},
                  {"type": "done", "finish_reason": "tool_calls"}]
        (self.root / "first.reply").write_text("".join(json.dumps(e) + "\n" for e in events))
        for size in (63_999, 64_000, 64_001, 65_535, 65_536, 65_537, 65_538):
            with self.subTest(bytes_including_lf=size):
                summary = "x" * (size - len(template.stdout) + len("PLACEHOLDER"))
                frame = template.stdout.replace(b"PLACEHOLDER", summary.encode())
                producer = emit(summary)
                accepted = producer.returncode == 0
                self.assertEqual(len(frame), size)
                self.assertTrue(producer.stdout == (frame if accepted else b""), "unexpected fd 6 bytes")
                if size <= 64_000:
                    self.assertTrue(accepted)
                if size >= 65_538:
                    self.assertFalse(accepted)

                # Write the same bytes independently so that rejection by aclr
                # cannot short-circuit the check of the agent's receiving side.
                (self.root / "ctl.frame").write_bytes(frame)
                (self.root / "first.ir").unlink(missing_ok=True)
                (self.root / "second.ir").unlink(missing_ok=True)
                wal = self.root / f"{size}.wal"
                run = subprocess.run([self.agent, "-q", "-y", "-c", wal, "case.agent", "--", "go"],
                                     cwd=self.root, env=self.env, stdin=subprocess.DEVNULL,
                                     capture_output=True, timeout=10)
                rows = [json.loads(line) for line in wal.read_bytes().splitlines()]
                folds = [r for r in rows if r.get("type") == "fold"]
                self.assertEqual(len(folds), int(accepted),
                                 f"frame of {size} bytes including LF: aclr accepted={accepted}, WAL folds={len(folds)}")
                if accepted:
                    self.assertEqual(run.returncode, 0, run.stderr)
                    self.assertTrue(folds[0]["summary"] == summary)
                    actual = [json.loads(line) for line in (self.root / "second.ir").read_bytes().splitlines()]
                    self.assertTrue(any(summary in row.get("text", "") for row in actual))

    # RFC §3.3, §9.5
    def test_trace_envelope_identifies_the_live_process_and_effective_agent_name(self):
        (self.root / "first.reply").write_text(
            '{"type":"text_delta","text":"ok"}\n{"type":"done","finish_reason":"stop"}\n')
        for source, name in [("plain", "filename"), ("---\nname: named.agent-2\n---\nbody", "named.agent-2")]:
            with self.subTest(name=name):
                (self.root / "filename.agent").write_text(source)
                (self.root / "first.ir").unlink(missing_ok=True)
                before = time.time()
                with subprocess.Popen(["sh", "-c", 'exec "$@" 3>trace', "fixture",
                                       self.agent, "-q", "filename.agent", "--", "go"],
                                      cwd=self.root, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE, env={**self.env, "AGENT_TRACE_LEVEL": "events"}) as child:
                    try:
                        out, err = child.communicate(timeout=15)
                    finally:
                        if child.poll() is None:
                            child.kill()
                            child.communicate()
                after = time.time()
                self.assertEqual((child.returncode, out, err), (0, b"", b""))
                events = [json.loads(row) for row in (self.root / "trace").read_bytes().splitlines()]
                self.assertEqual(events[0]["type"], "run_start")
                self.assertEqual(events[-1]["type"], "run_end")
                for event in events:
                    self.assertEqual((event["v"], event["agent"], event["pid"], event["ppid"]),
                                     (1, name, child.pid, os.getpid()))
                    stamp = datetime.fromisoformat(event["ts"].replace("Z", "+00:00"))
                    self.assertIsNotNone(stamp.tzinfo)
                    self.assertLessEqual(before - 1, stamp.timestamp())
                    self.assertLessEqual(stamp.timestamp(), after + 1)

    # RFC §6.2, §6.4
    def test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state(self):
        # Use a real kernel-enforced short write or EFBIG. Limits apply only
        # to the disposable agent process, and never to the test runner.
        (self.root / "first.reply").write_text(
            '{"type":"text_delta","text":"ok"}\n{"type":"done","finish_reason":"stop"}\n')

        def run(wal, message, limit=None):
            def limit_file_growth():
                signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
                resource.setrlimit(resource.RLIMIT_FSIZE, (limit, limit))

            return subprocess.run([self.agent, "-q", "-c", wal, "case.agent", "--", message],
                                  cwd=self.root, env=self.env, stdin=subprocess.DEVNULL,
                                  capture_output=True, timeout=15,
                                  preexec_fn=None if limit is None else limit_file_growth)

        seed = self.root / "seed.wal"
        first = run(seed, "seed")
        self.assertEqual(first.returncode, 0, first.stderr)
        prefix = seed.read_bytes()
        self.assertTrue(prefix.endswith(b"\n"))
        for room in (0, 1, 7, 63):
            with self.subTest(extra_writable_bytes=room):
                wal = self.root / f"limited-{room}.wal"
                wal.write_bytes(prefix)
                failed = run(wal, "cannot-commit-" + "f" * 100, len(prefix) + room)
                self.assertEqual(failed.returncode, 1, failed.stderr)
                self.assertTrue(failed.stderr.strip())
                after = wal.read_bytes()
                self.assertEqual(after[:len(prefix)], prefix)
                self.assertLessEqual(len(after), len(prefix) + room)
                resumed = run(wal, "resume")
                if after == prefix:
                    self.assertEqual(resumed.returncode, 0, resumed.stderr)
                    rows = [json.loads(l) for l in wal.read_bytes().splitlines()]
                    self.assertEqual([r["text"] for r in rows if r.get("role") == "user"],
                                     ["<args>seed</args>", "<args>resume</args>"])
                else:
                    self.assertFalse(after.endswith(b"\n"), "a full record fit in the fault window")
                    self.assertEqual(resumed.returncode, 1, resumed.stderr)
                    self.assertEqual(wal.read_bytes(), after)

    # RFC §9.4, §7.1
    def test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand(self):
        guide = self.root / '.skills/R&D "examples"/SKILL.md'
        guide.parent.mkdir(parents=True)
        guide.write_text("---\nname: report\ndescription: 'Compare A&B with <例>.'\n---\n"
                         "body-visible-only-after-cat\n")
        # This mock chooses its command from the received index, so a bad path
        # or an unescaped XML attribute breaks the actual tool round trip.
        self.provider.write_text('#!/bin/sh\nexec "$FIXTURE_PYTHON" "$HOME/choose.py"\n')
        (self.root / "choose.py").write_text('''import json, shlex, sys
from pathlib import Path
from xml.etree import ElementTree
rows = [json.loads(line) for line in sys.stdin]
if not Path("first.ir").exists():
    Path("first.ir").write_text(json.dumps(rows))
    text = next(r["text"] for r in rows if r.get("role") == "system")
    xml = text[text.index("<available_skills>"):text.index("</available_skills>")+len("</available_skills>")]
    skills = ElementTree.fromstring(xml).findall("skill")
    assert len(skills) == 1
    entry = skills[0]
    Path("index.json").write_text(json.dumps([entry.attrib, entry.text]))
    print(json.dumps({"type":"tool_call","id":"read","name":"msh",
                      "arguments":{"cmd":"cat " + shlex.quote(entry.get("path"))}}))
    print(json.dumps({"type":"done","finish_reason":"tool_calls"}))
else:
    Path("second.ir").write_text(json.dumps(rows))
    print(json.dumps({"type":"text_delta","text":"finished"}))
    print(json.dumps({"type":"done","finish_reason":"stop"}))
''')
        run = subprocess.run([self.agent, "-q", "-y", "case.agent", "--", "go"], cwd=self.root,
                             stdin=subprocess.DEVNULL, capture_output=True, timeout=15,
                             env={**self.env, "FIXTURE_PYTHON": sys.executable})
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(json.loads((self.root / "index.json").read_text()),
                         [{"name":"report", "path":str(guide.resolve())}, "Compare A&B with <例>."])
        first = json.loads((self.root / "first.ir").read_text())
        self.assertFalse(any("body-visible-only-after-cat" in r.get("text", "") for r in first))
        second = json.loads((self.root / "second.ir").read_text())
        tool = [r for r in second if r.get("role") == "tool"]
        self.assertEqual(len(tool), 1)
        self.assertEqual(tool[0]["tool_call_id"], "read")
        self.assertEqual(tool[0]["text"].split("\n", 1)[1].rstrip("\n"), guide.read_text().rstrip("\n"))

    # RFC §4.1, §9.1
    def test_open_input_descriptor_defers_inference_until_eof(self):
        (self.root / "first.reply").write_text(
            '{"type":"text_delta","text":"ok"}\n{"type":"done","finish_reason":"stop"}\n')
        for fd in (0, 4, 9):
            with self.subTest(fd=fd), ExitStack() as stack:
                request = self.root / "first.ir"
                request.unlink(missing_ok=True)
                read_fd, write_fd = os.pipe()
                reader = stack.enter_context(os.fdopen(read_fd, "rb", buffering=0))
                writer = stack.enter_context(os.fdopen(write_fd, "wb", buffering=0))
                prefix = "first-\x00\r\n\x7f\u0080\u07ff\u0800\ufffd\u2028\U00010000\U0010ffff"
                writer.write(prefix.encode("utf-8") + b"\xe4")
                redirect = "" if fd == 0 else f"exec {fd}<&0; exec </dev/null; "
                child = subprocess.Popen(["/bin/sh", "-c", redirect + 'exec "$@"', "fixture",
                                          self.agent, "-q", "case.agent"], stdin=reader, cwd=self.root,
                                         env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                try:
                    pending = array.array("i", [0])
                    deadline = time.monotonic() + 10
                    while True:
                        fcntl.ioctl(reader, termios.FIONREAD, pending, True)
                        if pending[0] == 0:
                            break
                        self.assertIsNone(child.poll())
                        self.assertLess(time.monotonic(), deadline, f"fd {fd} was never read")
                        time.sleep(.01)
                    self.assertFalse(request.exists(), "provider started with its input still open")
                    writer.write(b"\xb8\xad-last")
                    writer.close()
                    out, err = child.communicate(timeout=10)
                finally:
                    writer.close()
                    if child.poll() is None:
                        child.kill()
                        child.communicate()
                self.assertEqual((child.returncode, out, err), (0, b"", b""))
                rows = [json.loads(line) for line in request.read_bytes().split(b"\n") if line]
                users = [r["text"] for r in rows if r.get("role") == "user"]
                self.assertEqual(len(users), 1)
                expected = prefix + "中-last"
                if fd == 0:
                    self.assertEqual(users[0], expected)
                else:
                    self.assertRegex(users[0], rf'^<input fd="{fd}">\n?{re.escape(expected)}\n?</input>$')

    # RFC §4.1, §9.5
    def test_inference_can_finish_with_an_open_trace_consumer_that_stops_reading(self):
        (self.root / "first.reply").write_text(
            '{"type":"text_delta","text":"ok"}\n{"type":"done","finish_reason":"stop"}\n')
        with ExitStack() as stack:
            sender, receiver = socket.socketpair()
            stack.enter_context(sender)
            stack.enter_context(receiver)
            sender.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, 4096)
            sender.setblocking(False)
            # Fill the send queue down to one byte. The receiver stays connected
            # throughout normal inference, and never consumes the prefilled data.
            for chunk in (b"T" * 8192, b"T"):
                while True:
                    try:
                        sender.send(chunk)
                    except BlockingIOError:
                        break
            sender.setblocking(True)
            cmd = ["sh", "-c", 'exec 3<&0; exec "$@" </dev/null', "fixture",
                   self.agent, "-q", "case.agent", "--", "go"]
            with subprocess.Popen(cmd, cwd=self.root, stdin=sender, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, env={**self.env, "AGENT_TRACE_LEVEL": "events"}) as child:
                try:
                    out, err = child.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    receiver.close()
                    try:
                        child.communicate(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.communicate()
                    self.fail("inference did not finish while the full trace socket stayed connected")
            self.assertEqual((child.returncode, out, err), (0, b"", b""))
            self.assertTrue((self.root / "first.ir").exists())

    # RFC §7.1, §6.4
    def test_msh_timeout_signals_the_whole_group_and_grants_three_seconds_before_kill(self):
        # Both witnesses keep a FIFO writer open until they actually exit. EOF
        # observes the last descendant's exit, including after its parent dies.
        (self.root / "witness.py").write_text(
            "import os, signal, subprocess, sys, time\n"
            "from pathlib import Path\n"
            "role = sys.argv[1]\n"
            "fifo = open('alive', 'wb', buffering=0)\n"
            "def term(sig, frame):\n"
            "    Path(role + '.term').write_text(str(time.monotonic()))\n"
            "    time.sleep(2.7)\n"
            "    Path(role + '.grace').touch()\n"
            "signal.signal(signal.SIGTERM, term)\n"
            "Path(role + '.group').write_text(str(os.getpgrp()))\n"
            "if role == 'leader':\n"
            "    subprocess.Popen([sys.executable, __file__, 'worker'])\n"
            "time.sleep(14)\n")
        fifo = self.root / "alive"
        os.mkfifo(fifo)
        with os.fdopen(os.open(fifo, os.O_RDONLY | os.O_NONBLOCK), "rb", buffering=0) as reader:
            out = subprocess.run([self.libexec / "msh", "-c", 'exec "$WITNESS" witness.py leader'],
                                 cwd=self.root, stdin=subprocess.DEVNULL, capture_output=True, timeout=12,
                                 env={**self.env, "MSH_TIMEOUT_SEC": "2", "WITNESS": sys.executable})
            finished = time.monotonic()
            self.assertEqual(out.returncode, 124, out)
            self.assertRegex((out.stdout + out.stderr).lower(), rb"timeout|timed out")
            groups = [(self.root / (role + ".group")).read_text() for role in ("leader", "worker")]
            self.assertEqual(groups[0], groups[1])
            self.assertNotEqual(int(groups[0]), os.getpgrp())
            for role in ("leader", "worker"):
                notified = float((self.root / (role + ".term")).read_text())
                # Timestamp comes from the Python signal handler, shortly after delivery.
                self.assertGreaterEqual(finished - notified, 2.8, role)
                self.assertTrue((self.root / (role + ".grace")).exists(), role)
            self.assertTrue(select.select([reader], [], [], 1)[0], "a group member still holds the FIFO")
            self.assertEqual(reader.read(1), b"")

    # RFC §6.2, §6.3, §6.4, §9.1
    def test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history(self):
        (self.root / "wait_for_signal.py").write_text('''import signal, time
from pathlib import Path
def stop(signum, frame):
    Path("observed-signal").write_text(str(signum))
    raise SystemExit(0)
signal.signal(signal.SIGTERM, stop)
print("partial tool result", flush=True)
Path("tool-ready").touch()
time.sleep(12)
''')
        events = [{"type": "tool_call", "id": "active", "name": "msh",
                   "arguments": {"cmd": 'exec "$WITNESS" wait_for_signal.py'}},
                  {"type": "done", "finish_reason": "tool_calls"}]
        (self.root / "first.reply").write_text("".join(json.dumps(e) + "\n" for e in events))
        for sig, code in [(signal.SIGINT, 130), (signal.SIGTERM, 143)]:
            with self.subTest(signal=sig):
                for name in ("first.ir", "second.ir", "tool-ready", "observed-signal"):
                    (self.root / name).unlink(missing_ok=True)
                wal = self.root / (sig.name + ".wal")
                args = [self.agent, "-q", "-y", "-c", wal, "case.agent", "--"]
                with subprocess.Popen([*args, "before"], cwd=self.root, stdin=subprocess.DEVNULL,
                                      stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                      env={**self.env, "WITNESS": sys.executable}) as child:
                    try:
                        deadline = time.monotonic() + 8
                        while not (self.root / "tool-ready").exists():
                            self.assertIsNone(child.poll(), "agent exited before msh was ready")
                            self.assertLess(time.monotonic(), deadline, "msh never became ready")
                            time.sleep(0.01)
                        child.send_signal(sig)
                        stdout, stderr = child.communicate(timeout=7)
                    finally:
                        if child.poll() is None:
                            child.kill()
                            child.communicate()
                self.assertEqual(child.returncode, code, stderr)
                self.assertEqual(stdout, b"")
                self.assertEqual(int((self.root / "observed-signal").read_text()), signal.SIGTERM)
                self.assertFalse((self.root / "second.ir").exists())
                with wal.open("rb") as locked:
                    fcntl.flock(locked, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    rows = [json.loads(line) for line in locked]
                self.assertFalse(any(r.get("role") == "tool" for r in rows))
                resumed = subprocess.run([*args, "resume"], cwd=self.root, env=self.env,
                                         stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
                self.assertEqual(resumed.returncode, 0, resumed.stderr)
                rows = [json.loads(line) for line in (self.root / "second.ir").read_text().splitlines()]
                self.assertEqual([r["text"] for r in rows if r.get("role") == "user"],
                                 ["<args>before</args>", "<args>resume</args>"])
                self.assertFalse(any(r.get("tool_calls") or r.get("role") == "tool" for r in rows))

    # RFC §4.1, §4.3
    def test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity(self):
        stdout = bytes(range(256)) * 4097 + b"stdout-end"
        stderr = bytes(reversed(range(256))) * 4099 + b"stderr-end"
        (self.root / "output").write_bytes(stdout)
        (self.root / "diagnostic").write_bytes(stderr)
        (self.root / "observe.py").write_text('''import fcntl, json, os, stat
from pathlib import Path
null, stdin = os.stat("/dev/null"), os.fstat(0)
print(json.dumps({
    "null_stdin": stat.S_ISCHR(stdin.st_mode) and stdin.st_rdev == null.st_rdev,
    "write_pipes": [stat.S_ISFIFO(os.fstat(fd).st_mode) and
                    (fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE) == os.O_WRONLY
                    for fd in (4, 5, 6)],
}))
for name, channel in (("output","AGENT_OUT"), ("diagnostic","AGENT_ERR")):
    with Path(name).open("rb") as src:
        while chunk := src.read(65521):
            with open(os.environ[channel], "wb") as dst:
                dst.write(chunk)
''')
        cmd = 'exec "$FIXTURE_PYTHON" observe.py'
        events = [{"type":"tool_call", "id":"relay", "name":"msh", "arguments":{"cmd":cmd}},
                  {"type":"done", "finish_reason":"tool_calls"}]
        (self.root / "first.reply").write_text("".join(json.dumps(e) + "\n" for e in events))
        out = subprocess.run([self.agent, "-y", "-q", "case.agent", "--", "go"], cwd=self.root,
                             stdin=subprocess.DEVNULL, capture_output=True, timeout=20,
                             env={**self.env, "FIXTURE_PYTHON": sys.executable})
        self.assertEqual(out.returncode, 0, out.stderr[-160:])
        for actual, expected in ((out.stdout, stdout), (out.stderr, stderr)):
            self.assertEqual(len(actual), len(expected))
            self.assertTrue(actual == expected, "relayed bytes changed")
        msgs = [json.loads(line) for line in (self.root / "second.ir").read_text().splitlines()]
        result = [item for item in msgs if item.get("role") == "tool"]
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["tool_call_id"], "relay")
        header, report = result[0]["text"].split("\n", 1)
        self.assertEqual(header, f'[t{result[0]["turn_id"]}] exit=0')
        self.assertEqual(json.loads(report), {"null_stdin": True, "write_pipes": [True]*3})

    # RFC §4.1, §4.3
    def test_stdout_and_stderr_terminal_modes_are_independent(self):
        cmd = r"""printf 'a\000\377b' > "$AGENT_OUT"; printf 'human\n' > "$AGENT_ERR" """
        reply = [{"type": "reasoning_delta", "text": "reasoning-fixture"},
                 {"type": "tool_call", "id": "tty-tool", "name": "msh", "arguments": {"cmd": cmd}},
                 {"type": "done", "finish_reason": "tool_calls"}]
        (self.root / "first.reply").write_text("".join(json.dumps(v) + "\n" for v in reply))

        def channel(stack, is_terminal):
            if not is_terminal:
                stream = stack.enter_context(tempfile.TemporaryFile())

                def read():
                    stream.seek(0)
                    return stream.read()
                return stream, read
            master, slave = pty.openpty()
            stack.callback(os.close, master)
            stack.callback(os.close, slave)
            tty.setraw(slave)
            os.set_blocking(master, False)

            def read():
                data = bytearray()
                while True:
                    try:
                        chunk = os.read(master, 8192)
                    except BlockingIOError:
                        break
                    if not chunk:
                        break
                    data.extend(chunk)
                return bytes(data)
            return slave, read

        for out_tty in (True, False):
            for err_tty in (True, False):
                for mode in ("auto", "explicit"):
                    with self.subTest(stdout_tty=out_tty, stderr_tty=err_tty, mode=mode), ExitStack() as stack:
                        (self.root / "first.ir").unlink(missing_ok=True)
                        out, read_out = channel(stack, out_tty)
                        err, read_err = channel(stack, err_tty)
                        result = subprocess.run([self.agent, "-y", "--stderr", mode, "case.agent", "--", "go"],
                                                cwd=self.root, env=self.env, stdout=out, stderr=err,
                                                stdin=subprocess.DEVNULL, timeout=20)
                        payload, diagnostic = read_out(), read_err()
                        self.assertEqual(result.returncode, 0, diagnostic)
                        self.assertEqual(payload, b"a\0\xffb")
                        if mode == "explicit":
                            self.assertEqual(diagnostic, b"human\n")
                            continue
                        for marker in (b"reasoning-fixture", b"model finished", b"human\n"):
                            self.assertIn(marker, diagnostic)
                        lines = [line for line in diagnostic.split(b"\n") if b"[usage]" in line]
                        self.assertEqual(len(lines), 1, diagnostic)
                        self.assertRegex(lines[0], rb"ctx \d+/128000 \(\d+%\).*run\b")
                        if err_tty:
                            prefix = lines[0].split(b"[usage]", 1)[0]
                            self.assertIn(b"\x1b[2m", prefix)
                        # The fixture produces no controls itself. Strip SGR and
                        # verify that the rest has no cursor motion or redraws.
                        plain = re.sub(rb"\x1b\[[\d;]*m", b"", diagnostic)
                        self.assertNotIn(b"\r", plain)
                        self.assertNotIn(b"\x1b", plain)

    # RFC §7.1, §6.4
    def test_real_shell_timeout_returns_an_error_tool_result_and_inference_continues(self):
        result, tool = self.invoke("exec /bin/sleep 15", {"MSH_TIMEOUT_SEC": "1"})
        self.assertTrue(tool["is_error"])
        self.assertTrue(tool["text"].startswith(f"[t{tool['turn_id']}] exit=124\n"))
        self.assertEqual(result.stdout, b"")

    # RFC §7.1, §9.5
    def test_wire_file_has_raw_ansi_and_model_receives_cleaned_text(self):
        _, tool = self.invoke(r"printf '\033[31mfixture color\033[0m'")
        self.assertTrue(tool["text"].endswith("\nfixture color"), tool)
        self.assertNotIn("\x1b", tool["text"])
        trace = [json.loads(line) for line in (self.root / "trace").read_text().splitlines()]

        def strings(value):
            if isinstance(value, str):
                yield value
            elif isinstance(value, list):
                for child in value:
                    yield from strings(child)
            elif isinstance(value, dict):
                for child in value.values():
                    yield from strings(child)

        self.assertTrue(any("\x1b[31mfixture color\x1b[0m" in s for s in strings(trace)),
                        "the private wire trace did not retain the unfiltered tool output")

    # RFC §2
    def test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both(self):
        override = self.root / "bin"
        override.mkdir()
        (override / "msh").write_text("#!/bin/sh\nprintf path-shell\n")
        (override / "msh").chmod(0o700)
        (self.root / "custom-shell").write_text("#!/bin/sh\nprintf explicit-shell\n")
        (self.root / "custom-shell").chmod(0o700)
        for shell, text in [(None, "path-shell"), ("./custom-shell", "explicit-shell")]:
            with self.subTest(shell=shell):
                (self.root / "first.ir").unlink(missing_ok=True)
                _, tool = self.invoke("printf original", {"PATH": f"{override}:/bin:/usr/bin"}, shell)
                self.assertTrue(tool["text"].endswith("\n" + text), tool)


    def run_nested(self, approval_opts, child_policy, permitted):
        config = self.root / "config/agent/config.toml"
        with config.open("a") as f:
            f.write("\n[approval]\nsafe_commands=['start-child']\n"
                    "[providers.inner]\nbin='inner-provider'\n")
        launcher_dir = self.root / "bin"
        launcher_dir.mkdir()
        launch = launcher_dir / "start-child"
        launch.write_text('#!/bin/sh\nexec "$TEST_AGENT_BINARY" child.agent -- subtask\n')
        launch.chmod(0o700)
        self.root.joinpath("child.agent").write_text(
            f"---\nprovider: inner\nmodel: child-model\napproval: {child_policy}\n"
            "stderr: explicit\ncontext: child.wal\nmax_turns: 5\n---\nchild instructions")
        inner = self.libexec / "inner-provider"
        inner.write_text("""#!/bin/sh
set -eu
printf '%s' "$AGENT_MODEL" > child-model
n=1
while [ -f "child-$n.ir" ]; do n=$((n+1)); done
/bin/cat > "child-$n.ir"
/bin/cat "child-$n.reply"
""")
        inner.chmod(0o700)
        for n in (1, 2, 3):
            if n < 3:
                event = {"type": "tool_call", "id": f"inner-{n}", "name": "msh",
                         "arguments": {"cmd": f"printf ran > child-marker-{n}"}}
                reason = "tool_calls"
            else:
                event, reason = {"type": "text_delta", "text": "child done"}, "stop"
            self.root.joinpath(f"child-{n}.reply").write_text(
                json.dumps(event) + "\n" + json.dumps({"type": "done", "finish_reason": reason}) + "\n")
        result, parent_tool = self.invoke(
            "start-child", {"PATH": f"{launcher_dir}:/bin:/usr/bin",
                            "TEST_AGENT_BINARY": str(self.agent), "AGENT_MODEL": "outer-model",
                            "AGENT_PROVIDER": "fixture", "AGENT_CONTEXT": "parent.wal",
                            "AGENT_MAX_TURNS": "2"}, approval_opts=approval_opts)
        self.assertIn("exit=0\n", parent_tool["text"], parent_tool)
        self.assertEqual(result.stdout, b"")
        self.assertEqual((self.root / "child-model").read_text(), "child-model")
        for n in (1, 2):
            self.assertEqual(self.root.joinpath(f"child-marker-{n}").is_file(), permitted)
            rows = [json.loads(line) for line in self.root.joinpath(f"child-{n+1}.ir").read_text().splitlines()]
            self.assertEqual(rows[-1]["tool_call_id"], f"inner-{n}")
            self.assertEqual(rows[-1].get("is_error", False), not permitted)
        parent = (self.root / "parent.wal").read_text()
        child = (self.root / "child.wal").read_text()
        self.assertIn("child instructions", child)
        self.assertNotIn("child instructions", parent)

    # RFC §3.4, §5.1
    def test_nested_agent_uses_own_model_wal_and_budget_and_inherits_cli_auto(self):
        self.run_nested(("-y",), "ask", True)

    # RFC §3.4, §5.1
    def test_nested_agent_inherits_cli_deny_over_its_own_auto_frontmatter(self):
        self.run_nested(("--approval", "deny"), "auto", False)


if __name__ == "__main__":
    unittest.main()
