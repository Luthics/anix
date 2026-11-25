"""Exercise the real agent and Responses adapter against a paused loopback HTTP peer."""
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import tempfile
import termios
import time
import unittest
from http.server import HTTPServer

from http_fixture import JsonRequest


@contextmanager
def interactive_shell(argv, cwd, env):
    """Give job control and keyboard signals to a disposable shell."""
    master, slave = pty.openpty()

    def acquire_terminal():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    try:
        shell = subprocess.Popen(argv, cwd=cwd, env=env, stdin=slave, stdout=slave,
                                 stderr=slave, preexec_fn=acquire_terminal)
        try:
            yield master
        finally:
            # Closing the master also releases a shell exiting on Darwin.
            os.close(master)
            master = -1
            try:
                shell.wait(timeout=2)
            except subprocess.TimeoutExpired:
                shell.kill()
                shell.wait(timeout=2)
    finally:
        os.close(slave)
        if master >= 0:
            os.close(master)


def read_until(master, marker):
    seen = bytearray()
    deadline = time.monotonic() + 8
    while marker not in seen:
        timeout = deadline - time.monotonic()
        if timeout <= 0 or not select.select([master], [], [], timeout)[0]:
            raise AssertionError(f"timed out waiting for {marker!r}: {bytes(seen)!r}")
        chunk = os.read(master, 8192)
        if not chunk:
            raise AssertionError(f"terminal closed before {marker!r}: {bytes(seen)!r}")
        seen.extend(chunk)
    return bytes(seen)


def accept_request(http, master):
    # A real terminal emulator drains the PTY even while the shell redraws
    # the submitted line. Keep servicing it until the HTTP request arrives.
    output = bytearray()
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        ready = select.select([http.socket, master], [], [], max(0, deadline - time.monotonic()))[0]
        if http.socket in ready:
            return http.get_request()
        if master in ready:
            output.extend(os.read(master, 8192))
    raise AssertionError(f"provider never reached loopback peer: {bytes(output)!r}")


class StreamingResponse(JsonRequest):
    protocol_version = "HTTP/1.1"

    def do_POST(self):
        self.close_connection = True
        self.server.requests.append(json.loads(self.body()))
        self.server.reply(self)

    def begin_stream(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Transfer-Encoding", "chunked")
        self.send_header("Connection", "close")
        self.end_headers()

    def event(self, data):
        line = b"data: " + json.dumps(data, ensure_ascii=False).encode() + b"\r\n\r\n"
        # Split the transport inside the first non-ASCII codepoint. SSE frames
        # and HTTP chunks have independent boundaries.
        split = next((i + 1 for i, byte in enumerate(line) if byte >= 128), len(line) // 2)
        for part in (line[:split], line[split:]):
            self.wfile.write(f"{len(part):x}\r\n".encode() + part + b"\r\n")

    def finish_stream(self):
        self.event({"type": "response.completed", "response": {"usage": {"input_tokens": 8, "output_tokens": 3}}})
        self.wfile.write(b"0\r\n\r\n")

    def msh_call(self, cmd, fragment_delay=0):
        arguments = json.dumps({"cmd": cmd})
        item = {"type": "function_call", "id": "item1", "call_id": "call1", "name": "msh"}
        self.event({"type": "response.output_item.added", "output_index": 0,
                    "item": {**item, "arguments": ""}})
        for start in range(0, len(arguments), 1024):
            if start and fragment_delay:
                time.sleep(fragment_delay)
            self.event({"type": "response.function_call_arguments.delta", "item_id": "item1",
                        "output_index": 0, "delta": arguments[start:start + 1024]})
        self.event({"type": "response.output_item.done", "output_index": 0,
                    "item": {**item, "arguments": arguments}})


class ProviderStreaming(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="agent-stream-rfc-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.bin_dir = Path(os.environ.get("OS_AGENT_BIN_DIR", Path(__file__).resolve().parents[1] / "target/debug"))
        (self.root / "case.agent").write_text("stream fixture")
        (self.root / "trace").touch(mode=0o600)

    def rows(self, name):
        # A concurrently written line is observable only once its LF arrives.
        return [json.loads(line) for line in (self.root / name).read_bytes().split(b"\n")[:-1]]

    def run_agent(self, level, replies, options=(), provider_timeout=20):
        config = self.root / ".config/agent"
        config.mkdir(parents=True)
        with HTTPServer(("127.0.0.1", 0), StreamingResponse) as http:
            http.socket.settimeout(12)
            http.requests = []
            (config / "config.toml").write_text(
                "[defaults]\nmodel='local'\n[providers.openai-responses]\n"
                f"base_url='http://127.0.0.1:{http.server_port}/v1'\n"
                f"api_key_env=''\napi_key='fixture'\ntimeout_sec={provider_timeout}\n")
            args = ["/bin/sh", "-c", 'exec "$@" 3>trace', "fixture", self.bin_dir / "agent",
                    "-q", "-c", "history", *options, "case.agent", "--", "input"]
            with subprocess.Popen(args, cwd=self.root, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, env={"HOME": str(self.root), "PATH": f"{self.bin_dir}:/bin:/usr/bin",
                                  "XDG_RUNTIME_DIR": str(self.root / "run"), "AGENT_TRACE_LEVEL": level}) as child:
                try:
                    for reply in replies:
                        http.reply = reply
                        conn, address = http.get_request()
                        with conn:
                            conn.settimeout(8)
                            # Dispatch on this thread so assertions inside the
                            # paused HTTP handler are visible to unittest.
                            StreamingResponse(conn, address, http)
                    out, err = child.communicate(timeout=12)
                finally:
                    if child.poll() is None:
                        child.kill()
                        child.communicate()
            self.assertEqual((child.returncode, out, err), (0, b"", b""))
            return http.requests

    # RFC §6.4, §7.1, §9.2, §9.5
    def test_long_msh_can_outlive_the_provider_timeout_between_two_requests(self):
        prefix = "printf started > marker\n#"
        suffix = "\n/bin/sleep 4\nprintf finished"
        cmd = prefix.ljust(32768 - len(suffix), "x") + suffix

        def call(peer):
            peer.begin_stream()
            peer.msh_call(cmd)
            peer.finish_stream()

        def finish(peer):
            self.assertEqual((self.root / "marker").read_text(), "started")
            peer.begin_stream()
            peer.event({"type": "response.output_text.delta", "delta": "done"})
            peer.finish_stream()

        requests = self.run_agent("events", [call, finish], options=("-y",), provider_timeout=2)
        self.assertEqual(len(requests), 2)
        calls = [i for i in requests[1]["input"] if i["type"] == "function_call"]
        self.assertEqual([json.loads(i["arguments"]) for i in calls], [{"cmd": cmd}])
        results = [i for i in requests[1]["input"] if i["type"] == "function_call_output"]
        tools = [r for r in self.rows("history") if r.get("role") == "tool"]
        self.assertEqual(len(tools), 1)
        self.assertFalse(tools[0].get("is_error", False))
        expected = f"[t{tools[0]['turn_id']}] exit=0\nfinished"
        self.assertEqual(tools[0]["text"], expected)
        self.assertEqual([(i["call_id"], i["output"]) for i in results], [("call1", expected)])
        observed = {"llm_request_start", "tool_start", "tool_end", "provider_error"}
        self.assertEqual([e["type"] for e in self.rows("trace") if e["type"] in observed],
                         ["llm_request_start", "tool_start", "tool_end", "llm_request_start"])

    # RFC §7.1, §9.2, §9.5
    def test_paced_msh_arguments_finish_before_the_shell_runs(self):
        suffix = "\nprintf ok"
        cmd = "printf x >> marker\n#".ljust(32768 - len(suffix), "a") + suffix

        def call(peer):
            peer.begin_stream()
            peer.msh_call(cmd, fragment_delay=.15)
            # A complete function_call has arrived, but the Provider turn is still open.
            self.assertFalse((self.root / "marker").exists())
            self.assertNotIn("tool_start", [e["type"] for e in self.rows("trace")])
            peer.finish_stream()

        def finish(peer):
            self.assertEqual((self.root / "marker").read_text(), "x")
            peer.begin_stream()
            peer.event({"type": "response.output_text.delta", "delta": "done"})
            peer.finish_stream()

        requests = self.run_agent("events", [call, finish], options=("-y",), provider_timeout=12)
        outputs = [i for i in requests[1]["input"] if i["type"] == "function_call_output"]
        self.assertEqual(len(outputs), 1)
        self.assertEqual(outputs[0]["call_id"], "call1")
        self.assertTrue(outputs[0]["output"].endswith("exit=0\nok"))
        self.assertFalse(any(e["type"] == "provider_error" for e in self.rows("trace")))

    # RFC §4.1, §6.2, §9.2, §9.5
    def test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame(self):
        sent = [("response.reasoning_summary_text.delta", "reasoning_delta", "先看"),
                ("response.output_text.delta", "text_delta", "前🙂"),
                ("response.output_text.delta", "text_delta", "\n後")]

        def reply(peer):
            peer.begin_stream()
            expected = []
            for upstream, kind, value in sent:
                peer.event({"type": upstream, "delta": value})
                expected.append((kind, value))
                deadline = time.monotonic() + 5
                while True:
                    trace = self.rows("trace")
                    observed = [(r["type"], r["text"]) for r in trace if r["type"] in ("text_delta", "reasoning_delta")]
                    if len(observed) >= len(expected):
                        break
                    self.assertLess(time.monotonic(), deadline, f"waiting for {expected!r}; received {observed!r}")
                    time.sleep(.01)
                self.assertEqual(observed, expected)
                self.assertFalse(any(r["type"] in ("llm_response", "run_end") for r in trace))
                self.assertFalse(any(r.get("role") == "assistant" for r in self.rows("history")))
            peer.finish_stream()

        self.assertEqual(len(self.run_agent("deltas", [reply])), 1)
        assistant = [r for r in self.rows("history") if r.get("role") == "assistant"]
        self.assertEqual([(r["text"], r["reasoning"]) for r in assistant], [("前🙂\n後", "先看")])
        trace = self.rows("trace")
        self.assertEqual(trace[-1]["type"], "run_end")
        usage = [r["usage"] for r in trace if r["type"] == "llm_response"]
        self.assertEqual([(u["input_tokens"], u["output_tokens"]) for u in usage], [(8, 3)])

    # RFC §4.3, §6.2, §9.1, §9.2
    def test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results(self):
        reasoning = {"type": "reasoning", "id": "reason-1", "summary": [],
                     "encrypted_content": "local-fixture+/="}
        calls = [
            {"type": "function_call", "id": f"item-{name}", "name": "msh", "call_id": f"call-{name}",
             "arguments": json.dumps({"cmd": f"printf {name}"})}
            for name in ("red", "blue")
        ]

        def tools(peer):
            peer.begin_stream()
            peer.event({"type": "response.output_item.done", "output_index": 0, "item": reasoning})
            for index, item in enumerate(calls, 1):
                peer.event({"type": "response.output_item.added", "output_index": index,
                            "item": {**item, "arguments": ""}})
            for part in (0, 1):
                for index, item in enumerate(calls, 1):
                    args = item["arguments"]
                    cut = len(args) // 2
                    peer.event({"type": "response.function_call_arguments.delta", "item_id": item["id"],
                                "output_index": index, "delta": args[:cut] if part == 0 else args[cut:]})
            # Both completion notifications can describe a call. Finishing
            # them in reverse order distinguishes the item ID from arrival order.
            for index, item in reversed(list(enumerate(calls, 1))):
                peer.event({"type": "response.function_call_arguments.done", "item_id": item["id"],
                            "output_index": index, "arguments": item["arguments"]})
                peer.event({"type": "response.output_item.done", "output_index": index, "item": item})
            peer.event({"type": "response.completed", "response": {"output": [reasoning, *calls]}})
            peer.wfile.write(b"0\r\n\r\n")

        def finish(peer):
            peer.begin_stream()
            peer.event({"type": "response.output_text.delta", "delta": "both complete"})
            peer.finish_stream()

        requests = self.run_agent("events", [tools, finish], options=("-y", "--shell", "/bin/sh"))
        self.assertEqual(len(requests), 2)
        items = requests[1]["input"]
        self.assertEqual([i for i in items if i["type"] == "reasoning"], [reasoning])
        forwarded = [i for i in items if i["type"] == "function_call"]
        self.assertCountEqual([(i["call_id"], i["name"], json.loads(i["arguments"])) for i in forwarded],
                              [(i["call_id"], i["name"], json.loads(i["arguments"])) for i in calls])
        results = [i for i in items if i["type"] == "function_call_output"]
        self.assertCountEqual([i["call_id"] for i in results], [i["call_id"] for i in calls])
        wal = self.rows("history")
        assistants = [r for r in wal if r.get("role") == "assistant"]
        self.assertEqual(len(assistants), 2)
        self.assertIsInstance(assistants[0].get("provider_opaque"), dict)
        self.assertTrue(assistants[0]["provider_opaque"])
        self.assertEqual(assistants[1]["text"], "both complete")
        tool_rows = [r for r in wal if r.get("role") == "tool"]
        self.assertCountEqual([r["tool_call_id"] for r in tool_rows], [i["call_id"] for i in calls])
        outputs = {i["call_id"]: i["output"] for i in results}
        for row in tool_rows:
            call_id = row["tool_call_id"]
            self.assertEqual(outputs[call_id], row["text"])
            self.assertEqual(row["text"], f"[t{row['turn_id']}] exit=0\n{call_id.removeprefix('call-')}")

    # RFC §6.2, §9.2, §9.5
    def test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request(self):
        def reject(status):
            def reply(peer):
                body = f"loopback status {status}".encode()
                peer.send_response(status)
                peer.send_header("Content-Length", str(len(body)))
                peer.send_header("Connection", "close")
                peer.end_headers()
                peer.wfile.write(body)
            return reply

        def success(peer):
            failures = [r for r in self.rows("trace") if r["type"] == "provider_error"]
            self.assertEqual([r["attempt"] for r in failures], [1, 2])
            for row, status in zip(failures, [429, 503]):
                self.assertTrue(row["code"])
                self.assertIn(f"loopback status {status}", row["provider_stderr"])
            self.assertFalse(any(r.get("role") == "assistant" for r in self.rows("history")))
            peer.begin_stream()
            peer.event({"type": "response.output_text.delta", "delta": "success"})
            peer.finish_stream()

        requests = self.run_agent("events", [reject(429), reject(503), success])
        self.assertEqual(len(requests), 3)
        self.assertEqual(requests[1:], [requests[0], requests[0]])
        self.assertEqual([r["text"] for r in self.rows("history") if r.get("role") == "assistant"], ["success"])
        self.assertFalse(any(r["type"] in ("text_delta", "reasoning_delta") for r in self.rows("trace")))


    # RFC §3.1, §4.1, §6.3, §6.4, §9.5
    def test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider(self):
        example = Path(__file__).resolve().parents[1] / "examples/bash.agent"
        (self.root / "bash.agent").symlink_to(example)
        config = self.root / ".config/agent"
        config.mkdir(parents=True)
        prompt = b"RFC-ready> "
        env = {"HOME": str(self.root), "PATH": f"{self.bin_dir}:/usr/bin:/bin",
               "TERM": "dumb", "RFC_PROMPT": prompt.decode(), "AGENT_TRACE_LEVEL": "events",
               "XDG_RUNTIME_DIR": str(self.root / "run")}
        shells = [["/bin/bash", "--noprofile", "--norc", "-i"]]
        if Path("/bin/zsh").is_file():
            shells.append(["/bin/zsh", "-f", "-i"])
        with HTTPServer(("127.0.0.1", 0), StreamingResponse) as http:
            http.socket.settimeout(8)
            http.requests = []
            (config / "config.toml").write_text(
                "[providers.openai-responses]\n"
                f"base_url='http://127.0.0.1:{http.server_port}/v1'\n"
                "api_key_env=''\napi_key='fixture'\ntimeout_sec=20\n")
            for argv in shells:
                with self.subTest(shell=argv[0]), interactive_shell(argv, self.root, env) as master:
                    os.write(master, b"stty -g > before; PS1=$RFC_PROMPT\n")
                    read_until(master, prompt)
                    for pipeline in (False, True):
                        with self.subTest(pipeline=pipeline):
                            cmd = b'./bash.agent "xxx" 3>trace'
                            os.write(master, cmd + (b" | cat" if pipeline else b"") + b"\n")

                            def interrupt(peer):
                                # Receiving the POST without keyboard input proves
                                # the argument has already passed through the TTY.
                                users = [i["content"] for i in http.requests[-1]["input"] if i.get("role") == "user"]
                                self.assertEqual(users[-1], [{"type": "input_text", "text": "<args>xxx</args>"}])
                                peer.begin_stream()
                                peer.event({"type": "response.created", "response": {}})
                                os.write(master, b"\x03")
                                output = read_until(master, prompt)
                                self.assertEqual(output.count(b"[usage]"), 1, output)
                                self.assertNotIn(b"Broken pipe", output)

                            http.reply = interrupt
                            conn, addr = accept_request(http, master)
                            with conn:
                                conn.settimeout(8)
                                StreamingResponse(conn, addr, http)
                            os.write(master, b'printf "%s" "$?" | cat > result; stty -g > after\n')
                            read_until(master, prompt)
                            self.assertEqual((self.root / "result").read_text(), "130")
                            self.assertEqual((self.root / "after").read_bytes(), (self.root / "before").read_bytes())
                            events = self.rows("trace")
                            self.assertEqual([e["type"] for e in events].count("llm_request_start"), 1)
                            self.assertFalse(any(e["type"] in ("llm_response", "tool_start") for e in events))
                            self.assertEqual((events[-1]["type"], events[-1]["exit_code"]), ("run_end", 130))
                    os.write(master, b"exit\n")

    # RFC §4.1, §6.3, §6.4
    def test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit(self):
        (self.root / "bash.agent").symlink_to(Path(__file__).resolve().parents[1] / "examples/bash.agent")
        config = self.root / ".config/agent"
        config.mkdir(parents=True)
        (config / "config.toml").write_text("[providers.openai-responses]\nbin='local-provider'\n")
        provider = self.root / "local-provider"
        provider.write_text("""#!/bin/sh
/bin/cat > request
trap 'printf term > stopped; exit 0' TERM
printf '%s\\n' '{"type":"text_delta","text":"fixture-final"}' '{"type":"done","finish_reason":"stop"}'
exec 1>&-
/bin/sleep 0.25
printf ready > stdout-closed
/bin/sleep 5
printf finished > natural-exit
""")
        provider.chmod(0o700)
        prompt = b"RFC-ready> "
        env = {"HOME": str(self.root), "PATH": f"{self.root}:{self.bin_dir}:/usr/bin:/bin",
               "TERM": "dumb", "RFC_PROMPT": prompt.decode(), "XDG_RUNTIME_DIR": str(self.root / "run")}
        with interactive_shell(["/bin/bash", "--noprofile", "--norc", "-i"], self.root, env) as master:
            os.write(master, b"PS1=$RFC_PROMPT\n")
            read_until(master, prompt)
            os.write(master, b'./bash.agent "xxx"\n')
            read_until(master, b"fixture-final")
            # Interrupt during child cleanup regardless of how agent formats
            # the final delta. Allow its pipe reader time to observe EOF.
            deadline = time.monotonic() + 4
            while not (self.root / "stdout-closed").exists():
                self.assertLess(time.monotonic(), deadline, "provider did not close stdout")
                time.sleep(.01)
            os.write(master, b"\x03")
            output = read_until(master, prompt)
            os.write(master, b'printf "%s" "$?" | cat > result\n')
            read_until(master, prompt)
            os.write(master, b"exit\n")
        self.assertEqual(((self.root / "stopped").exists(), (self.root / "natural-exit").exists(),
                          (self.root / "result").read_text()), (True, False, "130"), output)


if __name__ == "__main__":
    unittest.main()
