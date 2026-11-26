"""Read the discovery indexes from the first request actually delivered to a local Provider."""
from itertools import product
import json
from pathlib import Path
import re
from xml.etree import ElementTree

from anix_fixture import OfflineAnixCase, rfc_section


def declared_guides():
    declaration, = [line for line in rfc_section("9.6") if "**内置默认层（最低优先级）**" in line]
    names = re.search(r"内置默认指南（([^）]+)）", declaration)
    if names is None:
        raise ValueError("§9.6 is missing its builtin guide list")
    result = []
    for entry in names[1].split("、"):
        name = re.fullmatch(r"`([^`/]+\.md)`", entry.strip())
        if name is None or name[1] in result:
            raise ValueError(f"invalid or repeated §9.6 guide: {entry!r}")
        result.append(name[1])
    return result


class DiscoveryIndexes(OfflineAnixCase):
    def first_system(self):
        request = self.root / "request"
        request.unlink(missing_ok=True)
        run = self.invoke("-q", self.script, "--", "go")
        self.assertEqual(run.returncode, 0, run.stderr)
        rows = [json.loads(line) for line in request.read_bytes().splitlines()]
        return next(row["text"] for row in rows if row.get("role") == "system")

    def tips_paths(self, prompt):
        self.assertEqual(prompt.count("<runtime_tips>"), 1)
        self.assertEqual(prompt.count("</runtime_tips>"), 1)
        block = prompt.split("<runtime_tips>\n")[1].split("\n</runtime_tips>")[0]
        introduction, *paths = block.split("\n")
        self.assertFalse(introduction.startswith("/"))
        return paths

    def skill_entries(self, prompt):
        self.assertEqual(prompt.count("<available_skills>"), 1)
        start = prompt.index("<available_skills>")
        end = prompt.index("</available_skills>", start) + len("</available_skills>")
        return ElementTree.fromstring(prompt[start:end]).findall("skill")

    # RFC §9.6-05 — 对照 RFC 内置名单，核对默认及自定义 XDG 状态目录实际释放的指南和 Provider 收到的绝对路径。
    def test_declared_builtin_guides_match_materialized_files_and_the_first_request(self):
        names = sorted(declared_guides())
        for state in (self.root / ".local/state", self.root / "custom state"):
            with self.subTest(state=state.name):
                if state.name == "custom state":
                    self.env["XDG_STATE_HOME"] = str(state)
                folder = state.resolve() / "anix/builtin-tips"
                self.assertFalse(folder.exists())
                prompt = self.first_system()
                # RFC-ASSERT §9.6-05
                self.assertEqual(sorted(p.name for p in folder.glob("*.md") if p.is_file()), names)
                # RFC-ASSERT §9.6-05
                self.assertEqual(self.tips_paths(prompt), [str(folder / name) for name in names])
                for name in names:
                    self.assertGreater((folder / name).stat().st_size, 0)

    # RFC §9.6-06 — 对同一内置指南交叉测试全局和项目层的缺席、替换、空文件及 /dev/null 屏蔽，核对整个索引。
    # RFC §9.6-07 — 逐项比对跨目录、大小写与空格的路径排序，排除隐藏文件、子目录、其他后缀和正文。
    def test_tip_masks_overrides_and_sort_order_across_both_user_layers(self):
        config_home = self.root / "active config"
        global_dir = config_home / "anix/tips"
        project_dir = self.root / ".anix/tips"
        for directory in (global_dir, project_dir):
            directory.mkdir(parents=True)
        (config_home / "anix/config.toml").write_bytes(self.config.read_bytes())
        self.env["XDG_CONFIG_HOME"] = str(config_home)
        # A file under the old config root reveals even an accidental union of both roots.
        stale = self.config.parent / "tips"
        stale.mkdir()
        (stale / "unexpected.md").write_text("inactive")
        body = "TIP-BODY-ON-DISK-ONLY"
        builtin = self.root.resolve() / ".local/state/anix/builtin-tips"
        entries = {name: builtin / name for name in declared_guides()}
        target = next(iter(entries))
        for directory, name in ((global_dir, "Z-first.md"), (project_dir, "a guide.md")):
            path = directory / name
            path.write_text(body)
            entries[name] = path.resolve()
        (project_dir / "nested.md").mkdir()
        for ignored in (".hidden.md", "discard.txt", "nested.md/inner.md"):
            (project_dir / ignored).write_text(body)

        states = ("absent", "file", "empty", "devnull")
        for global_state, project_state in product(states, repeat=2):
            with self.subTest(global_state=global_state, project_state=project_state):
                expected = entries.copy()
                for directory, state in ((global_dir, global_state), (project_dir, project_state)):
                    path = directory / target
                    path.unlink(missing_ok=True)
                    if state == "file":
                        path.write_text(body)
                        expected[target] = path.resolve()
                    elif state in ("empty", "devnull"):
                        if state == "empty":
                            path.touch()
                        else:
                            path.symlink_to("/dev/null")
                        expected.pop(target, None)
                prompt = self.first_system()
                paths = self.tips_paths(prompt)
                # RFC-ASSERT §9.6-06
                self.assertEqual([line for line in paths if Path(line).name == target],
                                 [str(expected[target])] if target in expected else [])
                # RFC-ASSERT §9.6-07
                self.assertEqual(paths, [str(expected[name]) for name in sorted(expected)])
                # RFC-ASSERT §9.6-07
                self.assertNotIn(body, prompt)

    # RFC §9.4-05 — 仅从 SKILL.md 开头的 YAML 读取两个必需字段，缺字段、正文中的伪字段和其他元数据均不进入索引。
    def test_skill_index_requires_both_frontmatter_fields_and_excludes_private_text(self):
        private = "SKILL-PRIVATE-TEXT"
        guides = {
            "plain": f"name: plain\ndescription: {private}\n",
            "no-name": f"---\ndescription: one\n---\nname: {private}\n",
            "no-description": f"---\nname: two\n---\ndescription: {private}\n",
            "valid": f"---\ndescription: >-\n  Read A&B\n  with <例>.\nignored: {private}\n"
                     f"name: chosen\n---\n{private}\n",
        }
        for directory, content in guides.items():
            path = self.root / ".skills" / directory / "SKILL.md"
            path.parent.mkdir(parents=True)
            path.write_text(content)
        prompt = self.first_system()
        actual = [(entry.attrib, entry.text) for entry in self.skill_entries(prompt)]
        valid = (self.root / ".skills/valid/SKILL.md").resolve()
        # RFC-ASSERT §9.4-05
        self.assertEqual(actual, [({"name": "chosen", "path": str(valid)}, "Read A&B with <例>.")])
        # RFC-ASSERT §9.4-05
        self.assertNotIn(private, prompt)

    # RFC §9.4-06 — 解析实际 XML 后逐项打开技能文件，检查目录名中的空格、Tab、LF、CR 是否仍指向原文件。
    def test_skill_xml_paths_reopen_files_with_posix_whitespace(self):
        guides = {}
        for name, separator in (("space", " "), ("tab", "\t"), ("lf", "\n"), ("cr", "\r")):
            guide = self.root / ".skills" / f"{name}{separator}guide" / "SKILL.md"
            guide.parent.mkdir(parents=True)
            guide.write_text(f"---\nname: {name}\ndescription: read {name}\n---\nbody-{name}")
            guides[name] = guide.resolve()
        entries = self.skill_entries(self.first_system())
        self.assertCountEqual([entry.get("name") for entry in entries], guides.keys())
        for entry in entries:
            name = entry.get("name")
            with self.subTest(name=name):
                path = Path(entry.attrib["path"])
                self.assertEqual(path, guides[name])
                self.assertTrue(path.read_text().endswith(f"body-{name}"))
