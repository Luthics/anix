"""Compare the public lifecycle events to independently scripted requests and OS exits."""
import json
from pathlib import Path
import re
import signal
import subprocess

from anix_fixture import OfflineAnixCase, rfc_section


def rows(path):
    return [json.loads(line) for line in path.read_bytes().splitlines()]


def charge(input_count, output_count, cached, written, reasoning):
    return dict(input_tokens=input_count, output_tokens=output_count, cached_tokens=cached,
                cache_write_tokens=written, reasoning_tokens=reasoning)


def total(*charges):
    return {field: sum(entry[field] for entry in charges) for field in charges[0]}


def reply_frames(text):
    return [{"type": "text_delta", "text": text}, {"type": "done", "finish_reason": "stop"}]


TOOL_FRAMES = [{"type": "tool_call", "id": "reading", "name": "msh", "arguments": {"cmd": "printf fixture"}},
               {"type": "done", "finish_reason": "tool_calls"}]


class CycleAccounting(OfflineAnixCase):
    def traced(self, *args):
        trace_path = self.root / "trace.jsonl"
        with trace_path.open("wb") as trace:
            run = subprocess.run([self.anix, f"--trace-fd={trace.fileno()}", *args],
                                 pass_fds=(trace.fileno(),), cwd=self.root, env=self.env,
                                 stdin=subprocess.DEVNULL, capture_output=True, start_new_session=True, timeout=15)
        return run, rows(trace_path)

    def hook(self, stage, name, commands):
        path = self.config.parent / "hooks" / stage / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("#!/bin/sh\n" + commands + "\n")
        path.chmod(0o700)
        return path

    # RFC §9.5-15 — 逐个触发 RFC 列举的 cycle_end.reason，包括共享同一进程退出码的不同原因，并与 OS、run_end 对照。
    def test_each_declared_cycle_end_reason_matches_the_cause_and_os_status(self):
        declaration, = [line for line in rfc_section("9.5") if line.lstrip().startswith("* `cycle_end`")]
        domain = re.search(r"`reason`（([^）]+)）", declaration)
        self.assertIsNotNone(domain)
        declared = [json.loads(value.strip().strip("`")) for value in domain[1].split("|")]
        # max_turns vs max_tokens both exit 124; a local hook error and a provider error both exit 1.
        cases = [
            ("completed", 0, [], reply_frames("ok")),
            ("max_turns", 124, ["-y", "--max-turns=1"], TOOL_FRAMES),
            ("max_tokens", 124, ["--no-compact", "--max-tokens=1024"], reply_frames("unused")),
            ("approval_blocked", 75, ["--approval=deny", "--fail-on-deny"], TOOL_FRAMES),
            ("provider_error", 1, [], [{"type": "error", "code": "fixture", "message": "stop", "retryable": False}]),
            ("error", 1, ["-y"], TOOL_FRAMES),
            ("interrupted", 128 + signal.SIGTERM, [], []),
        ]
        observed = []
        for reason, status, options, frames in cases:
            with self.subTest(reason=reason):
                (self.config.parent / "hooks/post-tool.d/10-error").unlink(missing_ok=True)
                request = self.root / "request"
                request.unlink(missing_ok=True)
                self.script.write_text("X" * 9000 if reason == "max_tokens" else "reply")
                self.reply(frames)
                if reason == "error":
                    self.hook("post-tool.d", "10-error", "cat >/dev/null\nexit 3")
                if reason == "interrupted":
                    # Signal only the waiting parent, after the complete first request arrived.
                    self.provider.write_text('#!/bin/sh\ncat > request\nkill -TERM "$PPID"\nexec sleep 5\n')
                run, events = self.traced("-q", *options, self.script, "--", "go")
                self.assertEqual(request.exists(), reason != "max_tokens", run.stderr)
                cycle, = [e for e in events if e["type"] == "cycle_end"]
                end, = [e for e in events if e["type"] == "run_end"]
                # RFC-ASSERT §9.5-15
                self.assertEqual((cycle["reason"], cycle["exit_code"], end["exit_code"], run.returncode),
                                 (reason, status, status, status), run.stderr)
                observed.append(cycle["reason"])
        # A new, removed or repeated RFC reason must change this executable matrix.
        # RFC-ASSERT §9.5-15
        self.assertEqual(sorted(observed), sorted(declared))

    # RFC §9.5-16 — 对初始输入和两封 Inbox 消息分别核对起止配对、来源、递增编号以及最后一次 idle。
    # RFC §9.5-17 — 四次请求分属三个 cycle；每轮核对轮次数、五字段本轮和累计用量、会话总量及末次回答的水位。
    # RFC §9.5-18 — 将每个 llm_response 与实际发给 anix 的四组五字段 usage 逐项比对。
    # RFC §9.5-19 — 在队列排空并闲置退出后，比对 run_end 总轮数、进程用量、会话用量及最终水位。
    # RFC §9.3-04 — 同一批任务在无状态和恢复旧 WAL 时，on-exit 钩子均能读到独立核算的五项用量及最终水位。
    def test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals(self):
        history_charge = charge(101, 19, 42, 9, 7)
        calls = [charge(31, 11, 13, 3, 2), charge(47, 17, 19, 5, 3),
                 charge(67, 23, 29, 7, 5), charge(83, 29, 37, 11, 7)]
        self.hook("on-exit.d", "10-capture", 'cat > "$HOME/exit.json"')
        plan = [reply_frames("A"), TOOL_FRAMES, reply_frames("B"), reply_frames("C")]
        names = ["100-20-1", "200-20-2"]
        for persistent in (False, True):
            with self.subTest(persistent=persistent):
                wal = self.root / "history"
                options = ["-q", "-y", "--max-tokens=12000"]
                if persistent:
                    options.extend(["-c", wal])
                    self.reply([{"type": "usage", **history_charge}, *reply_frames("history")])
                    seed = self.invoke(*options, self.script, "--", "seed")
                    self.assertEqual(seed.returncode, 0, seed.stderr)
                inbox = self.root / ("persistent" if persistent else "stateless")
                for part in ("new", "tmp"):
                    (inbox / part).mkdir(parents=True)
                # Publish in reverse order so the source assertions distinguish sorting from creation order.
                for name in reversed(names):
                    staging = inbox / "tmp" / name
                    staging.write_text(name)
                    staging.rename(inbox / "new" / name)

                (self.root / "count").write_text("0")
                for i, (usage, frames) in enumerate(zip(calls, plan), 1):
                    events = [{"type": "usage", **usage}, *frames]
                    (self.root / f"reply-{i}").write_text("".join(json.dumps(e) + "\n" for e in events))
                self.provider.write_text('#!/bin/sh\nset -eu\nn=$(( $(cat count) + 1 ))\n'
                                         'echo "$n" > count\ncat > "request-$n"\ncat "reply-$n"\n')
                (self.root / "exit.json").unlink(missing_ok=True)
                run, events = self.traced(*options, "--inbox", inbox, "--idle-timeout=1", self.script, "--", "initial")
                self.assertEqual((run.returncode, run.stdout), (0, b"ABC"), run.stderr)
                self.assertEqual((self.root / "count").read_text().strip(), "4")
                sources = ["initial", *names]
                starts = [e for e in events if e["type"] == "cycle_start"]
                ends = [e for e in events if e["type"] == "cycle_end"]
                lifecycle = [e["type"] for e in events if e["type"] in ("cycle_start", "cycle_end", "idle", "run_end")]
                # RFC-ASSERT §9.5-16
                self.assertEqual(lifecycle, ["cycle_start", "cycle_end"] * len(sources) + ["idle", "run_end"])
                ids = [e["cycle_id"] for e in starts]
                # RFC-ASSERT §9.5-16
                self.assertEqual((ids[0], ids), (1, sorted(set(ids))))
                self.assertEqual([e["source"] for e in starts], sources)
                # RFC-ASSERT §9.5-16
                self.assertEqual([(e["cycle_id"], e["source"]) for e in ends], list(zip(ids, sources)))
                for e in events:
                    if e["type"] == "cycle_start":
                        self.assertIs(type(e["start_turn_id"]), int)
                    elif e["type"] == "idle":
                        self.assertEqual(e["idle_timeout_sec"], 1)
                run_start, = [e for e in events if e["type"] == "run_start"]
                self.assertEqual((run_start["fresh_session"], run_start["inbox_mode"]), (not persistent, True))
                responses = [e["usage"] for e in events if e["type"] == "llm_response"]
                # RFC-ASSERT §9.5-18
                self.assertEqual(responses, calls)
                expected_ends = []
                for first, stop in [(0, 1), (1, 3), (3, 4)]:
                    cycle_charge = total(*calls[first:stop])
                    process_charge = total(*calls[:stop])
                    session_charge = total(history_charge, process_charge) if persistent else process_charge
                    last = calls[stop - 1]
                    watermark = {"tokens": last["input_tokens"] + last["output_tokens"],
                                 "max_tokens": 12000}
                    expected_ends.append(("completed", 0, stop - first, cycle_charge, process_charge,
                                          session_charge, watermark))
                cycle_fields = ("reason", "exit_code", "turns", "cycle_usage", "usage", "session_usage", "context")
                # RFC-ASSERT §9.5-17
                self.assertEqual([tuple(e[k] for k in cycle_fields) for e in ends], expected_ends)
                final_fields = ("exit_code", "turns", "usage", "session_usage", "context")
                expected = (0, len(calls), *expected_ends[-1][4:])
                report = json.loads((self.root / "exit.json").read_bytes())
                # RFC-ASSERT §9.3-04
                self.assertEqual(tuple(report[k] for k in final_fields), expected)
                # RFC-ASSERT §9.5-19
                self.assertEqual(tuple(events[-1][k] for k in final_fields), expected)
                if persistent:
                    self.assertEqual(Path(report["context_file"]).resolve(), wal.resolve())
                    self.assertEqual([r["usage"] for r in rows(wal) if r.get("role") == "assistant"],
                                     [history_charge, *calls])
                for name in names:
                    self.assertEqual((inbox / "done" / name).read_text(), name)

    # RFC §9.3-05 — on-exit 钩子非零退出时，后续钩子仍收到相同的退出报告；失败只写 Trace，主进程保留成功或失败状态。
    def test_failed_exit_hook_preserves_reports_later_hooks_and_primary_status(self):
        self.hook("on-exit.d", "10-failing", "cat > before.json\necho hook-private-diagnostic >&2\nexit 17")
        self.hook("on-exit.d", "20-capture", "cat > after.json")
        for code, frames in [(0, reply_frames("deliver")),
                             (1, [{"type": "error", "code": "fixture", "message": "provider-failed", "retryable": False}])]:
            with self.subTest(primary_status=code):
                for name in ("before.json", "after.json"):
                    (self.root / name).unlink(missing_ok=True)
                self.reply(frames)
                run, events = self.traced("-q", self.script, "--", "input")
                before, after = [json.loads((self.root / name).read_bytes()) for name in ("before.json", "after.json")]
                # RFC-ASSERT §9.3-05
                self.assertEqual((run.returncode, before["exit_code"], after), (code, code, before))
                self.assertEqual(run.stdout, b"deliver" if code == 0 else b"")
                # RFC-ASSERT §9.3-05
                self.assertNotIn(b"hook-private-diagnostic", run.stderr)
                self.assertNotIn(b"10-failing", run.stderr)
                # RFC-ASSERT §9.3-05
                self.assertTrue(any("10-failing" in json.dumps(event) for event in events))
