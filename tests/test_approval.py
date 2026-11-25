"""RFC §5 black-box approval tests with disposable providers, deciders and terminals."""
import json
import os
from pathlib import Path
import shlex
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest
import uuid

from terminal_fixture import controlling_terminal, read_until, read_until_any


BUILD = Path(os.environ.get("ANIX_TEST_BIN_DIR", Path(__file__).resolve().parents[1] / "target/debug")).resolve()
COMMAND = "printf approved > receipt"
PIPE_INPUT = "pipeline input 文"
PROMPT = b"approval-fixture> "


def executable(path, source):
    path.write_text(source)
    path.chmod(0o700)
    return path


def rows(path):
    data = path.read_bytes()
    if not data.endswith(b"\n"):
        raise AssertionError(f"incomplete JSONL in {path}")
    return [json.loads(line) for line in data.splitlines()]


class ApprovalFixture:
    def __init__(self, root, calls, approval_settings):
        self.root = root
        self.env = {"HOME": str(root), "PATH": f"{root}:/bin:/usr/bin", "TERM": "dumb",
                    "XDG_CONFIG_HOME": str(root / "config"), "XDG_RUNTIME_DIR": str(root / "run"),
                    "ANIX_TRACE_LEVEL": "events", "FIXTURE_PROMPT": PROMPT.decode()}
        config = root / "config/anix"
        config.mkdir(parents=True)
        (config / "config.toml").write_text(
            "[defaults]\nmodel='local-model'\nprovider='fixture'\n"
            "[providers.fixture]\nbin='provider'\nmode='oneshot'\ntimeout_sec=8\n"
            "[approval]\n" + approval_settings)
        (root / "case.agent").write_text("---\nname: approval-fixture\n---\nLocal fixture")
        executable(root / "provider", """#!/bin/sh
set -eu
n=0
if test -f count; then read -r n < count; fi
n=$((n+1))
printf '%s\\n' "$n" > count
/bin/cat > "request-$n"
/bin/cat "reply-$n"
""")
        for i, batch in enumerate(calls, 1):
            if batch:
                reply = [{"type": "tool_call", "id": call_id, "name": "msh", "arguments": {"cmd": command}}
                         for call_id, command in batch]
                reply.append({"type": "done", "finish_reason": "tool_calls"})
            else:
                reply = [{"type": "text_delta", "text": "complete"}, {"type": "done", "finish_reason": "stop"}]
            (root / f"reply-{i}").write_text("".join(json.dumps(e, ensure_ascii=False) + "\n" for e in reply))

    def decider(self, body):
        # cat must see EOF before the decider can publish readiness or make its decision.
        executable(self.root / "decide", "#!/bin/sh\nset -eu\n/bin/cat > frame.tmp\n"
                   "/bin/mv frame.tmp frame\nprintf 'called\\n' >> decider-calls\n" + body + "\n")

    def argv(self, options):
        return [str(BUILD / "anix"), "-q", "--shell", "/bin/sh", "--trace-fd=7", "-c", "history",
                *options, "case.agent"]

    def detached(self, *options):
        return subprocess.run(["/bin/sh", "-c", 'exec "$@" 7>trace', "fixture", *self.argv(options)],
                              cwd=self.root, env=self.env, input=PIPE_INPUT.encode(),
                              capture_output=True, start_new_session=True, timeout=25)

    def terminal(self, exchanges, options=(), reissued_command=None):
        (self.root / "pipe-input").write_bytes(PIPE_INPUT.encode())
        shell = ["/bin/bash", "--noprofile", "--norc", "-i"]
        with controlling_terminal(shell, self.root, self.env) as (master, _):
            os.write(master, b"stty -g > tty-before; PS1=$FIXTURE_PROMPT\n")
            read_until(master, PROMPT)
            command = f"/bin/cat pipe-input | {shlex.join(self.argv(options))}"
            os.write(master, (command + " >payload 2>diagnostic 7>trace\n").encode())
            for marker, answer in exchanges:
                read_until(master, marker.encode())
                os.write(master, answer)
            if reissued_command is None:
                read_until(master, PROMPT)
            else:
                # RFC issue #10 leaves the action after an aborted edit open.
                # If the runtime prompts again, refuse before observing its effects.
                marker, _ = read_until_any(master, (PROMPT, reissued_command.encode()))
                if marker != PROMPT:
                    os.write(master, b"n\n\n")
                    read_until(master, PROMPT)
            # Read status before running another command. A pipeline and terminal state
            # observed from this same shell also exercise recovery from keyboard SIGINT.
            os.write(master, b'printf "%s" "$?" | cat > status; stty -g > tty-after\n')
            read_until(master, PROMPT)
            os.write(master, b"exit\n")
        if (self.root / "tty-before").read_bytes() != (self.root / "tty-after").read_bytes():
            raise AssertionError("terminal settings changed")
        return int((self.root / "status").read_text())

    def trace(self, kind):
        return [r for r in rows(self.root / "trace") if r["type"] == kind]

    def tools(self, request_number=2):
        return [r for r in rows(self.root / f"request-{request_number}") if r.get("role") == "tool"]


class Approval(unittest.TestCase):
    def fixture(self, calls=None, settings="safe_commands=[]\n"):
        tmp = tempfile.TemporaryDirectory(prefix="anix-approval-")
        self.addCleanup(tmp.cleanup)
        return ApprovalFixture(Path(tmp.name).resolve(), calls if calls is not None else [[("t", COMMAND)], []],
                               settings)

    def assert_cycle(self, fx, code, reason):
        cycle, = fx.trace("cycle_end")
        end, = fx.trace("run_end")
        self.assertEqual((cycle["reason"], cycle["exit_code"], end["exit_code"]), (reason, code, code))

    # RFC §5.3-01 — 提示与回答走控制终端，管道 stdin 保留为模型输入。
    # RFC §5.3-03 — n 返回模型错误，支持空理由及 UTF-8 文本。
    def test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input(self):
        for allow, reason in [(True, ""), (False, "稍后再试"), (False, "")]:
            with self.subTest(allow=allow, reason=reason):
                fx = self.fixture()
                answer = b"y\n" if allow else ("n\n" + reason + "\n").encode()
                exchanges = [(COMMAND, answer)]
                self.assertEqual(fx.terminal(exchanges), 0)
                self.assertEqual((fx.root / "payload").read_bytes(), b"complete")
                self.assertNotIn(COMMAND.encode(), (fx.root / "diagnostic").read_bytes())
                first = rows(fx.root / "request-1")
                self.assertEqual([r["text"] for r in first if r.get("role") == "user"], [PIPE_INPUT])
                result, = fx.tools()
                self.assertEqual(result["tool_call_id"], "t")
                self.assertEqual(result.get("is_error", False), not allow)
                self.assertEqual((fx.root / "receipt").exists(), allow)
                if not allow:
                    self.assertIn("Error: User denied this action.", result["text"])
                    if reason:
                        self.assertIn("Reason: " + reason, result["text"])
                event, = fx.trace("tool_approval")
                self.assertEqual(event["decider"], "builtin")
                self.assertNotIn("decider_exit_code", event)
                self.assert_cycle(fx, 0, "completed")

    # RFC §5.1-03 — Always 对同一可执行程序生效，换进程读取同一 WAL 后需重新审批。
    # RFC §5.3-04 — a 的授权以可执行程序及 PID 为界。
    def test_always_applies_to_one_executable_and_resets_on_wal_resume(self):
        fx = self.fixture([[("first", "printf one"), ("same", "printf two"), ("other", "echo three")],
                           [], [("new-process", "printf four")], []], "safe_commands=[]\nbin='decide'\n")
        fx.decider("exit 2")
        self.assertEqual(fx.terminal([("printf one", b"a\n"), ("echo three", b"y\n")]), 0)
        self.assertEqual([r["text"].split("\n", 1)[-1].strip() for r in fx.tools()], ["one", "two", "three"])
        self.assertEqual(fx.terminal([("printf four", b"y\n")]), 0)
        self.assertEqual((fx.root / "decider-calls").read_text().splitlines(), ["called"] * 3)
        self.assertEqual(fx.tools(4)[-1]["text"].split("\n", 1)[-1].strip(), "four")

    # RFC §5.3-05 — 编辑器收到私有临时文件，只执行保存后的命令并清理文件。
    def test_editor_receives_private_original_and_only_the_edited_command_runs(self):
        for use_default in (False, True):
            with self.subTest(fallback_vi=use_default):
                fx = self.fixture()
                name = "vi" if use_default else "fixture-editor"
                editor = executable(fx.root / name, f'''#!{sys.executable}
import json, pathlib, stat, sys
p = pathlib.Path(sys.argv[1])
pathlib.Path("edited-file").write_text(json.dumps([str(p), stat.S_IMODE(p.stat().st_mode), p.read_text()]))
p.write_text("printf edited > edited-receipt")
''')
                if not use_default:
                    fx.env["EDITOR"] = str(editor)
                self.assertEqual(fx.terminal([(COMMAND, b"e\n")]), 0)
                filename, mode, original = json.loads((fx.root / "edited-file").read_text())
                path = Path(filename)
                self.assertEqual(path.parent, Path("/tmp"))
                self.assertTrue(path.name.startswith("anix_edit_"))
                self.assertEqual(path.suffix, ".sh")
                uuid.UUID(path.stem.removeprefix("anix_edit_"))
                self.assertEqual((mode, original.rstrip("\n")), (stat.S_IRUSR | stat.S_IWUSR, COMMAND))
                self.assertFalse(path.exists())
                self.assertFalse((fx.root / "receipt").exists())
                self.assertEqual((fx.root / "edited-receipt").read_text(), "edited")
                self.assertFalse(fx.tools()[0].get("is_error", False))

    # RFC §5.3-05 — 编辑失败或保存为空时，原命令和编辑结果均未执行，临时文件被清理。
    def test_failed_or_empty_edit_removes_the_file_and_runs_neither_command(self):
        for edit in ["""printf 'printf altered > alternate' > "$1"\nexit 1""", ': > "$1"']:
            with self.subTest(editor_body=edit):
                fx = self.fixture()
                editor = executable(fx.root / "fixture-editor", '#!/bin/sh\nprintf %s "$1" > edit-path\n' + edit)
                fx.env["EDITOR"] = str(editor)
                fx.terminal([(COMMAND, b"e\n")], reissued_command=COMMAND)
                self.assertFalse(Path((fx.root / "edit-path").read_text()).exists())
                self.assertFalse((fx.root / "receipt").exists())
                self.assertFalse((fx.root / "alternate").exists())

    # RFC §5.3-02 — y 只批准一条，同批下一条可单独拒绝。
    def test_yes_authorizes_only_one_call_in_a_batch(self):
        fx = self.fixture([[("one", "printf first > first"), ("two", "printf second > second")], []])
        self.assertEqual(fx.terminal([("printf first > first", b"y\n"),
                                      ("printf second > second", b"n\nnot this one\n")]), 0)
        first, second = fx.tools()
        self.assertEqual([r["tool_call_id"] for r in (first, second)], ["one", "two"])
        self.assertFalse(first.get("is_error", False))
        self.assertIs(second["is_error"], True)
        self.assertIn("Reason: not this one", second["text"])
        self.assertEqual((fx.root / "first").read_text(), "first")
        self.assertFalse((fx.root / "second").exists())

    # RFC §5.2-04 — 需交给人工处理且无控制终端时返回 75，不执行命令。
    # RFC §5.4-01 — 无控制终端时打印原命令、记录终止事件并退出 75。
    # RFC §6.5-06 — 无控制终端且需要人工审批时返回 75。
    def test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution(self):
        for external in (False, True):
            with self.subTest(external_escalation=external):
                fx = self.fixture(settings="safe_commands=[]\n" + ("bin='decide'\n" if external else ""))
                if external:
                    fx.decider("exit 2")
                result = fx.detached()
                self.assertEqual(result.returncode, 75, result.stderr)
                self.assertIn(COMMAND.encode(), result.stderr)
                self.assertFalse((fx.root / "receipt").exists())
                self.assertFalse((fx.root / "request-2").exists())
                self.assertEqual((fx.root / "frame").exists(), external)
                self.assert_cycle(fx, 75, "approval_blocked")

    # RFC §5.2-04 — 外部返回 2 后从控制终端读取最终选择。
    def test_external_escalation_hands_the_decision_to_the_terminal(self):
        for allow in (True, False):
            with self.subTest(allow=allow):
                fx = self.fixture(settings="safe_commands=[]\nbin='decide'\n")
                fx.decider("exit 2")
                exchanges = [(COMMAND, b"y\n")] if allow else [(COMMAND, b"n\nlater\n")]
                self.assertEqual(fx.terminal(exchanges), 0)
                self.assertEqual((fx.root / "decider-calls").read_text(), "called\n")
                self.assertEqual(rows(fx.root / "frame")[0]["cmd"], COMMAND)
                self.assertEqual((fx.root / "receipt").exists(), allow)
                self.assertEqual(fx.tools()[0].get("is_error", False), not allow)

    # RFC §5.2-01 — 外部裁决器读到一行 JSON、真实进程标识及 EOF。
    # RFC §5.2-02 — 外部返回 0，无控制终端也能执行一次已批准的命令。
    # RFC §5.2-06 — 外部裁决允许时 Trace 关联真实调用和裁决来源。
    def test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity(self):
        command = "printf '%s' 'café 雪' > receipt"
        fx = self.fixture([[("t", command)], []], "safe_commands=[]\nbin='decide'\n")
        fx.decider("printf ignored-on-allow; exit 0")
        result = fx.detached()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, b"complete")
        self.assertEqual((fx.root / "receipt").read_text(), "café 雪")
        frame, = rows(fx.root / "frame")
        event, = fx.trace("tool_approval")
        self.assertEqual({k: frame[k] for k in ("v", "cmd", "cwd", "agent", "pid", "ppid", "approval_mode")},
                         {"v": 1, "cmd": command, "cwd": str(fx.root), "agent": "approval-fixture",
                          "pid": event["pid"], "ppid": os.getpid(), "approval_mode": "ask"})
        self.assertEqual(event["ppid"], os.getpid())
        self.assertEqual((event["decider"], event["decider_exit_code"]), ("external", 0))
        self.assertNotIn("ignored-on-allow", fx.tools()[0]["text"])

    # RFC §5.2-03 — 外部返回 1，裁剪拒绝理由、为空时给出默认值，继续推理。
    # RFC §5.2-06 — 外部拒绝时保留对应来源及裁决结果。
    def test_external_denial_trims_reason_or_supplies_default_and_continues(self):
        for why, options in [("拒绝本轮", ()), ("", ()), ("explicit", ("--fail-on-deny",))]:
            with self.subTest(reason=why, options=options):
                fx = self.fixture(settings="safe_commands=[]\nbin='decide'\n")
                fx.decider(f"printf '  %s \\n' {shlex.quote(why)}; exit 1")
                result = fx.detached(*options)
                self.assertEqual(result.returncode, 0, result.stderr)
                outcome, = fx.tools()
                self.assertIs(outcome["is_error"], True)
                text = outcome["text"].split("\n", 1)[1]
                if why:
                    self.assertEqual(text, why)
                else:
                    self.assertTrue(text.strip())
                self.assertFalse((fx.root / "receipt").exists())
                event, = fx.trace("tool_approval")
                self.assertEqual((event["decider"], event["decider_exit_code"]), ("external", 1))
                self.assert_cycle(fx, 0, "completed")

    # RFC §5.2-05 — 其他非零或超时透出诊断，结束当前轮且不执行工具。
    def test_external_error_and_timeout_surface_stderr_and_end_the_cycle_with_75(self):
        for code in (3, 75, 127, None):
            with self.subTest(code=code):
                # Fast exits need no short deadline. For the timeout case the shell
                # emits its diagnostic before blocking, without spawning setup tools.
                limit = 4 if code is None else 15
                fx = self.fixture(settings=f"safe_commands=[]\nbin='decide'\ntimeout_sec={limit}\n")
                body = "exec /bin/sleep 60" if code is None else f"exit {code}"
                executable(fx.root / "decide", "#!/bin/sh\nIFS= read -r frame || exit 99\n"
                           "printf '判定器详情\\n' >&2\n" + body + "\n")
                result = fx.detached()
                self.assertEqual(result.returncode, 75, result.stderr)
                self.assertIn("判定器详情", result.stderr.decode())
                self.assertFalse((fx.root / "receipt").exists())
                self.assertFalse((fx.root / "request-2").exists())
                event, = fx.trace("tool_approval")
                self.assertEqual(event["decider"], "external")
                if code is not None:
                    self.assertEqual(event["decider_exit_code"], code)
                self.assert_cycle(fx, 75, "approval_blocked")

    # RFC §5.2-05 — 裁决器不可用时以 75 退出，Trace 记录审批失败且命令未执行。
    def test_unavailable_external_decider_exits_75_and_records_the_failed_approval(self):
        fx = self.fixture(settings="safe_commands=[]\nbin='missing-decider'\n")
        result = fx.detached()
        self.assertEqual(result.returncode, 75, result.stderr)
        self.assertTrue(result.stderr.strip())
        self.assertFalse((fx.root / "receipt").exists())
        self.assertFalse((fx.root / "request-2").exists())
        event, = fx.trace("tool_approval")
        self.assertEqual(event.get("decider"), "external", event)
        self.assert_cycle(fx, 75, "approval_blocked")

    # RFC §5.1-01 — auto 和白名单命令无需调用外部裁决器。
    # RFC §5.1-02 — deny 保留白名单，拦截时返回工具错误，开启 fail-on-deny 时退出 75。
    def test_auto_deny_and_allowlisted_commands_skip_the_external_decider(self):
        for policy, listed, fail, want_code, executes in [
            ("auto", False, False, 0, True), ("deny", False, False, 0, False),
            ("deny", False, True, 75, False), ("deny", True, True, 0, True),
            ("ask", True, False, 0, True),
        ]:
            with self.subTest(policy=policy, listed=listed, fail_on_deny=fail):
                fx = self.fixture([[("t", "printf approved")], []],
                                  f"safe_commands={json.dumps(['printf'] if listed else [])}\nbin='decide'\n")
                fx.decider("exit 9")
                options = ["--approval", policy] + (["--fail-on-deny"] if fail else [])
                result = fx.detached(*options)
                self.assertEqual(result.returncode, want_code, result.stderr)
                self.assertFalse((fx.root / "frame").exists())
                event, = fx.trace("tool_approval")
                self.assertEqual(event["decider"], "builtin")
                self.assertNotIn("decider_exit_code", event)
                if want_code == 0:
                    tool, = fx.tools()
                    self.assertEqual(tool.get("is_error", False), not executes)
                    self.assertIn("approved" if executes else "Error: Command denied by approval=deny policy",
                                  tool["text"])
                else:
                    self.assertFalse((fx.root / "request-2").exists())

    # RFC §6.4-02 — 在内置审批处只按一次 Ctrl+C，命令不执行，原 shell 恢复并读到退出码 130。
    def test_ctrl_c_at_the_approval_prompt_returns_to_the_same_shell(self):
        fx = self.fixture()
        self.assertEqual(fx.terminal([(COMMAND, b"\x03")]), 130)
        self.assertFalse((fx.root / "receipt").exists())
        self.assertFalse((fx.root / "request-2").exists())
        self.assert_cycle(fx, 130, "interrupted")

    # RFC §5.2-06 — 内置审批被 SIGINT 中断时 Trace 仍应标记 builtin。
    def test_interrupt_during_builtin_decision_identifies_the_decider_in_trace(self):
        fx = self.fixture()
        argv = ["/bin/sh", "-c", 'exec "$@" </dev/null >payload 2>diagnostic 7>trace',
                "fixture", *fx.argv(()), "--", "input"]
        with controlling_terminal(argv, fx.root, fx.env) as (master, process):
            read_until(master, COMMAND.encode())
            process.send_signal(signal.SIGINT)
            # Wake a reader already sleeping in /dev/tty (#49). This case
            # checks the audit record even when a single signal cannot unblock it.
            os.write(master, b"\n")
            deadline = time.monotonic() + 8
            while True:
                # Ignore an in-flight partial JSONL record. A Darwin session
                # leader may still be exiting until the PTY master is closed.
                complete = (fx.root / "trace").read_bytes().split(b"\n")[:-1]
                if any(json.loads(line)["type"] == "run_end" for line in complete):
                    break
                self.assertLess(time.monotonic(), deadline, "missing final trace record")
                time.sleep(.01)
        self.assertFalse((fx.root / "receipt").exists())
        self.assert_cycle(fx, 130, "interrupted")
        event, = fx.trace("tool_approval")
        self.assertEqual(event.get("decider"), "builtin", event)
        self.assertNotIn("decider_exit_code", event)

    # RFC §5.2-06 — 外部审批被 SIGTERM 中断时 Trace 仍应标记 external。
    # RFC §6.4-03 — 外部审批中收到 SIGTERM，退出 143 并记录中断。
    def test_interrupt_during_external_decision_identifies_the_decider_in_trace(self):
        fx = self.fixture(settings="safe_commands=[]\nbin='decide'\ntimeout_sec=20\n")
        fx.decider("exec /bin/sleep 12")
        with subprocess.Popen(["/bin/sh", "-c", 'exec "$@" 7>trace', "fixture", *fx.argv(())],
                              cwd=fx.root, env=fx.env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, start_new_session=True) as process:
            try:
                deadline = time.monotonic() + 8
                while not (fx.root / "frame").exists():
                    self.assertIsNone(process.poll())
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.01)
                process.send_signal(signal.SIGTERM)
                _, stderr = process.communicate(timeout=8)
                self.assertEqual(process.returncode, 143, stderr)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
        self.assertFalse((fx.root / "receipt").exists())
        self.assertFalse((fx.root / "request-2").exists())
        self.assert_cycle(fx, 143, "interrupted")
        event, = fx.trace("tool_approval")
        self.assertEqual(event.get("decider"), "external", event)
