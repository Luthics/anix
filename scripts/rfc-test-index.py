"""Link RFC requirements and test declarations in both directions."""
import argparse
import ast
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
RFC = Path("docs/v1.0.0-RFC.md")
INDEX = ROOT / "tests/RFC-cases.md"
REQUIREMENTS = ROOT / "tests/rfc-requirements.tsv"
REVERSE_INDEX = ROOT / "tests/RFC-requirements.md"


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


def python_tests(source):
    for node in ast.walk(ast.parse(source)):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.startswith("test_"):
            start = min([node.lineno, *(d.lineno for d in node.decorator_list)])
            yield start - 1, node.lineno, node.name


def build_index():
    sections = {}
    # GitHub line anchors count LF bytes. The RFC also contains a vertical tab
    # inside a table cell, which str.splitlines() would count as a new line.
    rfc_lines = (ROOT / RFC).read_bytes().decode("utf-8").split("\n")
    for line, heading in enumerate(rfc_lines, 1):
        match = re.match(r"^#{2,3} (\d+(?:\.\d+)?)(?:\\\.)?\s", heading)
        if match:
            sections[match[1]] = line
    doc = ["# RFC 逐测试索引", "",
           "每行对应一个测试函数，链接分别指向测试声明和规范原文。参数化用例共用所在函数的条款。", "",
           "运行 `python3 scripts/rfc-test-index.py` 更新此文件；CI 使用 `--check` 校验。", ""]
    files = sorted([*(ROOT / "crates").rglob("*.rs"), *(ROOT / "tests").rglob("*.py")])
    cases = {}
    count = 0
    for path in files:
        source = path.read_bytes().decode("utf-8")
        lines = source.split("\n")
        tests = list(rust_tests(lines) if path.suffix == ".rs" else python_tests(source))
        if not tests:
            continue
        rel = path.relative_to(ROOT).as_posix()
        doc.extend([f"## `{rel}`", "", "| 测试 | RFC 条款 |", "| --- | --- |"])
        for start, line, name in tests:
            annotation = lines[start-1].strip() if start else ""
            match = re.fullmatch(r"(?:#|//) RFC (§\d+(?:\.\d+)?(?:, §\d+(?:\.\d+)?)*)", annotation)
            if not match:
                raise ValueError(f"{rel}:{line} {name} needs a per-test RFC annotation")
            links = []
            for token in match[1].split(", "):
                section = token[1:]
                if section not in sections:
                    raise ValueError(f"{rel}:{line} {name} cites missing RFC section {token}")
                links.append(f"[{token}](../{RFC}?plain=1#L{sections[section]})")
            doc.append(f"| [`{name}`](../{rel}#L{line}) | {' · '.join(links)} |")
            link = f"[`{name}`](../{rel}#L{line})"
            cases.setdefault(name, []).append(link)
            cases.setdefault(f"{rel}::{name}", []).append(link)
            count += 1
        doc.append("")
    if not cases:
        raise ValueError("no tests discovered")
    return "\n".join(doc), build_requirements(sections, rfc_lines, cases), count


def build_requirements(sections, rfc_lines, cases):
    doc = ["# RFC 条款验收表", "",
           "每行链接到规范原句、可复跑的断言或待裁决的 issue。测试链接表示已有用例，执行结果见对应提交的 CI。",
           "§1.1 约定关键词，§10.1 给出部署建议，§11 为非规范性扩展。§10.2 的 WSL2 验证暂缓。", "",
           "维护时编辑 `tests/rfc-requirements.tsv`，再运行 `python3 scripts/rfc-test-index.py`。"]
    tables = {}
    for n, row in enumerate(REQUIREMENTS.read_text().splitlines(), 1):
        if not row or row.startswith("#"):
            continue
        section, excerpt, description, refs = row.split("\t")
        if section not in sections:
            raise ValueError(f"{REQUIREMENTS.name}:{n}: unknown section {section}")
        start = sections[section]
        end = next((line for line in sections.values() if line > start), len(rfc_lines) + 1)
        matches = [line for line in range(start, end) if excerpt in rfc_lines[line - 1]]
        if len(matches) != 1:
            raise ValueError(f"{REQUIREMENTS.name}:{n}: expected one match for §{section} {excerpt!r}, got {matches}")
        links = []
        for ref in refs.split():
            if ref in cases:
                if len(cases[ref]) != 1:
                    raise ValueError(f"{REQUIREMENTS.name}:{n}: qualify {ref!r} with its source path")
                links.append(cases[ref][0])
            elif re.fullmatch(r"#[1-9]\d*", ref):
                links.append(f"[{ref}](https://github.com/Luthics/os-agent/issues/{ref[1:]})")
            elif ref in ("crates/os-agent-core/Cargo.toml", ".github/workflows/ci.yml") and (ROOT / ref).is_file():
                links.append(f"[`{ref}`](../{ref})")
            else:
                raise ValueError(f"{REQUIREMENTS.name}:{n}: unknown evidence {ref!r}")
        if not links:
            raise ValueError(f"{REQUIREMENTS.name}:{n}: missing evidence")
        tables.setdefault(section, []).append(
            f"| [{description}](../{RFC}?plain=1#L{matches[0]}) | {' · '.join(links)} |")
    normative = {"2"} | {s for s in sections if "." in s and not s.startswith("11.")}
    missing = normative - tables.keys() - {"1.1", "10.1"}
    if missing:
        raise ValueError(f"missing RFC sections in requirement index: {sorted(missing)}")
    for section in sections:
        if section in tables:
            doc.extend(["", f"## §{section}", "", "| 条款与验收点 | 测试或待裁决问题 |", "| --- | --- |"])
            doc.extend(tables[section])
    return "\n".join(doc) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        content, reverse, count = build_index()
    except ValueError as err:
        sys.exit(str(err))
    for path, rendered in [(INDEX, content), (REVERSE_INDEX, reverse)]:
        if args.check:
            if not path.exists() or path.read_text() != rendered:
                sys.exit(f"{path.name} is stale. Run python3 scripts/rfc-test-index.py")
        else:
            path.write_text(rendered)
    print(f"{count} tests linked to RFC clauses")
