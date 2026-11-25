"""Offline release bootstrap, reinstallation and installed end-to-end CLI tests."""
import hashlib
import json
import os
from pathlib import Path
import platform
import pty
import shutil
import tarfile
import subprocess
import tempfile
import termios
import threading
import unittest
from http.server import HTTPServer

from http_fixture import JsonRequest, pictures


REPO = Path(__file__).resolve().parents[1]


DIALECTS = ("openai-responses", "openai-compat", "anthropic")
PNG = (REPO / "tests/fixtures/media/blue.png").read_bytes()


class Responses(JsonRequest):
    def do_POST(self):
        if self.path != "/v1/responses":
            self.send_error(404)
            return
        request = json.loads(self.body())
        self.server.requests.append(request)
        turn = len(self.server.requests)
        if turn in (1, 2):
            cmd = ('cat pixel.png | tee pipe-copy.png | aview - && printf "A\\000B" > "$ANIX_OUT"'
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
    @classmethod
    def setUpClass(cls):
        area = tempfile.TemporaryDirectory(prefix="anix-release-assets-")
        cls.addClassCleanup(area.cleanup)
        cls.assets = Path(area.name)
        cls.tag = "v0.0.0-fixture"
        arch = {"arm64": "aarch64", "amd64": "x86_64"}.get(platform.machine().lower(), platform.machine().lower())
        suffix = {"Darwin": "apple-darwin", "Linux": "unknown-linux-musl"}[platform.system()]
        stage = f"anix-{cls.tag}-{arch}-{suffix}"
        build = Path(os.environ.get("ANIX_TEST_BIN_DIR", REPO / "target/debug")).resolve()
        archive = cls.assets / f"{stage}.tar.gz"
        with tarfile.open(archive, "w:gz", compresslevel=1) as tar:
            for name, dest in [
                ("anix", "bin"), ("msh", "libexec/anix"), ("anix-provider", "libexec/anix"),
                ("aview", "libexec/anix/tools"), ("aclr", "libexec/anix/tools"),
            ]:
                tar.add((build / name).resolve(), arcname=f"{stage}/{dest}/{name}")
            for dialect in DIALECTS:
                link = tarfile.TarInfo(f"{stage}/libexec/anix/anix-provider-{dialect}")
                link.type = tarfile.SYMTYPE
                link.linkname = "anix-provider"
                tar.addfile(link)
            tar.add(REPO / "examples/bash.agent", arcname=f"{stage}/examples/bash.agent")
        cls.archive_name = archive.name
        digest = hashlib.sha256()
        with archive.open("rb") as content:
            for block in iter(lambda: content.read(1024 * 1024), b""):
                digest.update(block)
        (cls.assets / "SHA256SUMS.txt").write_text(f"{digest.hexdigest()}  {archive.name}\n")
        # Only the two local release assets are accessible to the installer.
        # An unexpected download must fail locally, including gh/wget fallbacks.
        (cls.assets / "curl").write_text("""#!/bin/sh
set -eu
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) dest=$2; shift 2 ;;
        https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
base="https://github.com/Luthics/anix/releases/download/$ANIX_VERSION"
case "$url" in
    "$base/$FIXTURE_ARCHIVE"|"$base/SHA256SUMS.txt")
        exec /bin/cp "$FIXTURE_ASSETS/${url##*/}" "$dest" ;;
    *) exit 1 ;;
esac
""")
        for name in ("curl", "wget", "gh"):
            path = cls.assets / name
            if name != "curl":
                path.write_text("#!/bin/sh\nexit 1\n")
            path.chmod(0o700)

    def install(self, root):
        env = {"HOME": str(root), "PATH": f"{self.assets}:/usr/bin:/bin",
               "FIXTURE_ASSETS": str(self.assets), "FIXTURE_ARCHIVE": self.archive_name, "ANIX_VERSION": self.tag}
        # Feed the real installer to sh, exactly as in the bootstrap command.
        with (REPO / "install.sh").open("rb") as source:
            result = subprocess.run(["/bin/sh"], cwd=root, env=env, stdin=source,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=75)
        self.assertEqual(result.returncode, 0, result.stderr)

    # RFC v1.1.1 §2, §10.3
    def test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin(self):
        with tempfile.TemporaryDirectory(prefix="anix reinstall ") as home:
            root = Path(home)
            user_bin = root / ".local/bin"
            user_bin.mkdir(parents=True)
            existing = user_bin / "existing-command"
            existing.write_bytes(b"#!/bin/sh\nexit 0\n")
            existing.chmod(0o755)
            startup = {name: f"# {name}\nexport LANG=C\n".encode()
                       for name in (".bashrc", ".bash_profile", ".profile", ".zprofile", ".zshenv")}
            for name, data in startup.items():
                (root / name).write_bytes(data)
            (root / ".zshrc").symlink_to(".zprofile")
            for iteration in (1, 2):
                with self.subTest(installation=iteration):
                    self.install(root)
                    cfg = root / ".config/anix/config.toml"
                    starter = cfg.parent / "agents/bash.agent"
                    private = root / ".local/libexec/anix"
                    for dialect in DIALECTS:
                        alias = private / f"anix-provider-{dialect}"
                        self.assertTrue(alias.is_symlink() and alias.samefile(private / "anix-provider"), alias)
                    self.assertTrue(os.access(starter, os.X_OK))
                    if iteration == 1:
                        self.assertTrue(cfg.stat().st_size)
                        self.assertEqual(starter.read_bytes().splitlines()[0], b"#!/usr/bin/env anix")
                        cfg.write_text("# retained Anix configuration\n")
                        starter.write_text("#!/usr/bin/env anix\nretain my script\n")
                    else:
                        self.assertEqual(cfg.read_text(), "# retained Anix configuration\n")
                        self.assertEqual(starter.read_text(), "#!/usr/bin/env anix\nretain my script\n")
                    for name, data in startup.items():
                        self.assertEqual((root / name).read_bytes(), data)
                    self.assertEqual(os.readlink(root / ".zshrc"), ".zprofile")
                    self.assertEqual({p.name for p in user_bin.iterdir()}, {"existing-command", "anix"})
                    self.assertEqual(existing.read_bytes(), b"#!/bin/sh\nexit 0\n")
                    for rel in ("bin/anix", "libexec/anix/msh", "libexec/anix/anix-provider-openai-responses",
                                "libexec/anix/tools/aview", "libexec/anix/tools/aclr"):
                        path = root / ".local" / rel
                        self.assertTrue(path.is_file() and os.access(path, os.X_OK), path)

    # RFC v1.1.1 §4.1, §10.3
    @unittest.skipUnless(platform.system() == "Linux" and shutil.which("strace"), "requires Linux strace")
    def test_doctor_detects_unavailable_pipes_and_unusable_file_locking(self):
        with tempfile.TemporaryDirectory(prefix="anix-doctor-") as home:
            root = Path(home)
            self.install(root)
            (root / ".config/anix/config.toml").write_text(
                "[providers.openai-responses]\napi_key_env=''\napi_key=''\n")
            env = {"HOME": home, "PATH": "/usr/bin:/bin", "XDG_RUNTIME_DIR": str(root / "run")}
            doctor = [str(root / ".local/bin/anix"), "--doctor"]

            def run(argv):
                return subprocess.run(argv, cwd=root, env=env, stdin=subprocess.DEVNULL,
                                      capture_output=True, timeout=12)

            healthy = run(doctor)
            self.assertEqual(healthy.returncode, 0, healthy.stderr)
            trace = root / "syscalls.log"
            for syscalls, fault, subject, repair in [
                ("?pipe,pipe2", "error=EMFILE", rb"pipe|/dev/fd", rb"/dev/fd|mount"),
                ("flock", "error=ENOSYS", rb"flock|lock", rb"XDG_RUNTIME_DIR|filesystem"),
                # Each open file description appears to receive an exclusive lock.
                ("flock", "retval=0", rb"flock|lock", rb"XDG_RUNTIME_DIR|filesystem"),
            ]:
                with self.subTest(syscalls=syscalls, fault=fault):
                    result = run([shutil.which("strace"), "-f", "-qq", "-o", str(trace),
                                  "-e", f"trace={syscalls}", "-e", f"inject={syscalls}:{fault}", *doctor])
                    self.assertIn("INJECTED", trace.read_text(), result.stderr)
                    self.assertEqual(result.returncode, 1, (result.stdout, result.stderr))
                    self.assertRegex(result.stderr, subject)
                    self.assertRegex(result.stderr, repair)
            restored = run(doctor)
            self.assertEqual(restored.returncode, 0, restored.stderr)

    def install_with_text_provider(self, root):
        self.install(root)
        agents = root / ".config/anix/agents"
        mock = root / "local-fixture-provider"
        mock.write_text("""#!/bin/sh
set -eu
/bin/cat > "$HOME/request"
printf '%s\\n' '{"type":"text_delta","text":"fixture reply"}' '{"type":"done","finish_reason":"stop"}'
""")
        mock.chmod(0o700)
        (agents.parent / "config.toml").write_text(
            f"""[providers.openai-responses]
bin={json.dumps(str(mock))}
mode="oneshot"
api_key_env=""
timeout_sec=3
""")
        env = {"HOME": str(root), "PATH": f"{agents}:{root}/.local/bin:/usr/bin:/bin",
               "XDG_RUNTIME_DIR": str(root / "run")}
        return agents, env

    # RFC v1.1.1 §4.1
    def test_usage_line_resets_dim_before_the_final_newline_on_a_terminal(self):
        with tempfile.TemporaryDirectory(prefix="anix-usage-tty-") as home:
            root = Path(home)
            agents, env = self.install_with_text_provider(root)
            master, slave = pty.openpty()
            try:
                # Capture the bytes written by Anix, before the terminal adds CRs.
                attrs = termios.tcgetattr(slave)
                attrs[1] &= ~termios.OPOST
                termios.tcsetattr(slave, termios.TCSANOW, attrs)
                result = subprocess.run([agents / "bash.agent", "xxx"], cwd=root,
                                        env={**env, "TERM": "xterm-256color"}, stdin=subprocess.DEVNULL,
                                        stdout=subprocess.PIPE, stderr=slave, timeout=8)
                os.set_blocking(master, False)
                output = bytearray()
                try:
                    while chunk := os.read(master, 8192):
                        output.extend(chunk)
                except BlockingIOError:
                    pass
            finally:
                os.close(slave)
                os.close(master)
            self.assertEqual(result.returncode, 0, bytes(output))
            self.assertEqual(result.stdout, b"fixture reply")
            self.assertNotIn(b"fixture reply", output)
            self.assertEqual(output.count(b"[usage]"), 1)
            self.assertRegex(bytes(output), rb"(?:^|\n)\x1b\[2m\[usage\] [^\r\n]+\x1b\[0m\n\Z")

    # RFC v1.1.1 §2, §3.1, §4.1, §9.2, §10.3
    def test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof(self):
        with tempfile.TemporaryDirectory(prefix="anix-install-tty-") as home:
            root = Path(home)
            agents, env = self.install_with_text_provider(root)
            for program, cwd in [("bash.agent", root), ("./bash.agent", agents)]:
                with self.subTest(program=program):
                    (root / "request").unlink(missing_ok=True)
                    master, slave = pty.openpty()
                    try:
                        # Keep the master open and never send EOF or a keystroke.
                        result = subprocess.run([program, "xxx"], cwd=cwd, env=env, stdin=slave,
                                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                                start_new_session=True, timeout=8)
                    finally:
                        os.close(slave)
                        os.close(master)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stdout, b"fixture reply")
                    self.assertNotIn(b"fixture reply", result.stderr)
                    self.assertIn(b"[usage]", result.stderr)
                    frames = [json.loads(line) for line in (root / "request").read_bytes().splitlines()]
                    self.assertEqual([f["text"] for f in frames if f.get("role") == "user"], ["<args>xxx</args>"])

    # RFC v1.1.1 §2, §3.1, §4.1, §4.4, §6.1, §6.2, §7.2, §9.2, §9.6, §10.3
    def test_installed_pipeline_and_fold(self):
        with tempfile.TemporaryDirectory(prefix="anix-fhs-") as home:
            root = Path(home)
            prefix = root / ".local"
            for name in (".bashrc", ".zshrc", ".profile"):
                (root / name).write_bytes(b"# fixture rc\n")
            self.install(root)
            for name in (".bashrc", ".zshrc", ".profile"):
                self.assertEqual((root / name).read_bytes(), b"# fixture rc\n")
            self.assertEqual({p.name for p in (prefix / "bin").iterdir()}, {"anix"})
            libexec = prefix / "libexec/anix"
            for native in ("cat", "ls", "grep"):
                self.assertFalse(os.path.lexists(libexec / "tools" / native), native)
            for path in [libexec / "msh", libexec / "anix-provider-openai-responses",
                         libexec / "tools/aview", libexec / "tools/aclr"]:
                self.assertTrue(path.is_file() and os.access(path, os.X_OK), path)

            with HTTPServer(("127.0.0.1", 0), Responses) as http:
                http.requests = []
                thread = threading.Thread(target=http.serve_forever, kwargs={"poll_interval": 0.01}, daemon=True)
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
        config = root / ".anix"
        config.mkdir(parents=True, exist_ok=True)
        config.joinpath("config.toml").write_text(
            "[defaults]\nprovider='openai-responses'\nmodel='test-model'\n[providers.openai-responses]\n"
            f"base_url='http://127.0.0.1:{http.server_port}/v1'\n"
            "api_key_env=''\napi_key='loopback-fixture'\ntimeout_sec=10\n")
        (root / "pixel.png").write_bytes(PNG)
        script = root / "demo.agent"
        script.write_text("#!/usr/bin/env anix\n---\napproval: auto\noutput: explicit\nstderr: explicit\n"
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
            self.assertEqual((root / "pipe-copy.png").read_bytes(), PNG)
            self.assertEqual(len(http.requests), 3)
            first, second, third = http.requests
            self.assertEqual(first["model"], "test-model")
            self.assertIn("inspect fixture", first["instructions"])
            for contract in ("$ANIX_OUT", "$ANIX_ERR", "<runtime_tips>",
                             "how-to-background.md"):
                self.assertIn(contract, first["instructions"])
            self.assertEqual(first["instructions"], third["instructions"])
            user_text = first["input"][-1]["content"][0]["text"]
            self.assertLess(user_text.index("<args>sample</args>"), user_text.index("from stdin"))
            self.assertEqual(len(media), 1)
            self.assertEqual(media[0].stat().st_mode & 0o777, 0o600)
            output = next(i for i in second["input"] if i["type"] == "function_call_output")
            self.assertEqual(output["call_id"], "call-1")
            self.assertEqual(list(pictures(second["input"])), [("image/png", media[0].read_bytes())])
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
