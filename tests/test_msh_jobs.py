"""RFC v1.1.2 background jobs through the public msh and anix executables."""
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest


BIN = Path(os.environ.get("ANIX_TEST_BIN_DIR", Path(__file__).resolve().parents[1] / "target/debug")).resolve()


def rows(path):
    data = path.read_bytes()
    if data and not data.endswith(b"\n"):
        raise AssertionError(f"incomplete JSONL in {path}")
    return [json.loads(line) for line in data.splitlines()]


def wait_for(path, timeout=8):
    deadline = time.monotonic() + timeout
    while not path.exists():
        if time.monotonic() >= deadline:
            raise AssertionError(f"timed out waiting for fixture {path.name}")
        time.sleep(0.02)


def write_executable(path, text):
    path.write_text(text)
    path.chmod(0o700)


class MshJobs(unittest.TestCase):
    def setUp(self):
        area = tempfile.TemporaryDirectory(prefix="anix-jobs-")
        self.addCleanup(area.cleanup)
        self.root = Path(area.name).resolve()
        self.jobs = self.root / "run/anix/jobs"
        self.config = self.root / "config/anix/config.toml"
        self.config.parent.mkdir(parents=True)
        self.env = {"HOME": str(self.root), "XDG_RUNTIME_DIR": str(self.root / "run"),
                    "XDG_CONFIG_HOME": str(self.root / "config"), "PATH": "/usr/bin:/bin",
                    "PYTHON": sys.executable}
        (self.root / "case.agent").write_text("Frozen prefix 文\n")
        self.addCleanup((self.root / "release").touch)

    def direct(self, cmd, timeout, kill=None):
        env = dict(self.env, MSH_TIMEOUT_SEC=str(timeout), ANIX_OUT="/dev/fd/4",
                   ANIX_ERR="/dev/fd/5", ANIX_CTL="/dev/fd/6")
        if kill is not None:
            env["MSH_KILL_ON_TIMEOUT"] = str(kill)
        return subprocess.run(["/bin/sh", "-c", 'exec "$@" 4>payload 5>diagnostic 6>control', "fixture",
                               BIN / "msh", "-c", cmd], cwd=self.root, env=env,
                              stdin=subprocess.DEVNULL, capture_output=True, timeout=12)

    def worker(self, output, status):
        (self.root / "raw").write_bytes(output)
        (self.root / "worker.py").write_text("""import json, os, sys, time
from pathlib import Path
def inspect(fd):
    try:
        os.fstat(fd)
    except OSError:
        return False
    return True
Path("identity").write_text(json.dumps([os.getpgrp(), os.getsid(0),
    [inspect(fd) for fd in (4, 5, 6)], [os.getenv(k) for k in ("ANIX_OUT", "ANIX_ERR", "ANIX_CTL")]]))
sys.stdout.buffer.write(Path("raw").read_bytes())
sys.stdout.flush()
Path("ready").touch()
deadline = time.monotonic() + 15
while not Path("release").exists():
    if time.monotonic() >= deadline:
        sys.exit(99)
    time.sleep(0.02)
os.write(2, b"WORKER-DONE\\n")
sys.exit(int(sys.argv[1]))
""")
        return f'exec "$PYTHON" worker.py {status}'

    # RFC v1.1.2 §7.1, §7.2
    def test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit(self):
        raw = b"actual stdout\x00\xff\n"
        cmd = self.worker(raw, 7)
        result = self.direct(cmd, 0)
        self.assertEqual(result.returncode, 0, result.stderr)
        frames = rows(self.root / "control")
        self.assertEqual(len(frames), 1)
        job = frames[0]
        self.assertEqual((job["op"], job["cmd"]), ("bg_job", cmd))
        self.assertTrue(job["job_id"])
        for key in ("job_id", "pid", "pgid", "log_path", "exit_file"):
            self.assertIn(str(job[key]), result.stdout.decode())
        log = self.jobs / (job["job_id"] + ".log")
        exit_file = self.jobs / (job["job_id"] + ".exit")
        self.assertEqual((job["log_path"], job["exit_file"]), (str(log), str(exit_file)))
        wait_for(self.root / "ready")
        group, session, fds, env = json.loads((self.root / "identity").read_text())
        self.assertEqual((job["pid"], job["pgid"], session), (group, group, group))
        self.assertNotEqual(group, os.getpgrp())
        self.assertEqual((fds, env), ([False] * 3, [None] * 3))
        self.assertEqual((stat.S_IMODE(self.jobs.stat().st_mode), stat.S_IMODE(log.stat().st_mode)), (0o700, 0o600))
        self.assertEqual(log.read_bytes(), raw)
        self.assertFalse(exit_file.exists())
        (self.root / "release").touch()
        wait_for(exit_file)
        self.assertEqual(int(exit_file.read_bytes()), 7)
        self.assertEqual(log.read_bytes(), raw + b"WORKER-DONE\n")

    # RFC v1.1.2 §7.1, §7.2
    def test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log(self):
        source = b"".join(f"row-{n:03d}:".encode() + b"x" * 110 + b"\n" for n in range(90))
        raw = source + b"\x1b[31mLAST\x1b[0m\n"
        cmd = self.worker(raw, 0)
        result = self.direct(cmd, 1)
        self.assertEqual(result.returncode, 0, result.stderr)
        job, = rows(self.root / "control")
        self.assertEqual((job["cmd"], job["op"]), (cmd, "bg_job"))
        preview = result.stdout.split(b"[msh: still running", 1)[0]
        self.assertIn(b"LAST", preview)
        self.assertNotIn(b"row-000:", preview)
        self.assertNotIn(b"\x1b", preview)
        # Count actual source-line contents, excluding handle and any truncation notice.
        visible = [line for line in preview.splitlines() if b"[msh:" not in line and b"earlier output" not in line]
        self.assertLessEqual(len(visible), 40)
        self.assertLessEqual(len(b"\n".join(visible)), 4096)
        self.assertFalse(Path(job["exit_file"]).exists())
        self.assertEqual(Path(job["log_path"]).read_bytes(), raw)
        (self.root / "release").touch()
        wait_for(Path(job["exit_file"]))
        self.assertEqual(int(Path(job["exit_file"]).read_bytes()), 0)
        self.assertEqual(Path(job["log_path"]).read_bytes(), raw + b"WORKER-DONE\n")

    # RFC v1.1.2 §7.1, §7.2
    def test_foreground_completion_returns_real_status_and_removes_job_files(self):
        for status in (0, 7):
            with self.subTest(status=status):
                result = self.direct(f"printf one; printf two >&2; exit {status}", 8)
                self.assertEqual((result.returncode, result.stdout, result.stderr), (status, b"onetwo", b""))
                self.assertEqual(rows(self.root / "control"), [])
                self.assertEqual(list(self.jobs.iterdir()), [])

    # RFC v1.1.2 §6.4, §6.5, §7.1
    def test_foreground_int_and_term_clean_up_in_both_timeout_modes(self):
        script = self.root / "sig.py"
        script.write_text("""import signal, time
from pathlib import Path
def finish(sig, frame):
    Path('received').write_text(str(sig))
    raise SystemExit(0)
signal.signal(signal.SIGTERM, finish)
Path('ready').touch()
time.sleep(12)
""")
        for sig in (signal.SIGINT, signal.SIGTERM):
            for kill in ("0", "1"):
                with self.subTest(signal=sig, kill=kill):
                    for name in ("ready", "received"):
                        (self.root / name).unlink(missing_ok=True)
                    with subprocess.Popen([BIN / "msh", "-c", 'exec "$PYTHON" sig.py'], cwd=self.root,
                                          stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                          env=dict(self.env, MSH_TIMEOUT_SEC="18", MSH_KILL_ON_TIMEOUT=kill)) as p:
                        try:
                            wait_for(self.root / "ready")
                            p.send_signal(sig)
                            _, stderr = p.communicate(timeout=8)
                        finally:
                            if p.poll() is None:
                                p.kill()
                    self.assertEqual(p.returncode, 128 + sig, stderr)
                    self.assertEqual(int((self.root / "received").read_text()), signal.SIGTERM)
                    self.assertEqual(list(self.jobs.iterdir()), [])

    # RFC v1.1.2 §6.5, §7.1
    def test_zero_with_explicit_kill_waits_one_second_and_leaves_no_job(self):
        started = time.monotonic()
        run = self.direct("printf begun; exec /bin/sleep 12", 0, kill=1)
        self.assertGreaterEqual(time.monotonic() - started, 0.95)
        self.assertEqual(run.returncode, 124, run.stderr)
        self.assertIn(b"begun", run.stdout)
        self.assertIn(b"[msh: command timed out after 1s; process group terminated (exit=124)]", run.stdout)
        self.assertEqual(rows(self.root / "control"), [])
        self.assertEqual(list(self.jobs.iterdir()), [])

    # RFC v1.1.2 §7.1, §7.2
    def test_long_commands_execute_their_suffix_and_handoff_with_a_single_bounded_frame(self):
        for length in (511, 512, 513, 32_768, 66_000):
            with self.subTest(command_bytes=length):
                suffix = f"\nprintf 'finished-{length}'"
                cmd = "#".ljust(length - len(suffix), "x") + suffix
                run = self.direct(cmd, 0)
                self.assertEqual(run.returncode, 0, run.stderr)
                control = self.root / "control"
                self.assertLessEqual(control.stat().st_size, 65_536)
                job, = rows(control)
                self.assertEqual(job["op"], "bg_job")
                self.assertIsInstance(job["cmd"], str)
                self.assertTrue(job["cmd"])
                wait_for(Path(job["exit_file"]))
                self.assertEqual(int(Path(job["exit_file"]).read_bytes()), 0)
                self.assertEqual(Path(job["log_path"]).read_text(), f"finished-{length}")

    def configure(self, defaults=""):
        self.config.write_text(f"[defaults]\nmodel='local'\nprovider='fixture'\n{defaults}"
                               f"\n[providers.fixture]\nmode='oneshot'\nbin={json.dumps(str(self.root / 'provider'))}\n")
        write_executable(self.root / "provider", """#!/bin/sh
set -eu
n=0
if test -f n; then read -r n < n; fi
n=$((n+1))
printf '%s\\n' "$n" > n
cat > "request-$n"
cat "reply-$n"
""")

    def reply(self, turn, calls=()):
        events = [{"type": "tool_call", "id": name, "name": "msh", "arguments": args} for name, args in calls]
        if not calls:
            events.append({"type": "text_delta", "text": "done"})
        events.append({"type": "done", "finish_reason": "tool_calls" if calls else "stop"})
        (self.root / f"reply-{turn}").write_text("".join(json.dumps(e) + "\n" for e in events))

    def invoke(self, shell, extra_env=None):
        run = subprocess.run(["/bin/sh", "-c", 'exec "$@" 7>>trace', "fixture", BIN / "anix", "--trace-fd=7",
                              "-q", "-y", "-c", "wal", "--shell", shell, "case.agent", "--", "go"], cwd=self.root,
                             env={**self.env, **(extra_env or {})}, stdin=subprocess.DEVNULL, capture_output=True,
                             timeout=20)
        self.assertEqual(run.returncode, 0, run.stderr)

    def capture_policy(self, options, env=None):
        shell = self.root / "capture-shell"
        write_executable(shell, """#!/bin/sh
set -eu
printf '%s|%s|%s|%s|%s\\n' "$1" "$2" "$MSH_TIMEOUT_SEC" "$MSH_KILL_ON_TIMEOUT" "$MSH_JOBS_DIR" >> settings
""")
        self.reply(1, [(f"call-{i}", {"cmd": f"printf {i}", **args}) for i, args in enumerate(options)])
        self.reply(2)
        self.invoke(shell, env)
        observed = []
        for i, line in enumerate((self.root / "settings").read_text().splitlines()):
            flag, cmd, seconds, kill, jobs = line.split("|")
            self.assertEqual((flag, cmd, jobs), ("-c", f"printf {i}", str(self.jobs)))
            observed.append((int(seconds), int(kill)))
        return observed

    # RFC v1.1.2 §7.1, §8.2, §9.1
    def test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call(self):
        self.configure("msh_timeout_sec=3\nmax_msh_timeout_sec=5\n")
        args = [{}, {"timeout_sec": 0}, {"timeout_sec": 1, "kill_on_timeout": True},
                {"timeout_sec": 4}, {"timeout_sec": 5}, {"timeout_sec": 6}, {"kill_on_timeout": False}, {}]
        self.assertEqual(self.capture_policy(args), [(3, 0), (0, 0), (1, 1), (4, 0), (5, 0), (5, 0), (3, 0), (3, 0)])
        header = rows(self.root / "request-1")[0]
        tool, = [t for t in header["tools"] if t["name"] == "msh"]
        schema = tool["parameters"]
        properties = schema["properties"]
        self.assertEqual(schema["required"], ["cmd"])
        self.assertEqual((properties["cmd"]["type"], properties["timeout_sec"]["type"],
                          properties["timeout_sec"]["minimum"], properties["kill_on_timeout"]["type"]),
                         ("string", "integer", 0, "boolean"))

    # RFC v1.1.2 §7.1, §8.2
    def test_single_call_never_exceeds_a_configured_maximum_below_the_default_wait(self):
        self.configure("max_msh_timeout_sec=2\n")
        self.assertEqual(self.capture_policy([{"timeout_sec": n} for n in (1, 2, 3, 999)]),
                         [(1, 0), (2, 0), (2, 0), (2, 0)])

    # RFC v1.1.2 §7.1, §8.2
    def test_environment_maximum_overrides_toml_for_each_explicit_request(self):
        self.configure("msh_timeout_sec=1\nmax_msh_timeout_sec=8\n")
        self.assertEqual(self.capture_policy([{}, {"timeout_sec": 8}, {}],
                                            {"MSH_TIMEOUT_SEC": "2", "MSH_MAX_TIMEOUT_SEC": "4"}),
                         [(2, 0), (4, 0), (2, 0)])

    # RFC v1.1.2 §6.2, §7.1, §7.2, §9.1, §9.5
    def test_background_completion_is_reported_once_before_inference_and_replays_from_wal(self):
        self.configure()
        cmd = self.worker(b"\x1b[31mnotice-body\x1b[0m\n", 7)
        (self.root / "finish.py").write_text("""import json, time
from pathlib import Path
job, = [json.loads(line) for line in Path('trace').read_bytes().splitlines()
        if json.loads(line)['type'] == 'bg_job_started']
Path('release').touch()
deadline = time.monotonic() + 8
while not Path(job['exit_file']).is_file():
    if time.monotonic() >= deadline:
        raise SystemExit(2)
    time.sleep(0.02)
""")
        self.reply(1, [("launch", {"cmd": cmd, "timeout_sec": 0})])
        self.reply(2, [("release", {"cmd": 'exec "$PYTHON" finish.py'})])
        self.reply(3, [("after", {"cmd": "printf next"})])
        self.reply(4)
        self.reply(5)
        self.invoke(BIN / "msh")
        self.invoke(BIN / "msh")
        trace = rows(self.root / "trace")
        start, = [r for r in trace if r["type"] == "bg_job_started"]
        finish, = [r for r in trace if r["type"] == "bg_job_finished"]
        self.assertLess(trace.index(start), trace.index(finish))
        for key in ("job_id", "pid", "pgid", "log_path"):
            self.assertEqual(start[key], finish[key], key)
        self.assertEqual(start["cmd"], cmd)
        self.assertEqual(int(Path(start["exit_file"]).read_bytes()), 7)
        self.assertEqual(finish["exit_code"], 7)
        self.assertIs(type(finish["elapsed_ms"]), int)
        self.assertGreaterEqual(finish["elapsed_ms"], 0)
        records = rows(self.root / "wal")
        update, = [r for r in records if r["type"] == "runtime_update"]
        for token in (start["job_id"], "7", "notice-body", "WORKER-DONE"):
            self.assertIn(token, update["content"])
        self.assertNotIn("\x1b", update["content"])
        frozen = rows(self.root / "request-1")[1]
        for n in range(1, 6):
            request = rows(self.root / f"request-{n}")
            self.assertEqual(request[1], frozen)
            updates = [r for r in request[2:] if r.get("role") == "system" and "<runtime_update>" in r["text"]]
            self.assertEqual(len(updates), int(n >= 3), f"request {n}")
            if updates:
                self.assertEqual(updates[0]["turn_id"], update["turn_id"])
                self.assertEqual(updates[0]["text"], "<runtime_update>" + update["content"] + "</runtime_update>")

    # RFC v1.1.2 §7.2, §9.5
    def test_finished_group_without_an_exit_file_reports_null_exactly_once(self):
        self.configure()
        # A custom shell reaps its own child so that the group has definitely
        # disappeared. This exercises the observer independently of PID 1.
        script = self.root / "reap.py"
        script.write_text("""import json, os, subprocess, sys
from pathlib import Path
if sys.argv[1] != "launch":
    raise SystemExit(0)
jobs = Path(os.environ["MSH_JOBS_DIR"])
jobs.mkdir(parents=True, exist_ok=True)
log = jobs / "j_fixture.log"
log.write_text("interrupted task output")
with subprocess.Popen(["/bin/sleep", "6"], start_new_session=True) as child:
    pgid = os.getpgid(child.pid)
    assert pgid == child.pid
    child.terminate()
    child.wait(timeout=3)
frame = dict(op="bg_job", job_id="j_fixture", pid=child.pid, pgid=pgid,
             cmd=sys.argv[1], log_path=str(log), exit_file=str(jobs / "j_fixture.exit"))
with open(os.environ["ANIX_CTL"], "w") as ctl:
    print(json.dumps(frame), file=ctl)
""")
        shell = self.root / "report-job"
        write_executable(shell, '#!/bin/sh\nexec "$PYTHON" reap.py "$2"\n')
        self.reply(1, [("one", {"cmd": "launch"})])
        self.reply(2, [("two", {"cmd": "next"})])
        self.reply(3)
        self.invoke(shell)
        self.assertFalse((self.jobs / "j_fixture.exit").exists())
        started, finished = [e for e in rows(self.root / "trace") if e["type"].startswith("bg_job_")]
        self.assertEqual((started["type"], finished["type"]), ("bg_job_started", "bg_job_finished"))
        self.assertEqual(finished["job_id"], "j_fixture")
        self.assertIn("exit_code", finished)
        self.assertIsNone(finished["exit_code"])
        update, = [r for r in rows(self.root / "wal") if r["type"] == "runtime_update"]
        self.assertIn("interrupted task output", update["content"])
        for turn in (2, 3):
            hits = [r for r in rows(self.root / f"request-{turn}") if r.get("turn_id") == update["turn_id"]]
            self.assertEqual(len(hits), 1)
            self.assertEqual(hits[0]["text"], "<runtime_update>" + update["content"] + "</runtime_update>")

    # RFC v1.1.2 §6.3, §7.1
    @unittest.skipUnless(sys.platform.startswith("linux") and shutil.which("strace"), "requires Linux strace")
    def test_hard_kill_grace_waits_for_a_living_descendant_without_repeated_polling(self):
        trace = self.root / "wait.trace"
        # The outer shell exits on TERM. Its finite child inherits SIG_IGN
        # and remains alive throughout the grace period. Trace only msh itself.
        cmd = "trap '' TERM; exec /bin/sleep 7"
        run = subprocess.run([shutil.which("strace"), "-c", "-qq", "-e", "trace=?poll,ppoll,?epoll_wait,epoll_pwait",
                              "-o", trace, BIN / "msh", "-c", cmd], cwd=self.root,
                             env=dict(self.env, LC_ALL="C", MSH_TIMEOUT_SEC="1", MSH_KILL_ON_TIMEOUT="1"),
                             stdin=subprocess.DEVNULL, capture_output=True, timeout=12)
        self.assertEqual(run.returncode, 124, run.stderr)
        # strace -c reports calls in column four. Count both elapsed timeouts
        # and repeated immediate returns from an already-readable signal FD.
        calls = {fields[-1]: int(fields[3]) for row in trace.read_text().splitlines()
                 if (fields := row.split()) and fields[-1] in {"poll", "ppoll", "epoll_wait", "epoll_pwait"}}
        self.assertTrue(calls, trace.read_text())
        # One command, one TERM, one grace deadline. Allow for startup probes,
        # child-exit races and rounding a millisecond deadline in either wait.
        self.assertLessEqual(sum(calls.values()), 16, calls)
