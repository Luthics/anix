//! §3.2 rules 1, 3 and 4 through the shipping CLI. A local executable records
//! exactly the IR seen by the provider. Its capture file also witnesses invocation.

use super::fixture::Fixture;
use serde_json::{Value, json};
use std::fs;

fn run_script(fixture: &Fixture, script: &[u8]) -> std::process::Output {
    fs::write(fixture.dir.0.join("case.anix"), script).unwrap();
    fixture.run(
        fixture
            .anix()
            .args(["--stderr", "explicit", "case.anix", "--", "user input"]),
    )
}

impl Fixture {
    fn assert_rejected(&self, label: &str, source: &[u8]) {
        let result = run_script(self, source);
        let (code, stdout, stderr) = (result.status.code(), result.stdout, result.stderr);
        assert_eq!(
            code,
            Some(2),
            "{label}: {}",
            String::from_utf8_lossy(&stderr)
        );
        assert!(
            stdout.is_empty(),
            "{label}: diagnostic on stdout: {stdout:?}"
        );
        assert!(
            stderr.iter().any(|b| !b.is_ascii_whitespace()),
            "{label}: missing diagnostic"
        );
        assert!(
            !self.capture.try_exists().unwrap(),
            "{label}: invalid script reached provider"
        );
    }
}

// RFC v1.1 §3.2, §9.1
#[test]
fn crlf_script_reaches_the_provider_as_the_intended_system_message() {
    let fixture = Fixture::new();
    let source = "#!fixture --not-the-prompt\r\n---\r\nx-extra: [one, two]\r\nparams: {sample: 7}\r\n---\r\n\r\n  执行任务  \r\n\r\n第二行\r\n\r\n";
    let result = run_script(&fixture, source.as_bytes());
    let (code, stdout, stderr) = (result.status.code(), result.stdout, result.stderr);
    assert_eq!(code, Some(0), "{}", String::from_utf8_lossy(&stderr));
    assert!(stdout.is_empty());
    let ir = fs::read_to_string(fixture.capture).unwrap();
    let rows: Vec<Value> = ir
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(rows[0]["params"]["sample"], json!(7));
    assert_eq!(rows[1]["role"], "system");
    let prompt = rows[1]["text"].as_str().unwrap();
    assert_eq!(
        prompt.lines().collect::<Vec<_>>(),
        ["  执行任务  ", "", "第二行"]
    );
}

// RFC v1.1 §3.2, §3.3, §6.5
#[test]
fn unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position() {
    let fixture = Fixture::new();
    for key in [
        "unknown",
        "nAme",
        "max-turns",
        "x_private",
        "X-vendor",
        "extra.x-key",
    ] {
        for fields in [
            format!("{key}: wrong\nname: ok\nx-vendor: ok"),
            format!("name: ok\n{key}: wrong\nx-vendor: ok"),
            format!("name: ok\nx-vendor: ok\n{key}: wrong"),
        ] {
            let script = format!("---\n{fields}\n---\nbody");
            fixture.assert_rejected(&fields, script.as_bytes());
        }
    }
}

// RFC v1.1 §3.2, §6.5
#[test]
fn invalid_file_bytes_and_yaml_report_usage_errors_to_stderr() {
    let fixture = Fixture::new();
    for (case, source) in [
        ("invalid shebang bytes", b"#!\xff\nbody".as_slice()),
        (
            "invalid prompt bytes",
            b"---\r\nname: demo\r\n---\r\nbody\xc3",
        ),
        ("missing closing delimiter", b"---\nname: demo\n"),
        ("ill-formed YAML", b"---\nname: [\n---\nbody"),
    ] {
        fixture.assert_rejected(case, source);
    }
}

// RFC v1.1 §3.2, §3.3, §9.1
#[test]
fn yaml_block_scalar_chomping_survives_the_frontmatter_boundary() {
    let fixture = Fixture::new();
    let mut differences = Vec::new();
    for (style, expected) in [
        ("|-", "one\n---\n# text\nlast"),
        ("|", "one\n---\n# text\nlast\n"),
        ("|+", "one\n---\n# text\nlast\n\n"),
        (">-", "one --- # text last"),
        (">", "one --- # text last\n"),
        (">+", "one --- # text last\n\n"),
    ] {
        for eol in ["\n", "\r\n"] {
            // The final content line is terminated. Keep also has an empty
            // line to retain. Moving an unrelated field must preserve the value.
            let extra = if style.ends_with('+') { "\n" } else { "" };
            for tail in ["model: sample\n", ""] {
                let source = format!("---\nparams:\n  note: {style}\n    one\n    ---\n    # text\n    last\n{extra}{tail}---\nprompt").replace('\n', eol);
                let out = run_script(&fixture, source.as_bytes());
                assert_eq!(
                    out.status.code(),
                    Some(0),
                    "{}",
                    String::from_utf8_lossy(&out.stderr)
                );
                let capture = fs::read_to_string(&fixture.capture).unwrap();
                let mut rows = capture
                    .lines()
                    .map(|line| serde_json::from_str::<Value>(line).unwrap());
                let note = rows.next().unwrap()["params"]["note"].clone();
                assert_eq!(rows.next().unwrap()["text"], "prompt");
                if note != expected {
                    differences.push(format!(
                        "style={style}, eol={eol:?}, tail={tail:?}: {note} vs {expected:?}"
                    ));
                }
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

// RFC v1.1 §3.2, §3.3, §6.5
#[test]
fn duplicate_yaml_keys_are_invalid_even_inside_open_maps() {
    let fixture = Fixture::new();
    for fields in [
        "model: first\n\"model\": second",
        "params: {nested: {k: 1, 'k': 2}}",
        "x-fixture: one\n'x-fixture': two",
    ] {
        fixture.assert_rejected(fields, format!("---\n{fields}\n---\nprompt").as_bytes());
    }
}

// RFC v1.1 §3.2, §3.3, §9.1
#[test]
fn yaml_aliases_can_supply_typed_provider_params_from_an_extension_field() {
    let fixture = Fixture::new();
    let input = "---\nx-examples: &sample {enabled: true, notes: [off, 'a:b', null]}\nparams:\n  original: *sample\n  copied: *sample\n---\nprompt";
    let out = run_script(&fixture, input.as_bytes());
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ir = fs::read_to_string(&fixture.capture).unwrap();
    let header: Value = serde_json::from_str(ir.lines().next().unwrap()).unwrap();
    let item = json!({"enabled":true,"notes":["off","a:b",null]});
    assert_eq!(header["params"], json!({"original":item,"copied":item}));
}

// RFC v1.1 §3.2, §3.3, §5.5, §6.5
#[test]
fn vision_requires_a_yaml_boolean_and_leaves_provider_params_open() {
    let fixture = Fixture::new();
    for invalid in ["'false'", "1", "0", "null", "[]", "{}"] {
        let source = format!("---\nvision: {invalid}\n---\nprompt");
        fixture.assert_rejected(invalid, source.as_bytes());
    }
    for vision in [true, false] {
        let source = format!("---\nvision: {vision}\nparams: {{vision: 'custom'}}\n---\nprompt");
        let parsed = anix_core::frontmatter::parse_script_str(&source, "fixture").unwrap();
        assert_eq!(parsed.frontmatter.vision, Some(vision));
        let result = run_script(&fixture, source.as_bytes());
        assert_eq!(result.status.code(), Some(0), "{:?}", result.stderr);
        let captured = fs::read_to_string(&fixture.capture).unwrap();
        let header: Value = serde_json::from_str(captured.lines().next().unwrap()).unwrap();
        assert_eq!(header["params"]["vision"], "custom");
    }
}
