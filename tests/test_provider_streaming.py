"""Exercise the real agent and Responses adapter against a paused loopback HTTP peer."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from http.server import HTTPServer

from http_fixture import JsonRequest


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

    def run_agent(self, level, replies):
        config = self.root / ".config/agent"
        config.mkdir(parents=True)
        with HTTPServer(("127.0.0.1", 0), StreamingResponse) as http:
            http.socket.settimeout(12)
            http.requests = []
            (config / "config.toml").write_text(
                "[defaults]\nmodel='local'\n[providers.openai-responses]\n"
                f"base_url='http://127.0.0.1:{http.server_port}/v1'\n"
                "api_key_env=''\napi_key='fixture'\ntimeout_sec=20\n")
            args = ["/bin/sh", "-c", 'exec "$@" 3>trace', "fixture", self.bin_dir / "agent",
                    "-q", "-c", "history", "case.agent", "--", "input"]
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


if __name__ == "__main__":
    unittest.main()
