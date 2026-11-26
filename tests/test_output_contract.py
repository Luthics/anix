"""RFC v1.2 output behavior at configuration, provider, terminal and WAL boundaries."""
from itertools import product
import json
import os
import pty
import select
import subprocess
import sys
import time
import tty

from anix_fixture import OfflineAnixCase


class OutputContract(OfflineAnixCase):
    # RFC §3.4-12 — 对五层覆盖的 32 种开关组合轮换三个候选值，最高生效层决定 WAL、IR 和 stdout。
    def test_output_resolves_every_subset_of_cli_env_script_project_and_global(self):
        modes = ("reply", "none", "explicit")
        base = self.config.read_text()
        project = self.root / ".anix/config.toml"
        project.parent.mkdir()
        wal = self.root / "history"
        for bits in product((False, True), repeat=5):
            for offset in range(len(modes)):
                with self.subTest(layers=bits, rotation=offset):
                    highest = max((i for i, active in enumerate(bits) if active), default=-1)
                    # Give the winner a distinct value from every enabled lower layer.
                    layers = [None] * len(bits)
                    for i, active in enumerate(bits):
                        if active:
                            step = 0 if i == highest else 1 + i % 2
                            layers[i] = modes[(offset + step) % len(modes)]
                    glob, local, script, env, option = layers
                    self.config.write_text(base.replace("[defaults]", f'[defaults]\noutput="{glob}"') if glob else base)
                    project.write_text(f'[defaults]\noutput="{local}"' if local else "")
                    self.script.write_text(f"---\noutput: {script}\n---\nreply" if script else "reply")
                    if env:
                        self.env["ANIX_OUTPUT"] = env
                    else:
                        self.env.pop("ANIX_OUTPUT", None)
                    wal.unlink(missing_ok=True)
                    (self.root / "request").unlink(missing_ok=True)
                    args = ["-o", option] if option else []
                    result = self.invoke(*args, "-q", "-c", wal, self.script, "--", "go")
                    self.assertEqual(result.returncode, 0, result.stderr)
                    expected = next((v for v in reversed(layers) if v), "reply")
                    header = json.loads((self.root / "request").read_bytes().splitlines()[0])
                    saved = json.loads(wal.read_bytes().splitlines()[0])
                    # RFC-ASSERT §3.4-12
                    self.assertEqual((saved["config"]["output"], header["meta"]["output_mode"], result.stdout),
                                     (expected, expected, b"OK" if expected == "reply" else b""))

    # RFC §3.4-13 — 遍历 shell、output、approval 组合和正文迁移提示；禁止的组合在 WAL 或 Provider 请求出现之前报错。
    def test_output_startup_matrix_warns_or_stops_before_wal_and_provider(self):
        wal, request = self.root / "history", self.root / "request"
        for shell, output, approval, mentions_out in product(
                ("msh", "none"), ("reply", "explicit", "none"), ("ask", "auto", "deny"), (False, True)):
            with self.subTest(shell=shell, output=output, approval=approval, mentions_out=mentions_out):
                wal.unlink(missing_ok=True)
                request.unlink(missing_ok=True)
                self.script.write_text("write to $ANIX_OUT" if mentions_out else "reply")
                result = self.invoke("--shell", shell, "-o", output, "--approval", approval,
                                     "-q", "-c", wal, self.script, "--", "go")
                invalid = output == "explicit" and (shell == "none" or approval == "deny")
                warns = ((output == "reply" and shell != "none" and approval == "deny")
                         or (output == "none" and shell == "none") or (mentions_out and output != "explicit"))
                # Quiet mode excludes usage and normal model diagnostics. Each warning also occurs in isolation.
                # RFC-ASSERT §3.4-13
                self.assertEqual((result.returncode, bool(result.stderr.strip()), wal.exists(), request.exists()),
                                 (2 if invalid else 0, invalid or warns, not invalid, not invalid), result.stderr)
                self.assertEqual(result.stdout, b"OK" if not invalid and output == "reply" else b"")

    # RFC §3.4-11 — 对不可执行的文件和目录，分别通过 PATH 名称与绝对路径启动，须在写 WAL 和启动 Provider 前以 2 退出。
    def test_shell_resolution_requires_an_executable_file_before_any_provider_request(self):
        binary_dir = self.root / "bin"
        binary_dir.mkdir()
        self.env["PATH"] = str(binary_dir) + os.pathsep + self.env["PATH"]
        disabled = binary_dir / "no-exec"
        disabled.write_text("#!/bin/sh\nexit 0\n")
        disabled.chmod(0o600)
        directory = binary_dir / "dir-shell"
        directory.mkdir()
        wal = self.root / "history"
        request = self.root / "request"
        for path in (directory, disabled):
            for shell in (path.name, str(path)):
                with self.subTest(shell=shell):
                    wal.unlink(missing_ok=True)
                    request.unlink(missing_ok=True)
                    result = self.invoke("--shell", shell, "-q", "-c", wal, self.script, "--", "go")
                    # RFC-ASSERT §3.4-11
                    self.assertEqual((result.returncode, result.stdout, bool(result.stderr.strip()),
                                      wal.exists(), request.exists()), (2, b"", True, False, False))

    # RFC §4.1-19 — 以已处理的第二帧作同步点：纯文本的首段在 done 前可读，启用 shell 时同一时点的 stdout 为空。
    def test_reply_visibility_before_done_in_oneshot_and_coprocess(self):
        events = [
            {"type": "text_delta", "text": "先"},
            {"type": "reasoning_delta", "text": "barrier"},
            {"type": "text_delta", "text": "后"},
            {"type": "done", "finish_reason": "stop"},
        ]
        base = self.config.read_text()
        release = self.root / "release"
        self.provider.write_text(f"""#!{sys.executable}
import json, os, sys, time
from pathlib import Path
coprocess = os.environ.get("ANIX_PROVIDER_MODE") == "coprocess"
if coprocess:
    sys.stdin.readline()
else:
    sys.stdin.read()
frames = {events!r}
for frame in frames[:2]:
    print(json.dumps(frame), flush=True)
deadline = time.monotonic() + 10
while not Path("release").exists():
    if time.monotonic() >= deadline:
        sys.exit(1)
    time.sleep(.01)
for frame in frames[2:]:
    print(json.dumps(frame), flush=True)
if coprocess:
    sys.stdin.read()
""")
        for mode, shell in product(("oneshot", "coprocess"), ("none", "msh")):
            with self.subTest(mode=mode, shell=shell):
                self.config.write_text(base.replace('mode="oneshot"', f'mode="{mode}"'))
                release.unlink(missing_ok=True)
                trace_path = self.root / "trace"
                with trace_path.open("wb") as trace:
                    args = [self.anix, f"--trace-fd={trace.fileno()}", "--shell", shell, "-q", self.script, "--", "go"]
                    with subprocess.Popen(args, env={**self.env, "ANIX_TRACE_LEVEL": "deltas"}, cwd=self.root,
                                          stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                          pass_fds=(trace.fileno(),), start_new_session=True) as child:
                        try:
                            deadline = time.monotonic() + 6
                            while True:
                                complete = trace_path.read_bytes().split(b"\n")[:-1]
                                if any((r := json.loads(line)).get("type") == "reasoning_delta"
                                       and r.get("text") == "barrier" for line in complete):
                                    break
                                self.assertIsNone(child.poll(), "provider exited before the synchronization frame")
                                self.assertLess(time.monotonic(), deadline, "core did not process the second frame")
                                time.sleep(.01)
                            readable, _, _ = select.select([child.stdout], [], [], 0)
                            prefix = os.read(child.stdout.fileno(), 64) if readable else b""
                            # RFC-ASSERT §4.1-19
                            self.assertEqual(prefix, "先".encode() if shell == "none" else b"")
                        finally:
                            release.touch()
                            tail, err = child.communicate(timeout=10)
                        self.assertEqual(child.returncode, 0, err)
                        # RFC-ASSERT §4.1-19
                        self.assertEqual(prefix + tail, "先后".encode())

    # RFC §4.1-20 — 比较真实 TTY 和管道；空回复、无换行、已有一个或两个换行时只在规定条件下补一个 LF。
    def test_reply_newline_depends_on_stdout_tty_and_last_byte(self):
        for shell, is_tty, body in product(("msh", "none"), (False, True), ("", "末", "末\n", "末\n\n")):
            with self.subTest(shell=shell, tty=is_tty, body=body):
                self.reply([{"type": "text_delta", "text": body}, {"type": "done", "finish_reason": "stop"}])
                args = ["--shell", shell, "-o", "reply", "-q", self.script, "--", "go"]
                if is_tty:
                    master, slave = pty.openpty()
                    try:
                        tty.setraw(slave)
                        result = subprocess.run([self.anix, *args], cwd=self.root, env=self.env, stdout=slave,
                                                stdin=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=10)
                        payload = bytearray()
                        while select.select([master], [], [], 0)[0]:
                            payload.extend(os.read(master, 4096))
                    finally:
                        os.close(slave)
                        os.close(master)
                else:
                    result = self.invoke(*args)
                    payload = result.stdout
                if body:
                    self.assertEqual(result.returncode, 0, result.stderr)
                # With no text and no tool call the response can be rejected. §4.1 still forbids a stray LF.
                newline = b"\n" if is_tty and body and not body.endswith("\n") else b""
                # RFC-ASSERT §4.1-20
                self.assertEqual(bytes(payload), body.encode() + newline)

    # RFC §6.2-20 — 恢复同一 WAL 时只对 output 或 shell 启用状态的实际变化追加一次尾部通知，冻结前缀保持原样。
    def test_resume_distinguishes_shell_capability_from_executable_path_and_output(self):
        wal = self.root / "history"
        modes = [("msh", "reply"), ("/bin/sh", "reply"), ("none", "reply"), ("none", "none"),
                 ("msh", "explicit"), ("/bin/sh", "explicit"), ("msh", "reply")]
        previous = None
        frozen = None
        for shell, output in modes:
            for _ in range(2):
                with self.subTest(shell=shell, output=output, previous=previous):
                    prefix = wal.read_bytes() if wal.exists() else b""
                    result = self.invoke("--shell", shell, "-o", output, "-q", "-c", wal, self.script, "--", "go")
                    self.assertEqual(result.returncode, 0, result.stderr)
                    after = wal.read_bytes()
                    self.assertTrue(after.startswith(prefix))
                    appended = [json.loads(line) for line in after[len(prefix):].splitlines()]
                    updates = [r for r in appended if r["type"] == "runtime_update"]
                    capability = (shell != "none", output)
                    changed = previous is not None and previous != capability
                    previous = capability
                    # RFC-ASSERT §6.2-20
                    self.assertEqual(len(updates), int(changed))
                    messages = [json.loads(line) for line in (self.root / "request").read_bytes().splitlines()[1:]]
                    if frozen is None:
                        frozen = messages[0]
                    # RFC-ASSERT §6.2-20
                    self.assertEqual(messages[0], frozen)
                    if changed:
                        # The notification precedes this invocation's user input, after the previous dialogue.
                        # RFC-ASSERT §6.2-20
                        self.assertEqual((messages[-2]["role"], messages[-2]["text"]),
                                         ("system", f'<runtime_update>{updates[0]["content"]}</runtime_update>'))
