"""Build the per-test index for the v1.1.1 RFC."""
import argparse
import ast
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
RFC = Path("docs/v1.1.1-RFC.md")
INDEX = ROOT / "tests/RFC-cases.md"


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
    # GitHub line anchors count LF bytes, including in Markdown tables.
    rfc_lines = (ROOT / RFC).read_bytes().decode("utf-8").split("\n")
    for line, heading in enumerate(rfc_lines, 1):
        match = re.match(r"^#{2,3} (\d+(?:\.\d+)?)\.?\s", heading)
        if match:
            sections[match[1]] = line
    doc = ["# v1.1.1 RFC 逐测试索引", "",
           "每行对应一个测试函数，链接分别指向测试声明和规范原文。参数化用例共用所在函数的条款。", "",
           "v1.0 的 198 个用例保存在[上一版测试索引](https://github.com/Luthics/anix/blob/45e55a9f16f6bead6a933c67051b94b0acb753a0/tests/RFC-cases.md)。", "",
           "运行 `python3 scripts/rfc-test-index.py` 更新此文件。CI 使用 `--check` 校验。", ""]
    files = sorted([*(ROOT / "crates").rglob("*.rs"), *(ROOT / "tests").rglob("*.py")])
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
            match = re.fullmatch(r"(?:#|//) RFC v1\.1(?:\.1)? (§\d+(?:\.\d+)?(?:, §\d+(?:\.\d+)?)*)", annotation)
            if not match:
                raise ValueError(f"{rel}:{line} {name} needs a per-test RFC annotation")
            links = []
            for token in match[1].split(", "):
                section = token[1:]
                if section not in sections:
                    raise ValueError(f"{rel}:{line} {name} cites missing RFC section {token}")
                links.append(f"[{token}](../{RFC}?plain=1#L{sections[section]})")
            doc.append(f"| [`{name}`](../{rel}#L{line}) | {' · '.join(links)} |")
            count += 1
        doc.append("")
    if not count:
        raise ValueError("no tests discovered")
    return "\n".join(doc), count


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        content, count = build_index()
    except ValueError as err:
        sys.exit(str(err))
    if args.check:
        if not INDEX.is_file() or INDEX.read_text() != content:
            sys.exit(f"{INDEX.name} is stale. Run python3 scripts/rfc-test-index.py")
    else:
        INDEX.write_text(content)
    print(f"{count} tests linked to v1.1.1 RFC clauses")
