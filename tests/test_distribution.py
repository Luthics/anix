"""RFC §§2, 3.1, 4.1–4.4, 6.1–6.2, 7, 9 and 10.3 through the installed binaries.

The default entry point builds from the checkout. To test a release bootstrap,
set OS_AGENT_BOOTSTRAP to a locally staged installer that can run under sh.
"""
import base64
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import threading
import unittest
import zlib
from http.server import HTTPServer

from http_fixture import JsonRequest


REPO = Path(__file__).resolve().parents[1]


def one_pixel_png():
    def chunk(name, data):
        crc = zlib.crc32(name + data)
        return struct.pack(">I", len(data)) + name + data + struct.pack(">I", crc)

    header = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(b"\0\xff\x7f\0")) + chunk(b"IEND", b""))


class Responses(JsonRequest):
    def do_POST(self):
        if self.path != "/v1/responses":
            self.send_error(404)
            return
        request = json.loads(self.body())
        self.server.requests.append(request)
        turn = len(self.server.requests)
        if turn in (1, 2):
            cmd = ('cat pixel.png | tee pipe-copy.png | aview - && printf "A\\000B" > "$AGENT_OUT"'
                   if turn == 1 else 'aclr -t 3 "saved image"')
            event = {
                "type": "response.output_item.done",
                "item": {"type": "function_call", "call_id": f"call-{turn}",
                         "name": "msh", "arguments": json.dumps({"cmd": cmd})},
            }
        elif turn == 3:
            event = {"type": "response.output_text.delta", "delta": "private final text"}
        else:
            self.send_error(400, "unexpected additional inference")
            return
        events = [event, {"type": "response.completed", "response": {}}]
        data = "".join(f"data: {json.dumps(e)}\n\n" for e in events).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


class Distribution(unittest.TestCase):
    # RFC §1.2, §2, §3.1, §10.3, §7.2, §6.1, §4.4, §9.6
    def test_installed_pipeline_and_fold(self):
        with tempfile.TemporaryDirectory(prefix="agent-fhs-") as home:
            root = Path(home)
            prefix = root / ".local"
            for name in (".bashrc", ".zshrc", ".profile"):
                (root / name).write_bytes(b"# fixture rc\n")
            # Cargo's cache/toolchain belong to the build environment. All installed
            # artifacts and all subsequent agent processes use a fresh HOME.
            build_env = dict(os.environ, HOME=home, OS_AGENT_PREFIX=str(prefix),
                             XDG_CONFIG_HOME=str(root / ".config"),
                             XDG_STATE_HOME=str(root / ".local/state"),
                             XDG_RUNTIME_DIR=str(root / "run"))
            for var, subdir in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")]:
                build_env.setdefault(var, str(Path.home() / subdir))
            build_env.pop("CARGO_TARGET_DIR", None)
            bootstrap = os.environ.get("OS_AGENT_BOOTSTRAP")
            if bootstrap:
                # Match the documented sh-on-stdin invocation. Test the release
                # script itself, before the fixture provisions its loopback API.
                release_env = {k: build_env[k] for k in (
                    "HOME", "PATH", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR")}
                with Path(bootstrap).open("rb") as source:
                    subprocess.run(["/bin/sh"], stdin=source, cwd=root, env=release_env,
                                   stdout=subprocess.PIPE, check=True, timeout=600)
            else:
                subprocess.run(["bash", str(REPO / "scripts/install-local.sh")], check=True,
                               cwd=REPO, env=build_env, stdout=subprocess.PIPE, timeout=600)
            for name in (".bashrc", ".zshrc", ".profile"):
                self.assertEqual((root / name).read_bytes(), b"# fixture rc\n")
            self.assertEqual({p.name for p in (prefix / "bin").iterdir()}, {"agent"})
            libexec = prefix / "libexec/agent"
            for native in ("cat", "ls", "grep"):
                self.assertFalse(os.path.lexists(libexec / "tools" / native), native)
            for path in [libexec / "msh", libexec / "agent-provider-openai-responses",
                         libexec / "tools/aview", libexec / "tools/aclr"]:
                self.assertTrue(path.is_file() and os.access(path, os.X_OK), path)

            with HTTPServer(("127.0.0.1", 0), Responses) as http:
                http.requests = []
                thread = threading.Thread(target=http.serve_forever, daemon=True)
                thread.start()
                try:
                    self.exercise(root, http)
                finally:
                    http.shutdown()
                    thread.join(timeout=5)
                    self.assertFalse(thread.is_alive())

    def exercise(self, root, http):
        # The project only redirects inference to the test server. Leave the
        # installer's global config and hooks in place for the actual first run.
        config = root / ".agent"
        config.mkdir(parents=True, exist_ok=True)
        config.joinpath("config.toml").write_text(
            "[defaults]\nprovider='openai-responses'\nmodel='test-model'\n[providers.openai-responses]\n"
            f"base_url='http://127.0.0.1:{http.server_port}/v1'\n"
            "api_key_env=''\napi_key='loopback-fixture'\ntimeout_sec=10\n")
        (root / "pixel.png").write_bytes(one_pixel_png())
        script = root / "demo.agent"
        script.write_text("#!/usr/bin/env agent\n---\napproval: auto\nstderr: explicit\n"
                          "context: session.jsonl\n---\ninspect fixture")
        script.chmod(0o700)
        env = {"HOME": str(root), "PATH": f"{root}/.local/bin:/usr/bin:/bin",
               "XDG_RUNTIME_DIR": str(root / "run")}
        result = subprocess.run([str(script), "--", "sample"], input=b"from stdin", cwd=root,
                                env=env, capture_output=True, timeout=35)
        wal_path = root / "session.jsonl"
        records = [json.loads(line) for line in wal_path.read_text().splitlines()] if wal_path.exists() else []
        media = [Path(m["path"]) for row in records for m in row.get("media", [])]
        try:
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, b"A\x00B")
            self.assertEqual(result.stderr, b"")
            self.assertEqual((root / "pipe-copy.png").read_bytes(), one_pixel_png())
            self.assertEqual(len(http.requests), 3)
            first, second, third = http.requests
            self.assertEqual(first["model"], "test-model")
            self.assertIn("inspect fixture", first["instructions"])
            for contract in ("$AGENT_OUT", "$AGENT_ERR", "<runtime_tips>",
                             "how-to-background.md"):
                self.assertIn(contract, first["instructions"])
            self.assertEqual(first["instructions"], third["instructions"])
            user_text = first["input"][-1]["content"][0]["text"]
            self.assertLess(user_text.index("<args>sample</args>"), user_text.index("from stdin"))
            self.assertEqual(len(media), 1)
            self.assertEqual(media[0].stat().st_mode & 0o777, 0o600)
            output = next(i for i in second["input"] if i["type"] == "function_call_output")
            self.assertEqual(output["call_id"], "call-1")
            img = next(p for p in output["output"] if p["type"] == "input_image")
            mime, data = img["image_url"].split(",", 1)
            self.assertEqual(mime, "data:image/png;base64")
            self.assertEqual(base64.b64decode(data, validate=True), media[0].read_bytes())
            self.assertNotIn("base64,", wal_path.read_text())
            self.assertEqual([r["type"] for r in records if r.get("type") == "fold"], ["fold"])
            self.assertFalse(any(i["type"].startswith("function_call") for i in third["input"]))
            self.assertIn("saved image", third["input"][-1]["content"][0]["text"])
        finally:
            for path in media:
                if path.parent == Path("/tmp") and path.name.startswith("aview_"):
                    path.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
