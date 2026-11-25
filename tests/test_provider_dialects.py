"""Exercise the installed provider wire formats on loopback, including coprocess replay."""
import base64
from collections import deque
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
import uuid
from http.server import HTTPServer

from http_fixture import JsonRequest


DIALECTS = ("openai-responses", "openai-compat", "anthropic")
HERE = Path(__file__).resolve().parent
SAMPLES = HERE / "fixtures/media"
TERMINAL = {"done", "error"}


def sse(*payloads):
    return b"".join(b"data: " + json.dumps(p, ensure_ascii=False).encode() + b"\r\n\r\n"
                    for p in payloads)


def completion(dialect):
    if dialect == "openai-responses":
        return sse({"type": "response.completed", "response": {}})
    if dialect == "openai-compat":
        return b"data: [DONE]\r\n\r\n"
    return sse({"type": "message_delta", "delta": {"stop_reason": "end_turn"}},
               {"type": "message_stop"})


def header(**fields):
    return {"tools": [], "meta": {"cwd": "/", "stderr_mode": "explicit"}, **fields}


def envelope(number, messages, **fields):
    return {"ir_version": 1, "type": "request", "request_id": str(number),
            "header": header(), "messages": messages, **fields}


def user(text, **fields):
    return {"role": "user", "turn_id": 1, "text": text, **fields}


class AdapterHTTP(JsonRequest):
    def do_POST(self):
        self.server.requests.append(json.loads(self.body()))
        self.server.request_headers.append(self.headers)
        if self.server.replies:
            status, body, after_read = self.server.replies.popleft()
            if after_read:
                after_read()
        else:
            # An extra dispatch remains observable even for an unexpected retry.
            status, body = 400, b"unexpected HTTP request"
        self.send_response(status)
        self.send_header("Content-Type", "text/event-stream" if status == 200 else "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def pictures(node):
    """Read actual image blocks, retaining wire order, MIME and decoded bytes."""
    if isinstance(node, list):
        for item in node:
            yield from pictures(item)
    elif isinstance(node, dict):
        kind = node.get("type")
        if kind in ("image_url", "input_image"):
            url = node["image_url"]
            if isinstance(url, dict):
                url = url["url"]
            if not url.startswith("data:"):
                raise ValueError("expected an inline image data URI")
            media_type, data = url[5:].split(";base64,")
            yield media_type, base64.b64decode(data, validate=True)
        elif kind == "image":
            source = node["source"]
            if source["type"] != "base64":
                raise ValueError("expected a base64 image source")
            yield source["media_type"], base64.b64decode(source["data"], validate=True)
        else:
            for item in node.values():
                yield from pictures(item)


class ProviderDialects(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="anix-dialect-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.binary = Path(os.environ.get("ANIX_TEST_BIN_DIR", HERE.parent / "target/debug")).resolve() / "anix-provider"
        self.http = HTTPServer(("127.0.0.1", 0), AdapterHTTP)
        self.http.requests = []
        self.http.request_headers = []
        self.http.replies = deque()
        worker = threading.Thread(target=self.http.serve_forever, kwargs={"poll_interval": 0.01})
        worker.start()
        self.addCleanup(self.http.server_close)
        self.addCleanup(worker.join)
        self.addCleanup(self.http.shutdown)

    def run_provider(self, dialect, mode, rows, replies, session_env=None):
        self.http.requests.clear()
        self.http.request_headers.clear()
        self.http.replies = deque(replies)
        env = {"HOME": str(self.root), "ANIX_PROVIDER_MODE": mode, "ANIX_MODEL": "default-model",
               "ANIX_PROVIDER_BASE_URL": f"http://127.0.0.1:{self.http.server_port}/v1",
               "ANIX_PROVIDER_TIMEOUT_SEC": "5", "ANIX_PROVIDER_API_KEY": "fixture-only"}
        if session_env is not None:
            env["ANIX_SESSION_ID"] = session_env
        payload = b"".join(json.dumps(row, ensure_ascii=False).encode() + b"\n" for row in rows)
        result = subprocess.run([self.binary, dialect], env=env, cwd=self.root, input=payload,
                                capture_output=True, timeout=15)
        if result.stdout:
            self.assertTrue(result.stdout.endswith(b"\n"), result.stdout)
        events = [json.loads(line) for line in result.stdout.decode().splitlines()]
        self.assertFalse(self.http.replies, "adapter did not consume the expected HTTP replies")
        self.assertEqual(len(self.http.requests), len(replies), "unexpected HTTP dispatch or retry")
        return result, events, list(self.http.requests)

    def oneshot(self, dialect, messages, replies, **fields):
        return self.run_provider(dialect, "oneshot", [
            {"type": "header", "ir_version": 1, **header(**fields)}, *messages], replies)

    def assert_terminals(self, events, kinds):
        self.assertEqual([e["type"] for e in events if e["type"] in TERMINAL], kinds, events)
        self.assertEqual(events[-1]["type"], kinds[-1], events)

    # RFC v1.1 §6.2, §9.1, §9.2
    def test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream(self):
        frozen = "  冻结\r\n\n前缀\t "
        update = "<runtime_update>\n切换\t</runtime_update>"
        messages = [{"role": "system", "turn_id": 0, "text": frozen}, user("first"),
                    {"role": "assistant", "turn_id": 2, "text": "answer"},
                    {"role": "system", "turn_id": 3, "text": update},
                    {"role": "user", "turn_id": 4, "text": "next"}]
        for dialect in DIALECTS:
            with self.subTest(dialect=dialect):
                proc, events, (body,) = self.oneshot(
                    dialect, messages, [(200, completion(dialect), None)], max_output_tokens=37,
                    x_fixture=10, params={"x_unsupported_fixture": {"nested": True}})
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["done"])
                self.assertNotIn("tools", body)
                self.assertEqual(body["model"], "default-model")
                if dialect == "openai-responses":
                    self.assertEqual(body["instructions"], frozen)
                    items = [(m["role"], b["text"]) for m in body["input"] for b in m["content"]]
                    cap = body["max_output_tokens"]
                elif dialect == "openai-compat":
                    first, *history = body["messages"]
                    self.assertEqual((first["role"], first["content"]), ("system", frozen))
                    items = [(m["role"], m["content"]) for m in history]
                    cap = body.get("max_completion_tokens", body.get("max_tokens"))
                else:
                    self.assertEqual("".join(b["text"] for b in body["system"]), frozen)
                    items = [(m["role"], b["text"]) for m in body["messages"] for b in m["content"]]
                    cap = body["max_tokens"]
                self.assertEqual(cap, 37)
                self.assertEqual([text for _, text in items], ["first", "answer", update, "next"])
                self.assertEqual([role for role, _ in (items[0], items[1], items[3])],
                                 ["user", "assistant", "user"])
                self.assertIn(items[2][0], ["user", "system", "developer"] if dialect != "anthropic" else ["user"])

    # RFC v1.1 §9.1, §9.2
    def test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes(self):
        samples = [(f"image/{ext}", (SAMPLES / f"blue.{ext}").read_bytes())
                   for ext in ("webp", "png", "jpeg", "gif")]
        attachments = []
        for i, (mime, data) in enumerate(samples):
            path = self.root / f"图 {i}"
            path.write_bytes(data)
            attachments.append({"path": str(path), "mime": mime})
        messages = [user("user images", media=attachments[:2]),
                    {"role": "assistant", "turn_id": 2,
                     "tool_calls": [{"id": "A", "name": "msh", "arguments": {"cmd": "true"}}]},
                    {"role": "tool", "turn_id": 3, "tool_call_id": "A", "text": "tool images",
                     "media": attachments[2:]}]
        for dialect in DIALECTS:
            with self.subTest(dialect=dialect):
                proc, events, (body,) = self.oneshot(
                    dialect, messages, [(200, completion(dialect), None)],
                    tools=[{"name": "msh", "description": "fixture", "parameters": {"type": "object"}}])
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["done"])
                history = body["input" if dialect == "openai-responses" else "messages"]
                self.assertEqual(list(pictures(history[0])), samples[:2])
                self.assertEqual(list(pictures(history[1:])), samples[2:])
                for attachment in attachments:
                    self.assertNotIn(attachment["path"], json.dumps(body, ensure_ascii=False))

    # RFC v1.1 §9.1, §9.2
    def test_coprocess_reopens_replaced_media_and_recovers_after_unlink(self):
        path = self.root / "same image"
        blue = (SAMPLES / "blue.gif").read_bytes()
        # Entry zero of the GIF global color table starts at byte 13.
        gold = blue[:13] + bytes.fromhex("ffd700") + blue[16:]
        img = [user("", media=[{"mime": "image/gif", "path": str(path)}])]
        for dialect in DIALECTS:
            with self.subTest(dialect=dialect):
                path.write_bytes(blue)
                frames = [envelope(i, img, model=f"round-{i}") for i in (1, 2, 3)]
                frames.append(envelope(4, [user("still alive")]))
                ok = completion(dialect)
                proc, events, bodies = self.run_provider(dialect, "coprocess", frames, [
                    (200, ok, lambda: path.write_bytes(gold)), (200, ok, path.unlink), (200, ok, None)])
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["done", "done", "error", "done"])
                error, = [e for e in events if e["type"] == "error"]
                self.assertEqual((error["code"], error["retryable"]), ("media_not_found", False))
                self.assertEqual([b["model"] for b in bodies], ["round-1", "round-2", "default-model"])
                self.assertEqual([list(pictures(b)) for b in bodies],
                                 [[("image/gif", blue)], [("image/gif", gold)], []])

    # RFC v1.1 §6.5, §9.1, §9.2
    def test_oneshot_unreadable_and_missing_paths_fail_before_dispatch(self):
        denied = self.root / "private"
        denied.write_bytes((SAMPLES / "blue.png").read_bytes())
        denied.chmod(0)
        paths = [self.root / "missing", self.root]
        if not os.access(denied, os.R_OK):
            paths.append(denied)
        for dialect in DIALECTS:
            for path in paths:
                with self.subTest(dialect=dialect, path=path):
                    proc, events, _ = self.oneshot(dialect, [user(
                        "image", media=[{"path": str(path), "mime": "image/png"}])], [])
                    self.assertEqual(proc.returncode, 1, proc.stderr)
                    self.assert_terminals(events, ["error"])
                    self.assertEqual(events[-1]["code"], "media_not_found")
                    self.assertIs(events[-1]["retryable"], False)

    # RFC v1.1 §5.5, §9.2
    def test_upstream_media_rejection_is_distinct_from_auth_and_transient_failures(self):
        image = user("see", media=[{"mime": "image/png", "path": str(SAMPLES / "blue.png")}])
        cases = [(400, image, "model does not support image inputs", False, True),
                 (400, user("hi"), "invalid argument", False, False),
                 (401, image, "authentication required", False, False),
                 (403, image, "permission denied", False, False),
                 (429, image, "too many requests", True, False),
                 (500, image, "server error", True, False),
                 (503, image, "service unavailable", True, False)]
        for dialect in DIALECTS:
            with self.subTest(dialect=dialect):
                frames = [envelope(i, [msg]) for i, (_, msg, _, _, _) in enumerate(cases)]
                replies = [(status, json.dumps({"error": {"message": why}}).encode(), None)
                           for status, _, why, _, _ in cases]
                frames.append(envelope(len(cases), [user("recover")]))
                replies.append((200, completion(dialect), None))
                proc, events, _ = self.run_provider(dialect, "coprocess", frames, replies)
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["error"] * len(cases) + ["done"])
                errors = [e for e in events if e["type"] == "error"]
                for (status, _, _, retryable, unsupported), error in zip(cases, errors):
                    with self.subTest(status=status, retryable=retryable):
                        self.assertIs(error["retryable"], retryable)
                        self.assertEqual(error["code"] == "unsupported_media", unsupported)
                        self.assertTrue(error["message"])

    # RFC v1.1 §9.2
    def test_partial_text_then_upstream_eof_yields_error_and_coprocess_accepts_next_request(self):
        partials = {
            "openai-responses": sse({"type": "response.output_text.delta", "delta": "半"}),
            "openai-compat": sse({"choices": [{"index": 0, "delta": {"content": "半"}}]}),
            "anthropic": sse({"type": "content_block_start", "index": 0,
                              "content_block": {"type": "text", "text": ""}},
                             {"type": "content_block_delta", "index": 0,
                              "delta": {"type": "text_delta", "text": "半"}})}
        for dialect, partial in partials.items():
            for mode in ("oneshot", "coprocess"):
                with self.subTest(dialect=dialect, mode=mode):
                    replies = [(200, partial, None)]
                    if mode == "oneshot":
                        proc, events, _ = self.oneshot(dialect, [user("hi")], replies)
                        self.assertNotEqual(proc.returncode, 0)
                        kinds = ["error"]
                    else:
                        proc, events, _ = self.run_provider(dialect, mode,
                            [envelope(1, [user("hi")]), envelope(2, [user("next")])],
                            replies + [(200, completion(dialect), None)])
                        self.assertEqual(proc.returncode, 0, proc.stderr)
                        kinds = ["error", "done"]
                    self.assert_terminals(events, kinds)
                    first_error = next(i for i, e in enumerate(events) if e["type"] == "error")
                    self.assertEqual("".join(e["text"] for e in events[:first_error]
                                             if e["type"] == "text_delta"), "半")


    # RFC v1.1 §6.5, §9.1, §9.2
    def test_unsupported_ir_version_is_diagnosed_before_http_in_both_modes(self):
        for dialect in DIALECTS:
            for mode in ("oneshot", "coprocess"):
                with self.subTest(dialect=dialect, mode=mode):
                    if mode == "oneshot":
                        rows = [{"type": "header", "ir_version": 2, **header()}, user("bad version")]
                    else:
                        rows = [envelope(1, [user("bad version")], ir_version=2)]
                    proc, events, _ = self.run_provider(dialect, mode, rows, [])
                    self.assertTrue(proc.stderr.strip())
                    self.assert_terminals(events, ["error"])
                    self.assertTrue(events[-1]["message"])
                    if mode == "oneshot":
                        self.assertEqual(proc.returncode, 2)

    # RFC v1.1 §9.1, §9.2
    def test_fragmented_tool_arguments_produce_one_call_with_the_original_id(self):
        call_id = "call-42"
        parts = ['{"cmd":', '"echo 雪"}']
        item = {"id": "item-7", "type": "function_call", "call_id": call_id, "name": "msh"}
        responses = [{"type": "response.output_item.added", "output_index": 0,
                      "item": {**item, "arguments": ""}}]
        responses.extend({"type": "response.function_call_arguments.delta", "item_id": item["id"],
                          "output_index": 0, "delta": part} for part in parts)
        whole = {**item, "arguments": "".join(parts)}
        responses.extend([{"type": "response.function_call_arguments.done", "item_id": item["id"],
                           "output_index": 0, "arguments": whole["arguments"]},
                          {"type": "response.output_item.done", "output_index": 0, "item": whole},
                          {"type": "response.completed", "response": {"output": [whole]}}])

        def chat_call(call):
            return {"choices": [{"index": 0, "delta": {"tool_calls": [call]}}]}

        chat = [chat_call({"index": 0, "id": call_id, "type": "function",
                           "function": {"name": "msh", "arguments": parts[0]}}),
                chat_call({"index": 0, "function": {"arguments": parts[1]}}),
                {"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]}]
        anthropic = [{"type": "content_block_start", "index": 0,
                      "content_block": {"type": "tool_use", "id": call_id, "name": "msh", "input": {}}}]
        anthropic.extend({"type": "content_block_delta", "index": 0, "delta": {
            "type": "input_json_delta", "partial_json": part}} for part in parts)
        anthropic.extend([{"type": "content_block_stop", "index": 0},
                          {"type": "message_delta", "delta": {"stop_reason": "tool_use"}},
                          {"type": "message_stop"}])
        streams = {"openai-responses": sse(*responses), "openai-compat": sse(*chat) + completion("openai-compat"),
                   "anthropic": sse(*anthropic)}
        for dialect, stream in streams.items():
            with self.subTest(dialect=dialect):
                proc, events, _ = self.oneshot(dialect, [user("fixture")], [(200, stream, None)],
                    tools=[{"name": "msh", "description": "fixture", "parameters": {"type": "object"}}])
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["done"])
                self.assertEqual(events[-1]["finish_reason"], "tool_calls")
                call, = [e for e in events if e["type"] == "tool_call"]
                self.assertEqual((call["id"], call["name"], call["arguments"]),
                                 (call_id, "msh", {"cmd": "echo 雪"}))

    # RFC v1.1 §9.2
    def test_inclusive_usage_preserves_wide_counts_and_available_cache_details(self):
        total = (1 << 32) + 100
        responses = sse({"type": "response.completed", "response": {"usage": {
            "input_tokens": total, "input_tokens_details": {"cached_tokens": 70},
            "output_tokens": 37, "output_tokens_details": {"reasoning_tokens": 11}}}})
        chat = sse({"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]},
                   {"choices": [], "usage": {"prompt_tokens": total, "completion_tokens": 37,
                    "prompt_tokens_details": {"cached_tokens": 70},
                    "completion_tokens_details": {"reasoning_tokens": 11}}}) + completion("openai-compat")
        anthropic = sse({"type": "message_start", "message": {"usage": {
            "input_tokens": total - 73, "cache_read_input_tokens": 70,
            "cache_creation_input_tokens": 3, "output_tokens": 1}}},
            {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 37}},
            {"type": "message_stop"})
        for dialect, stream in zip(DIALECTS, (responses, chat, anthropic)):
            with self.subTest(dialect=dialect):
                proc, events, _ = self.oneshot(dialect, [user("stats")], [(200, stream, None)])
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["done"])
                usage = [e for e in events if e["type"] == "usage"][-1]
                self.assertEqual((usage["input_tokens"], usage["output_tokens"]), (total, 37))
                self.assertEqual(usage["cached_tokens"], 70)
                self.assertEqual(usage.get("cache_write_tokens", 0), 3 if dialect == "anthropic" else 0)
                self.assertEqual(usage.get("reasoning_tokens", 0), 0 if dialect == "anthropic" else 11)

    # RFC v1.1.1 §6.2, §9.2, §9.5
    def test_wal_session_reaches_http_across_cli_restarts_and_provider_switches(self):
        config = self.root / ".config/anix"
        config.mkdir(parents=True)
        settings = ["[defaults]", "model='fixture'"]
        for dialect in DIALECTS:
            name = f"anix-provider-{dialect}"
            (self.root / name).symlink_to(self.binary)
            settings.extend([f"[providers.{dialect}]", f"bin='{name}'",
                             f"base_url='http://127.0.0.1:{self.http.server_port}/v1'",
                             "api_key='fixture'", "api_key_env=''", "timeout_sec=5"])
        (config / "config.toml").write_text("\n".join(settings))
        (self.root / "case.anix").write_text("routing fixture")
        env = {"HOME": str(self.root), "PATH": f"{self.root}:/usr/bin:/bin",
               "XDG_RUNTIME_DIR": str(self.root / "run")}
        text = {
            "openai-responses": {"type": "response.output_text.delta", "delta": "observed"},
            "openai-compat": {"choices": [{"index": 0, "delta": {"content": "observed"}}]},
            "anthropic": {"type": "content_block_delta", "index": 0,
                          "delta": {"type": "text_delta", "text": "observed"}},
        }
        ids = []
        runs = [("shared", dialect) for dialect in DIALECTS] + [("independent", DIALECTS[0])]
        for wal, dialect in runs:
            with self.subTest(wal=wal, dialect=dialect):
                self.http.replies.append((200, sse(text[dialect]) + completion(dialect), None))
                proc = subprocess.run(
                    ["/bin/sh", "-c", 'exec "$@" 7>trace', "fixture", self.binary.with_name("anix"),
                     "-q", "--trace-fd=7", "--provider", dialect, "-c", wal, "case.anix", "--", dialect],
                    env=env, cwd=self.root, stdin=subprocess.DEVNULL, capture_output=True, timeout=12)
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assertFalse(self.http.replies)
                self.assertEqual(len(self.http.requests), len(ids) + 1)
                records = [json.loads(line) for line in (self.root / wal).read_bytes().splitlines()]
                trace = [json.loads(line) for line in (self.root / "trace").read_bytes().splitlines()]
                start, = [e for e in trace if e["type"] == "run_start"]
                self.assertEqual(records[-1].get("text"), "observed")
                self.assertEqual(records[0]["type"], "session_init")
                sid = records[0]["session_id"]
                self.assertEqual(uuid.UUID(sid).version, 4)
                self.assertEqual((start["provider"], start["session_id"]), (dialect, sid))
                self.assertEqual(self.http.request_headers[-1].get_all("x-session-id"), [sid])
                body = self.http.requests[-1]
                if dialect == "openai-responses":
                    self.assertEqual(body["prompt_cache_key"], sid)
                else:
                    self.assertNotIn("prompt_cache_key", body)
                self.assertNotIn("session_id", body)
                ids.append(sid)
        self.assertEqual(ids[:3], [ids[0]] * 3)
        self.assertNotEqual(ids[3], ids[0])

    # RFC v1.1.1 §1.4, §9.2
    def test_session_routing_works_in_oneshot_and_per_envelope_in_coprocess(self):
        for dialect in DIALECTS:
            for mode in ("oneshot", "coprocess"):
                for default in (None, "env-session"):
                    with self.subTest(dialect=dialect, mode=mode, default=default):
                        if mode == "oneshot":
                            rows = [{"type": "header", "ir_version": 1, **header()}, user("legacy")]
                            expected = [default]
                        else:
                            rows = [envelope(1, [user("fallback")]),
                                    envelope(2, [user("override")], session_id="per-request"),
                                    envelope(3, [user("fallback again")])]
                            expected = [default, "per-request", default]
                        replies = [(200, completion(dialect), None)] * len(expected)
                        proc, events, bodies = self.run_provider(dialect, mode, rows, replies, default)
                        self.assertEqual(proc.returncode, 0, proc.stderr)
                        self.assert_terminals(events, ["done"] * len(expected))
                        for sid, body, headers in zip(expected, bodies, self.http.request_headers):
                            self.assertEqual(headers.get_all("x-session-id", []), [sid] if sid else [])
                            self.assertNotIn("session_id", body)
                            if dialect == "openai-responses" and sid:
                                self.assertEqual(body["prompt_cache_key"], sid)
                            else:
                                self.assertNotIn("prompt_cache_key", body)

    # RFC v1.1.1 §9.2
    def test_header_rename_and_disable_apply_only_to_the_current_request(self):
        cases = [({}, "x-session-id"), ({"session_header": "X-Fixture-Route"}, "x-fixture-route"),
                 ({"session_header": False}, None), ({"session_header": None}, None),
                 ({}, "x-session-id")]
        for dialect in DIALECTS:
            with self.subTest(dialect=dialect):
                rows = [envelope(i, [user("route")], header=header(params=params), session_id=f"route-{i}")
                        for i, (params, _) in enumerate(cases)]
                proc, events, bodies = self.run_provider(dialect, "coprocess", rows,
                    [(200, completion(dialect), None)] * len(rows))
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["done"] * len(rows))
                for i, ((_, key), headers, body) in enumerate(zip(cases, self.http.request_headers, bodies)):
                    for name in ("x-session-id", "x-fixture-route"):
                        self.assertEqual(headers.get_all(name, []), [f"route-{i}"] if name == key else [])
                    self.assertNotIn("session_header", body)
                    self.assertNotIn("session_id", body)
                    if dialect == "openai-responses":
                        self.assertEqual(body["prompt_cache_key"], f"route-{i}")
                    else:
                        self.assertNotIn("prompt_cache_key", body)

    # RFC v1.1.1 §9.2
    def test_responses_cache_key_has_native_string_boolean_and_null_semantics(self):
        cases = [({}, "session"), ({"prompt_cache_key": True}, "session"),
                 ({"prompt_cache_key": "explicit"}, "explicit"),
                 ({"prompt_cache_key": False}, None), ({"prompt_cache_key": None}, None)]
        dialect = "openai-responses"
        rows = [envelope(i, [user("cache")], header=header(params=params)) for i, (params, _) in enumerate(cases)]
        proc, events, bodies = self.run_provider(dialect, "coprocess", rows,
            [(200, completion(dialect), None)] * len(rows), "session")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assert_terminals(events, ["done"] * len(rows))
        for (_, expected), body, headers in zip(cases, bodies, self.http.request_headers):
            self.assertEqual(headers.get_all("x-session-id"), ["session"])
            if expected is None:
                self.assertNotIn("prompt_cache_key", body)
            else:
                self.assertEqual(body["prompt_cache_key"], expected)

    # RFC v1.1.1 §9.2
    def test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body(self):
        # Absence and explicit null are separate inputs to the provider.
        omitted = object()
        values = [omitted, True, "literal", False, None]
        combinations = [(cache, sid) for cache in values for sid in values]
        for dialect in ("openai-compat", "anthropic"):
            with self.subTest(dialect=dialect):
                rows, expected = [], []
                for i, pair in enumerate(combinations):
                    params = {key: val for key, val in zip(("prompt_cache_key", "session_id"), pair)
                              if val is not omitted}
                    route = f"turn-{i}"
                    rows.append(envelope(i, [user("opt in")], header=header(params=params), session_id=route))
                    fields = {key: route if val is True else val for key, val in params.items()
                              if val is True or isinstance(val, str)}
                    expected.append(fields if dialect == "openai-compat" else {})
                proc, events, bodies = self.run_provider(dialect, "coprocess", rows,
                    [(200, completion(dialect), None)] * len(rows))
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assert_terminals(events, ["done"] * len(rows))
                for i, (want, body, headers) in enumerate(zip(expected, bodies, self.http.request_headers)):
                    with self.subTest(params=rows[i]["header"]["params"]):
                        self.assertEqual(headers.get_all("x-session-id"), [f"turn-{i}"])
                        self.assertEqual({key: body[key] for key in ("prompt_cache_key", "session_id")
                                          if key in body}, want)


if __name__ == "__main__":
    unittest.main()
