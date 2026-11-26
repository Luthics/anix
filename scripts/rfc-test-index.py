"""Link current RFC requirements to their conformance tests in both directions."""
import argparse
import ast
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
RFC = Path("docs/RFC.md")


def rust_tests(lines):
    for i, line in enumerate(lines):
        if not re.fullmatch(r"\s*#\[(?:[\w:]+::)?test(?:\([^\]]*\))?\]\s*", line):
            continue
        start = i
        while start and lines[start - 1].lstrip().startswith("#["):
            start -= 1
        for j in range(i + 1, len(lines)):
            decl = lines[j].strip()
            if not decl or decl.startswith(("#[", "//")):
                continue
            name = re.match(r"(?:async )?fn\s+(\w+)\b", decl)
            if not name:
                raise ValueError(f"unrecognized test declaration at line {j+1}")
            yield start, j + 1, name[1]
            break
        else:
            raise ValueError(f"missing test declaration after line {i+1}")


ID = r"[1-9]\d*(?:\.\d+)?-[0-9]{2,}"
ANNOTATION = re.compile(rf"(?:#|//) RFC §({ID}) — (\S.*)")
MARKER = re.compile(rf"# RFC-ASSERT (§{ID}(?:, §{ID})*)")


@dataclass
class Case:
    path: str
    name: str
    line: int
    notes: dict
    assertions: dict

    @property
    def key(self):
        return f"{self.path}::{self.name}"

    def link(self, line=None, label=None):
        return f"[{label or ('`' + self.name + '`')}](../{self.path}#L{line or self.line})"


@dataclass
class Requirement:
    id: str
    behavior: str
    line: int
    refs: list

    @property
    def section(self):
        return self.id.split("-")[0]

    def link(self, label):
        return f"[{label}](../{RFC}?plain=1#L{self.line})"


def python_tests(source):
    for node in ast.walk(ast.parse(source)):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.startswith("test_"):
            start = min([node.lineno, *(d.lineno for d in node.decorator_list)])
            yield start - 1, node.lineno, node.name


def assertion_lines(source):
    """A marked comparison must read at least one operand beyond literal values."""
    for node in ast.walk(ast.parse(source)):
        operands = []
        if isinstance(node, ast.Assert):
            operands = [node.test]
        elif isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
            call = node.value
            func = call.func
            if (isinstance(func, ast.Attribute) and isinstance(func.value, ast.Name)
                    and func.value.id == "self" and func.attr in UNITTEST_ASSERTIONS):
                # The optional failure message can reference the CLI even when the
                # comparison itself only checks constants. Exclude both forms of msg.
                arity = 1 if func.attr in {"assertTrue", "assertFalse", "assertIsNone", "assertIsNotNone"} else 2
                operands = call.args[:arity] + [kw.value for kw in call.keywords if kw.arg != "msg"]
        if any(isinstance(part, (ast.Name, ast.Call)) for operand in operands for part in ast.walk(operand)):
            yield node.lineno


# Only comparison methods that perform an assertion when directly called are eligible.
UNITTEST_ASSERTIONS = {"assertEqual", "assertNotEqual", "assertTrue", "assertFalse", "assertIn",
                       "assertNotIn", "assertIs", "assertIsNone", "assertIsNotNone", "assertGreater",
                       "assertGreaterEqual", "assertLess", "assertLessEqual", "assertRegex"}


def read_cases(root):
    files = sorted([*(root / "crates").rglob("*.rs"), *(root / "tests").rglob("*.py")])
    cases = {}
    for path in files:
        source = path.read_bytes().decode("utf-8")
        lines = source.split("\n")
        rel = path.relative_to(root).as_posix()
        rust = path.suffix == ".rs"
        entries = list(rust_tests(lines) if rust else python_tests(source))
        marked = {i for i, line in enumerate(lines)
                  if re.match(r"\s*(?://|#) RFC(?: §| v\d|-ASSERT\b)", line)}
        consumed = set()
        # AST end positions prevent a helper or the next class from claiming a test's assertion.
        owners = {}
        valid_assertions = set()
        if not rust and any("RFC-ASSERT" in lines[i] for i in marked):
            valid_assertions = set(assertion_lines(source))
            for node in ast.walk(ast.parse(source)):
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.startswith("test_"):
                    owners[node.lineno] = node.end_lineno

        for start, line, name in entries:
            notes = {}
            for i in range(start - 1, -1, -1):
                match = ANNOTATION.fullmatch(lines[i].strip())
                if not match:
                    break
                id_, note = match.groups()
                if id_ in notes:
                    raise ValueError(f"{rel}:{i+1} {name}: duplicate annotation §{id_}")
                notes[id_] = note
                consumed.add(i)
            if not notes:
                raise ValueError(f"{rel}:{line} {name}: missing a detailed RFC annotation")
            assertions = defaultdict(list)
            for i in range(line, owners.get(line, line)):
                if i not in marked:
                    continue
                marker = MARKER.fullmatch(lines[i].strip())
                if not marker or i + 2 not in valid_assertions:
                    raise ValueError(f"{rel}:{i+1} {name}: RFC-ASSERT must immediately precede an assertion "
                                     "with a nonliteral operand in its condition")
                for token in marker[1].split(", "):
                    id_ = token[1:]
                    if i + 2 in assertions[id_]:
                        raise ValueError(f"{rel}:{i+1} {name}: duplicate RFC-ASSERT §{id_}")
                    assertions[id_].append(i + 2)
                consumed.add(i)
            case = Case(rel, name, line, dict(reversed(notes.items())), assertions)
            if case.key in cases:
                raise ValueError(f"{rel}:{line} duplicate test {name}")
            cases[case.key] = case
        unused = sorted(marked - consumed)
        if unused:
            raise ValueError(f"{rel}:{unused[0]+1} orphan or malformed RFC annotation")
    if not cases:
        raise ValueError("no tests discovered")
    return cases


# The first column identifies each closed table in the canonical RFC. New data rows need
# their own source-line evidence even if both the catalog and the annotations omit them.
CONTRACT_TABLES = {"3.3": "字段", "4.1": "FD", "4.3": "子进程 FD", "6.4": "信号", "6.5": "退出码"}


def check_table_rows(lines, sections, requirements, tables):
    covered = {r.line for r in requirements.values()
               if any(ref != "TODO" and not ref.startswith("#") for ref in r.refs)}
    for section, title in tables.items():
        if section not in sections:
            raise ValueError(f"{RFC}: missing table section §{section}")
        start = sections[section]
        end = next((line for line in sections.values() if line > start), len(lines) + 1)
        headers = [i for i in range(start, end - 1)
                   if re.match(rf"^\s*\|\s*{re.escape(title)}\s*\|", lines[i])]
        if len(headers) != 1:
            raise ValueError(f"{RFC}: §{section} expected one table headed {title!r}")
        header = headers[0]
        if header + 1 >= end - 1:
            raise ValueError(f"{RFC}:{header + 1} missing separator for §{section}")
        columns = [cell.strip() for cell in lines[header + 1].strip().strip("|").split("|")]
        if not columns or not all(re.fullmatch(r":?-+:?", cell) for cell in columns):
            raise ValueError(f"{RFC}:{header + 2} malformed separator for §{section}")
        first = header + 2
        keys = set()
        for i in range(first, end - 1):
            row = lines[i].strip()
            if not row.startswith("|"):
                break
            key = row.split("|", 2)[1].strip()
            if not key or key in keys:
                raise ValueError(f"{RFC}:{i + 1} §{section} empty or duplicate table key {key!r}")
            if i + 1 not in covered:
                raise ValueError(f"{RFC}:{i + 1} §{section} {key}: table row needs executable test evidence")
            keys.add(key)
        if not keys:
            raise ValueError(f"{RFC}:{first + 1} empty table in §{section}")


def read_requirements(root, *, checked_tables=CONTRACT_TABLES):
    # LF bytes match GitHub source line anchors even in tables.
    lines = (root / RFC).read_bytes().decode("utf-8").split("\n")
    versions = [m[1] for line in lines if (m := re.match(r"^\*\*版本\*\*.*?\b(v\d+\.\d+\.\d+)\b", line))]
    if len(versions) != 1:
        raise ValueError(f"{RFC}: expected one version")
    sections = {}
    for n, line in enumerate(lines, 1):
        if heading := re.match(r"^#{2,3} (\d+(?:\.\d+)?)\.?\s", line):
            if heading[1] in sections:
                raise ValueError(f"{RFC}:{n} repeated section §{heading[1]}")
            sections[heading[1]] = n
    requirements = {}
    seen = set()
    for n, row in enumerate((root / "tests/rfc-requirements.tsv").read_text().splitlines(), 1):
        if not row or row.startswith("#"):
            continue
        where = f"tests/rfc-requirements.tsv:{n}"
        parts = row.split("\t")
        if len(parts) != 4:
            raise ValueError(f"{where}: expected ID, RFC excerpt, behavior and evidence")
        id_, excerpt, behavior, evidence = parts
        section = id_.split("-")[0]
        if (not re.fullmatch(ID, id_) or section not in sections or not excerpt or not behavior
                or not evidence.strip() or len(set(evidence.split())) != len(evidence.split())):
            raise ValueError(f"{where}: invalid §{id_}, empty field or duplicate evidence")
        if id_ in requirements or (section, behavior) in seen:
            raise ValueError(f"{where}: duplicate ID or behavior for §{id_}")
        seen.add((section, behavior))
        start = sections[section]
        stop = next((line for line in sections.values() if line > start), len(lines) + 1)
        matches = [i for i in range(start, stop) if excerpt in lines[i-1]]
        if len(matches) != 1:
            raise ValueError(f"{where}: §{id_} RFC excerpt {excerpt!r} has {len(matches)} matches")
        requirements[id_] = Requirement(id_, behavior, matches[0], evidence.split())
    if not requirements:
        raise ValueError("empty requirement catalog")
    check_table_rows(lines, sections, requirements, checked_tables)
    return versions[0], requirements


def reconcile(cases, requirements):
    """Compare two independently edited sets, including the strict assertion witnesses."""
    by_name = defaultdict(list)
    for case in cases.values():
        by_name[case.name].append(case.key)
    expected = set()
    strict = set()
    for req in requirements.values():
        for ref in req.refs:
            if ref == "TODO" or re.fullmatch(r"#[1-9]\d*", ref):
                continue
            name = ref.removeprefix("!")
            keys = [name] if name in cases else by_name[name]
            if len(keys) != 1:
                raise ValueError(f"§{req.id}: unknown or ambiguous test {name!r}")
            edge = (req.id, keys[0])
            if edge in expected:
                raise ValueError(f"§{req.id}: repeated test {keys[0]}")
            expected.add(edge)
            if ref.startswith("!"):
                strict.add(edge)
    declared = {(id_, c.key) for c in cases.values() for id_ in c.notes}
    marked = {(id_, c.key) for c in cases.values() for id_ in c.assertions}
    errors = []
    for label, missing in [("missing source annotation", expected - declared),
                           ("missing catalog reference", declared - expected),
                           ("missing assertion witness", strict - marked),
                           ("undeclared assertion witness", marked - strict)]:
        for id_, key in sorted(missing):
            case = cases[key]
            errors.append(f"{case.path}:{case.line} {label}: §{id_} ↔ {case.name}")
    if errors:
        raise ValueError("\n".join(errors))
    return len(expected), len(strict)


def cell(text):
    return text.replace("|", r"\|")


def build_index(root=ROOT, *, checked_tables=CONTRACT_TABLES):
    version, requirements = read_requirements(root, checked_tables=checked_tables)
    cases = read_cases(root)
    edges, strict = reconcile(cases, requirements)
    forward = [f"# {version} RFC 逐测试索引", "",
               "按测试查看它所验证的行为。编号链接到 RFC 原句，断言链接到执行比较的代码。"
               "同一条款有多个测试时，每行只描述该测试实际检查的部分。", "",
               "反向查找见[条款验收表](RFC-requirements.md)。修改源码注释和验收表后，运行 "
               "`python3 scripts/rfc-test-index.py` 更新两个索引。", "",
               "历史快照：[v1.1.2](https://github.com/Luthics/anix/blob/e4fb614f314625ae282bd2fcb9b7e80312514946/tests/RFC-cases.md)、"
               "[v1.1.1](https://github.com/Luthics/anix/blob/0b11a3c60218b9313f9e81a806c9a471906766eb/tests/RFC-cases.md)、"
               "[v1.0](https://github.com/Luthics/anix/blob/45e55a9f16f6bead6a933c67051b94b0acb753a0/tests/RFC-cases.md)。"]
    by_id = defaultdict(list)
    path = None
    for case in cases.values():
        if case.path != path:
            path = case.path
            forward.extend(["", f"## `{path}`", "", "| 测试 | RFC 原文 | 本测试的预期结果 |",
                            "| --- | --- | --- |"])
        for id_, note in case.notes.items():
            req = requirements[id_]
            witnesses = [case.link(n, f"断言 L{n}") for n in case.assertions.get(id_, [])]
            detail = cell(note) + (" " + " · ".join(witnesses) if witnesses else "")
            forward.append(f"| {case.link()} | {req.link('§' + id_)} | {detail} |")
            by_id[id_].append(" ".join([case.link(), *witnesses]))
    reverse = [f"# {version} RFC 条款验收表", "",
               "编号的前半部分是 RFC 章节，后半部分是本仓库的稳定验收编号。"
               "源码注释与清单独立维护，CI 要求两边声明同一批测试。", "",
               "在源码测试声明前写 `RFC §编号 — 本用例的输入或操作及预期结果。`。"
               "编辑 [rfc-requirements.tsv](rfc-requirements.tsv) 时保留已有编号，为新行为分配新编号。"
               "清单中的 `!测试名` 还要求在 Python 断言正上方写 `# RFC-ASSERT §编号`。构建二进制后，运行 `python3 scripts/check-rfc-assertions.py` 验证每处标记的断言都已执行。", "",
               "CI 逐行检查 §3.3、§4.1、§4.3、§6.4、§6.5 的五张表，每行至少关联一项可执行测试。单列 issue 的验收项仍待规范裁决，TODO 表示还有待补断言。执行结果见对应提交的 CI。"
               "按测试查找见[逐测试索引](RFC-cases.md)。"]
    section = None
    for req in requirements.values():
        if section != req.section:
            section = req.section
            reverse.extend(["", f"## §{section}", "", "| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |",
                            "| --- | --- | --- |"])
        links = list(by_id[req.id])
        for ref in req.refs:
            if ref == "TODO":
                links.append("待补专项断言")
            elif ref.startswith("#"):
                links.append(f"[{ref}](https://github.com/Luthics/anix/issues/{ref[1:]})")
        reverse.append(f"| {req.link('§' + req.id)} | {cell(req.behavior)} | {' · '.join(links)} |")
    summary = f"{version}: {len(cases)} tests, {len(requirements)} requirements, {edges} reciprocal links, {strict} assertion-linked pairs"
    return {"RFC-cases.md": forward, "RFC-requirements.md": reverse}, summary


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        docs, summary = build_index()
        for name, lines in docs.items():
            path = ROOT / "tests" / name
            rendered = "\n".join(lines) + "\n"
            if args.check:
                if not path.is_file() or path.read_text() != rendered:
                    sys.exit(f"{name} is stale. Run python3 scripts/rfc-test-index.py")
            else:
                path.write_text(rendered)
    except ValueError as err:
        sys.exit(str(err))
    print(summary)
