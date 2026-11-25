"""Link current RFC requirements to their conformance tests in both directions."""
import argparse
import ast
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
RFC = Path("docs/RFC.md")
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
    # GitHub line anchors count LF bytes, including in Markdown tables.
    rfc_lines = (ROOT / RFC).read_bytes().decode("utf-8").split("\n")
    version_line = next((line for line in rfc_lines if line.startswith("**版本**")), "")
    version = re.search(r"v\d+\.\d+\.\d+", version_line)
    if not version:
        raise ValueError(f"{RFC} is missing its version")
    version = version.group()
    for line, heading in enumerate(rfc_lines, 1):
        match = re.match(r"^#{2,3} (\d+(?:\.\d+)?)\.?\s", heading)
        if match:
            sections[match[1]] = line
    doc = [f"# {version} RFC 逐测试索引", "",
           "每行对应一个测试函数，链接分别指向测试声明和规范原文。参数化用例共用所在函数的条款。", "",
           "按规范原句查找断言或相关 issue，见[条款验收表](RFC-requirements.md)。", "",
           "历史快照见 [v1.1.2 的 277 个用例](https://github.com/Luthics/anix/blob/e4fb614f314625ae282bd2fcb9b7e80312514946/tests/RFC-cases.md)、[v1.1.1 的 262 个用例](https://github.com/Luthics/anix/blob/0b11a3c60218b9313f9e81a806c9a471906766eb/tests/RFC-cases.md)。v1.0 的 198 个用例保存在[v1.0 测试索引](https://github.com/Luthics/anix/blob/45e55a9f16f6bead6a933c67051b94b0acb753a0/tests/RFC-cases.md)。", "",
           "运行 `python3 scripts/rfc-test-index.py` 更新此文件。CI 使用 `--check` 校验。", ""]
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
            match = re.fullmatch(r"(?:#|//) RFC v\d+\.\d+(?:\.\d+)? (§\d+(?:\.\d+)?(?:, §\d+(?:\.\d+)?)*)", annotation)
            if not match:
                raise ValueError(f"{rel}:{line} {name} needs a per-test RFC annotation")
            links = []
            for token in match[1].split(", "):
                section = token[1:]
                if section not in sections:
                    raise ValueError(f"{rel}:{line} {name} cites missing RFC section {token}")
                links.append(f"[{token}](../{RFC}?plain=1#L{sections[section]})")
            test_link = f"[`{name}`](../{rel}#L{line})"
            doc.append(f"| {test_link} | {' · '.join(links)} |")
            cases.setdefault(name, []).append(test_link)
            cases.setdefault(f"{rel}::{name}", []).append(test_link)
            count += 1
        doc.append("")
    if not count:
        raise ValueError("no tests discovered")
    return "\n".join(doc), build_requirements(sections, rfc_lines, cases, version), count, version


def build_requirements(sections, rfc_lines, cases, version):
    doc = [f"# {version} RFC 条款验收表", "",
           "每行给出一项可观察行为。第一列链接到 RFC 原文，第二列链接到执行断言或待裁决的 issue。通过与失败见对应提交的 CI。", "",
           "本表按条款逐步补录。[逐测试索引](RFC-cases.md)列出当前套件的全部测试函数。编辑 `tests/rfc-requirements.tsv` 后运行 `python3 scripts/rfc-test-index.py` 更新两份索引。"]
    tables = {}
    seen = set()
    for n, row in enumerate(REQUIREMENTS.read_text().splitlines(), 1):
        if not row or row.startswith("#"):
            continue
        where = f"{REQUIREMENTS.name}:{n}"
        section, excerpt, behavior, evidence = row.split("\t")
        if section not in sections or not excerpt or not behavior:
            raise ValueError(f"{where}: expected a section, unique excerpt and observable behavior")
        key = (section, behavior)
        if key in seen:
            raise ValueError(f"{where}: duplicate behavior {key}")
        seen.add(key)
        start = sections[section]
        stop = next((line for line in sections.values() if line > start), len(rfc_lines) + 1)
        matches = [line for line in range(start, stop) if excerpt in rfc_lines[line - 1]]
        if len(matches) != 1:
            raise ValueError(f"{where}: expected exactly one RFC line matching §{section} {excerpt!r}, got {matches}")
        links = []
        for ref in evidence.split():
            if re.fullmatch(r"#[1-9]\d*", ref):
                links.append(f"[{ref}](https://github.com/Luthics/anix/issues/{ref[1:]})")
            elif ref == "TODO":
                links.append("待补专项断言")
            elif ref in cases and len(cases[ref]) == 1:
                links.append(cases[ref][0])
            else:
                raise ValueError(f"{where}: unknown or ambiguous test {ref!r}")
        if not links:
            raise ValueError(f"{where}: expected tests, issue references or TODO")
        tables.setdefault(section, []).append(
            f"| [{behavior}](../{RFC}?plain=1#L{matches[0]}) | {' · '.join(links)} |")
    if not tables:
        raise ValueError("empty requirement index")
    for section in sections:
        if section in tables:
            doc.extend(["", f"## §{section}", "", "| 可验收行为 | 测试与问题 |", "| --- | --- |", *tables[section]])
    return "\n".join(doc) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        content, reverse, count, version = build_index()
    except ValueError as err:
        sys.exit(str(err))
    for path, rendered in ((INDEX, content), (REVERSE_INDEX, reverse)):
        if args.check:
            if not path.is_file() or path.read_text() != rendered:
                sys.exit(f"{path.name} is stale. Run python3 scripts/rfc-test-index.py")
        else:
            path.write_text(rendered)
    print(f"{count} tests linked to {version} RFC clauses")
