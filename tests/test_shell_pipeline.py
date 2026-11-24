"""RFC §§4.3, 7.1, 9.5 through the real agent and msh, with a local provider.

Build both executables first. OS_AGENT_BIN_DIR can select another build.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class ShellPipeline(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="agent-shell-rfc-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        binaries = Path(os.environ.get("OS_AGENT_BIN_DIR", Path(__file__).resolve().parents[1] / "target/debug"))
        self.agent = binaries / "agent"
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

    def test_real_shell_timeout_returns_an_error_tool_result_and_inference_continues(self):
        result, tool = self.invoke("exec /bin/sleep 15", {"MSH_TIMEOUT_SEC": "1"})
        self.assertTrue(tool["is_error"])
        self.assertTrue(tool["text"].startswith(f"[t{tool['turn_id']}] exit=124\n"))
        self.assertEqual(result.stdout, b"")

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
        tools = self.libexec / "tools"
        tools.mkdir()
        launch = tools / "start-child"
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
            "start-child", {"TEST_AGENT_BINARY": str(self.agent), "AGENT_MODEL": "outer-model",
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

    def test_nested_agent_uses_own_model_wal_and_budget_and_inherits_cli_auto(self):
        self.run_nested(("-y",), "ask", True)

    def test_nested_agent_inherits_cli_deny_over_its_own_auto_frontmatter(self):
        self.run_nested(("--approval", "deny"), "auto", False)


if __name__ == "__main__":
    unittest.main()
