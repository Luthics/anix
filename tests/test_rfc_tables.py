"""Exercise finite RFC tables against the CLI built from this checkout."""
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
RFC = ROOT / "docs/RFC.md"


def section(number):
    lines = RFC.read_text(encoding="utf-8").splitlines()
    header = re.compile(rf"^### {re.escape(number)}\s")
    start, = [i for i, line in enumerate(lines) if header.match(line)]
    end = next((i for i in range(start + 1, len(lines)) if re.match(r"^#{1,3} ", lines[i])), len(lines))
    return lines[start + 1:end]


def table_rows(number):
    lines = [line for line in section(number) if line.lstrip().startswith("|")]
    if len(lines) < 3:
        raise ValueError(f"§{number}: missing table or data rows")

    def cells(line):
        return [cell.strip() for cell in re.split(r"(?<!\\)\|", line.strip().strip("|"))]

    header, separator = map(cells, lines[:2])
    if len(header) != len(separator) or not all(re.fullmatch(r":?-+:?", cell) for cell in separator):
        raise ValueError(f"§{number}: malformed table header")
    for line in lines[2:]:
        row = cells(line)
        if len(row) != len(header) or not all(row):
            raise ValueError(f"§{number}: malformed row {line!r}")
        yield row


def choices(values):
    tokens = re.split(r"\s*[,|]\s*", values)
    if not tokens or len(set(tokens)) != len(tokens) or not all(re.fullmatch("[a-z][a-z0-9_-]*", t) for t in tokens):
        raise ValueError(f"invalid or duplicate enum choices: {values!r}")
    return set(tokens)


def enum_rows():
    rows = {}
    for field, kind, default, constraint in table_rows("3.3"):
        if kind != "enum":
            continue
        name = field.strip("`")
        match = re.search(r"仅允许 (.+?)[，。]", constraint)
        if not match or name in rows:
            raise ValueError(f"§3.3 malformed or repeated enum row: {field} {constraint}")
        values = choices(match[1].replace("`", "").replace("\\|", "|"))
        literal = re.fullmatch(r'`"([^"]+)"`', default)
        if not literal or literal[1] not in values:
            raise ValueError(f"§3.3 invalid default for {field}: {default}")
        rows[name] = (values, literal[1])
    if not rows:
        raise ValueError("§3.3 contains no enum rows")
    return rows


class RfcTables(unittest.TestCase):
    def setUp(self):
        area = tempfile.TemporaryDirectory(prefix="anix-rfc-tables-")
        self.addCleanup(area.cleanup)
        self.root = Path(area.name)
        bins = Path(os.environ.get("ANIX_TEST_BIN_DIR", ROOT / "target/debug")).resolve()
        self.anix = bins / "anix"
        self.env = {"HOME": str(self.root), "PATH": f"{bins}:/usr/bin:/bin",
                    "XDG_RUNTIME_DIR": str(self.root / "run"), "LC_ALL": "C"}
        config = self.root / ".config/anix"
        config.mkdir(parents=True)
        self.provider = self.root / "provider"
        (config / "config.toml").write_text(
            f'[defaults]\nprovider="fixture"\nmodel="local"\n[providers.fixture]\n'
            f'bin="{self.provider}"\nmode="oneshot"\n')
        self.script = self.root / "case.agent"
        self.script.write_text("reply")
        self.reply([{"type": "text_delta", "text": "OK"}, {"type": "done", "finish_reason": "stop"}])

    def reply(self, events):
        (self.root / "events").write_text("".join(json.dumps(event) + "\n" for event in events))
        self.provider.write_text("#!/bin/sh\ncat > request\ncat events\n")
        self.provider.chmod(0o700)

    def invoke(self, *args):
        return subprocess.run([self.anix, *args], cwd=self.root, env=self.env,
                              stdin=subprocess.DEVNULL, capture_output=True,
                              start_new_session=True, timeout=10)

    # RFC §3.3-09 — 从 RFC 读取 approval 的所有合法值，与真实 --help 公布的集合逐项比较。
    # RFC §3.3-10 — 从 RFC 读取 stderr 的所有合法值，与真实 --help 公布的集合逐项比较。
    # RFC §3.1-12 — 对照 §3.1 的封闭选项集合、§3.3 的枚举和真实 CLI help；新增、缺失或多余取值均可定位。
    # RFC §3.3-14 — 从 RFC 读取 output 的所有合法值，与真实 --help 公布的集合逐项比较。
    def test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets(self):
        declared = {}
        for line in section("3.1"):
            # --shell <PATH|none> accepts arbitrary paths and has an open domain.
            for name, values in re.findall(r"--([\w-]+) <([a-z][a-z0-9_|-]*\|[a-z0-9_|-]+)>", line):
                self.assertNotIn(name, declared)
                declared[name] = choices(values)
        result = self.invoke("--help")
        self.assertEqual(result.returncode, 0, result.stderr)
        advertised = {}
        for block in re.split(r"\n(?= +(?:-\w, )?--)", result.stdout.decode()):
            values = re.search(r"\[possible values: ([^\]]+)\]", block)
            if values:
                name = re.search(r"--([\w-]+)", block)[1]
                self.assertNotIn(name, advertised)
                advertised[name] = choices(values[1])
        expected = {name: values for name, (values, _) in enum_rows().items()}
        # RFC-ASSERT §3.1-12
        self.assertEqual(declared, expected)
        # RFC-ASSERT §3.3-14, §3.3-09, §3.3-10
        self.assertEqual(advertised, expected)

    # RFC §3.3-09 — 读取 approval 默认值；用 CLI 和 Frontmatter 逐个选择合法值，核对 session_init.config。
    # RFC §3.3-10 — 读取 stderr 默认值；用 CLI 和 Frontmatter 逐个选择合法值，核对 session_init.config。
    # RFC §3.3-14 — 读取 output 默认值；用 CLI 和 Frontmatter 逐个选择合法值，核对 session_init.config。
    # RFC §3.4-10 — 未知 output、approval 或 stderr 在 CLI 与 Frontmatter 两条路径都返回 2，WAL 和 Provider 请求均不存在。
    def test_effective_enum_defaults_choices_and_rejections_match_rfc(self):
        rows = enum_rows()
        wal = self.root / "history"
        request = self.root / "request"

        def snapshot(options=()):
            wal.unlink(missing_ok=True)
            request.unlink(missing_ok=True)
            result = self.invoke(*options, "-c", wal, self.script, "--", "go")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(request.exists())
            first = json.loads(wal.read_bytes().splitlines()[0])
            self.assertEqual(first["type"], "session_init")
            return {field: first["config"][field] for field in rows}

        defaults = {name: default for name, (_, default) in rows.items()}
        # RFC-ASSERT §3.3-14, §3.3-09, §3.3-10
        self.assertEqual(snapshot(), defaults)
        for field, (values, _) in rows.items():
            for source in ("cli", "frontmatter"):
                for value in sorted(values | {"unknown-fixture-value"}):
                    with self.subTest(field=field, source=source, value=value):
                        options = ["--" + field, value] if source == "cli" else []
                        self.script.write_text("reply" if options else f'---\n{field}: "{value}"\n---\nreply')
                        if value in values:
                            # RFC-ASSERT §3.3-14, §3.3-09, §3.3-10
                            self.assertEqual(snapshot(options), {**defaults, field: value})
                        else:
                            wal.unlink(missing_ok=True)
                            request.unlink(missing_ok=True)
                            result = self.invoke(*options, "-c", wal, self.script, "--", "go")
                            # RFC-ASSERT §3.4-10
                            self.assertEqual(result.returncode, 2, result.stderr)
                            self.assertTrue(result.stderr.strip())
                            # RFC-ASSERT §3.4-10
                            self.assertFalse(wal.exists() or request.exists())

    # RFC §6.5-09 — Provider 消费完请求后向主进程发送 INT 或 TERM，实际退出状态等于 128 加相应信号编号。
    # RFC §6.5-10 — 正常回复的进程状态等于表中 EX_OK。
    # RFC §6.5-11 — 永久 Provider 错误的进程状态等于 GENERAL_ERR。
    # RFC §6.5-12 — 未知 CLI 选项的进程状态等于 EX_USAGE。
    # RFC §6.5-13 — 拒绝工具并启用 fail-on-deny 时进程状态等于 EX_TEMPFAIL。
    # RFC §6.5-14 — 第一轮仍需执行工具且 max_turns=1 时进程状态等于 EX_TIMEOUT。
    def test_process_exit_statuses_match_every_rfc_table_row(self):
        expected = {}
        for number, symbol, detail in table_rows("6.5"):
            label = symbol.strip("`")
            self.assertNotIn(label, expected)
            code = number.strip("*`")
            if label == "SIGNAL":
                self.assertEqual(code.replace(" ", ""), "128+N")
                examples = re.findall(r"`(\d+)` 对应 `(SIG\w+)`", detail)
                expected[label] = {name: int(value) for value, name in examples}
                self.assertEqual(len(examples), len(expected[label]))
            else:
                expected[label] = int(code)

        def script_run(*options):
            return self.invoke(*options, self.script, "--", "go").returncode

        observed = {"EX_OK": script_run(), "EX_USAGE": self.invoke("--unknown-fixture-flag").returncode}
        self.reply([{"type": "error", "code": "fixture", "message": "stop", "retryable": False}])
        observed["GENERAL_ERR"] = script_run()
        self.reply([{"type": "tool_call", "id": "t", "name": "msh", "arguments": {"cmd": "printf ok"}},
                    {"type": "done", "finish_reason": "tool_calls"}])
        observed["EX_TEMPFAIL"] = script_run("--approval", "deny", "--fail-on-deny")
        observed["EX_TIMEOUT"] = script_run("-y", "--max-turns", "1")

        # The provider signals its waiting parent after consuming a whole request.
        # No terminal or timing-dependent sleep is required to choose the signal window.
        statuses = {}
        for sig in (signal.SIGINT, signal.SIGTERM):
            self.provider.write_text(f"#!/bin/sh\ncat > request\nkill -{sig.value} \"$PPID\"\nexec sleep 5\n")
            statuses[sig.name] = script_run()
            # RFC-ASSERT §6.5-09
            self.assertEqual(statuses[sig.name], 128 + sig.value)
        observed["SIGNAL"] = statuses
        # RFC-ASSERT §6.5-10, §6.5-11, §6.5-12, §6.5-13, §6.5-14, §6.5-09
        self.assertEqual(observed, expected)
