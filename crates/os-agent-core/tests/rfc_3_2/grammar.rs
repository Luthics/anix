//! §3.2 rules 1–4. Expected prompts and fields come from fixtures, independently
//! of the implementation. The layout matrix varies one grammar choice at a time.

use os_agent_core::frontmatter::parse_script_str;
use serde_json::json;

// RFC §3.2
#[test]
fn layout_matrix_for_line_ending_shebang_header_body_and_eof() {
    let bodies = [
        "instruction",
        "  首行  \n\n\t🦀 e\u{301}  ",
        "first\n---\nunknown: ordinary prompt text\n#!another line",
    ];
    for eol in ["\n", "\r\n"] {
        for shebang in ["", "#!\n", "#!/usr/bin/env agent --example\n"] {
            for (header, expected_name) in [
                ("", None),
                ("---\n---\n", None),
                ("---\nname: demo\n---\n", Some("demo")),
            ] {
                for body in bodies {
                    for eof in ["", "\n"] {
                        let source = format!("{shebang}{header}{body}{eof}").replace('\n', eol);
                        let parsed = parse_script_str(&source, "fixture")
                            .unwrap_or_else(|e| panic!("rejected {source:?}: {e}"));
                        assert_eq!(
                            parsed.frontmatter.name.as_deref(),
                            expected_name,
                            "{source:?}"
                        );
                        assert_eq!(
                            parsed.prompt.lines().collect::<Vec<_>>(),
                            body.lines().collect::<Vec<_>>(),
                            "{source:?}"
                        );
                    }
                }
            }
        }
    }
}

// RFC §3.2
#[test]
fn only_a_first_line_beginning_with_hash_bang_is_removed() {
    for (source, body) in [
        ("", ""),
        ("#!", ""),
        ("#!/usr/bin/env agent", ""),
        ("#!first\n#!second", "#!second"),
        (" #!indented", " #!indented"),
        ("x#!embedded", "x#!embedded"),
        ("# !separated", "# !separated"),
        ("first\n#!later", "first\n#!later"),
        (
            "#!first\n#!second\n---\nname: literal\n---",
            "#!second\n---\nname: literal\n---",
        ),
    ] {
        let parsed = parse_script_str(source, "fixture").unwrap();
        assert_eq!(parsed.prompt, body, "{source:?}");
        assert!(parsed.frontmatter.name.is_none(), "{source:?}");
    }
}

// RFC §3.2
#[test]
fn opening_delimiter_requires_the_designated_line_and_exact_marker() {
    for marker in ["--", "----", "—", "x---", "---x", "--- # comment"] {
        let source = format!("{marker}\nname: literal\n---\nbody");
        let parsed = parse_script_str(&source, "fixture").unwrap();
        assert_eq!(parsed.prompt, source);
        assert!(parsed.frontmatter.name.is_none(), "{source:?}");
    }
    for prefix in ["before\n", "\n", "#!interpreter\n\n"] {
        let literal = "---\nname: literal\n---\nbody";
        let source = format!("{prefix}{literal}");
        let parsed = parse_script_str(&source, "fixture").unwrap();
        assert!(parsed.frontmatter.name.is_none(), "{source:?}");
        // Issue #1 leaves blank-edge trimming open. The misplaced block must remain text.
        assert!(
            parsed.prompt.ends_with(literal),
            "{source:?}: {:?}",
            parsed.prompt
        );
    }
}

// RFC §3.2
#[test]
fn first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed() {
    for eol in ["\n", "\r\n"] {
        let header = "---\nx-note: |\n  ---\n  ordinary YAML scalar\nname: demo\n---";
        let body = "   first line  \n\t\n\n---\nname: not-a-field\nlast line   ";
        for (source, expected) in [
            (header.to_owned(), ""),
            (format!("{header}\n\n\n"), ""),
            (format!("{header}\n\n\n{body}\n\n\n"), body),
        ] {
            let source = source.replace('\n', eol);
            let parsed = parse_script_str(&source, "fixture").unwrap();
            assert_eq!(parsed.frontmatter.name.as_deref(), Some("demo"));
            assert_eq!(
                parsed.prompt.lines().collect::<Vec<_>>(),
                expected.lines().collect::<Vec<_>>(),
                "{source:?}"
            );
        }
    }
}

// RFC §3.2, §3.3
#[test]
fn yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields() {
    let source = r#"---
"name": demo
x-before: null
params:
  truth: true
  switch: on
  answer: yes
  nested: {new-key: [3, false]}
x-after:
  name: nested
  unknown-field: [a, b]
model: 'example: v1'
---
unknown-field: literal prompt"#;
    let parsed = parse_script_str(source, "fixture").unwrap();
    assert_eq!(parsed.frontmatter.name.as_deref(), Some("demo"));
    assert_eq!(parsed.frontmatter.model.as_deref(), Some("example: v1"));
    assert_eq!(parsed.frontmatter.params["truth"], json!(true));
    assert_eq!(parsed.frontmatter.params["switch"], json!("on"));
    assert_eq!(parsed.frontmatter.params["answer"], json!("yes"));
    assert_eq!(
        parsed.frontmatter.params["nested"],
        json!({"new-key": [3, false]})
    );
    assert_eq!(parsed.prompt, "unknown-field: literal prompt");
}

// RFC §3.2
#[test]
fn incomplete_delimiters_and_malformed_yaml_fail_at_the_header() {
    for source in [
        "---",
        "#!agent\r\n---\r\nname: demo",
        "---\nname: demo\n----",
        "---\nname: demo\n--- # comment",
        "---\nname: 'unterminated\n---\nbody",
        "---\nx-note: [a, b\n---\nbody",
    ] {
        assert!(
            parse_script_str(source, "fixture").is_err(),
            "accepted {source:?}"
        );
    }
}
