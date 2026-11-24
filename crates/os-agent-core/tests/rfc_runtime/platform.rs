//! §10.2. Exercise strict and best-effort startup independent of the user's files.
use super::{Fixture, expect_code};
use std::fs;

// RFC §10.2
#[test]
fn strict_requires_landlock_and_best_effort_warns_when_the_kernel_lacks_it() {
    let fx = Fixture::new();
    fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
    let available = os_agent_core::platform::probe_capabilities()
        .landlock_abi
        .is_some();
    for policy in ["--sandbox=strict", "--sandbox=best-effort"] {
        let out = fx.run(fx.agent().args([policy, "-q", "case.agent", "--", "start"]));
        let fail = !available && policy == "--sandbox=strict";
        expect_code(&out, if fail { 1 } else { 0 });
        assert_eq!(fx.capture.exists(), !fail);
        if !available {
            assert!(!out.stderr.is_empty(), "{policy}");
        }
        if fx.capture.exists() {
            fs::remove_file(&fx.capture).unwrap();
        }
    }
}

// RFC §10.2
#[cfg(target_os = "linux")]
#[test]
fn supported_landlock_allows_writes_to_the_three_roots_and_keeps_other_fixture_files_readable() {
    use super::{TempDir, jsonl, scripted_provider, success_text, tool, with_tools};
    use std::path::Path;
    if os_agent_core::platform::probe_capabilities()
        .landlock_abi
        .is_none()
    {
        // The separate startup test exercises both specified branches on such kernels.
        return;
    }
    let fx = Fixture::new();
    let root = &fx.dir.0;
    // /dev/shm on Linux is writable independently of the allowed /tmp, working
    // directory and runtime directory. Only files in newly allocated test dirs are used.
    let other = TempDir::under(Path::new("/dev/shm"));
    let runtime = other.0.join("run");
    fs::create_dir(&runtime).unwrap();
    let outsider = other.0.join("outside.txt");
    fs::write(&outsider, "readable").unwrap();
    let tmp = TempDir::under(Path::new("/tmp"));
    let scratch = tmp.0.join("scratch");
    let project = other.0.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("case.agent"), "body").unwrap();
    // Each marker below distinguishes a denied operation from a child failing to launch.
    let cmd = r#"printf cwd > allowed
printf tmp > "$FIXTURE_TMP"
(printf runtime > "$XDG_RUNTIME_DIR/output") 2>/dev/null || :
readable=$(/bin/cat "$FIXTURE_OTHER")
printf '%s' "$readable"
if (printf replaced > "$FIXTURE_OTHER") 2>/dev/null; then printf ' wrote-outside'; fi"#;
    scripted_provider(
        &fx,
        &[with_tools([tool("check", cmd)]), success_text("done")],
    );
    let out = fx.run(
        fx.agent()
            .current_dir(&project)
            .env("XDG_RUNTIME_DIR", &runtime)
            .env("FIXTURE_OTHER", &outsider)
            .env("FIXTURE_TMP", &scratch)
            .args([
                "--sandbox=strict",
                "--shell",
                "/bin/sh",
                "-q",
                "-y",
                "case.agent",
                "--",
                "start",
            ]),
    );
    expect_code(&out, 0);
    let ir = jsonl(&root.join("request-2"));
    let reply = ir.last().unwrap();
    assert_ne!(reply["is_error"], true, "{reply}");
    assert!(
        reply["text"].as_str().unwrap().ends_with("readable"),
        "{reply}"
    );
    for (file, expected) in [
        (project.join("allowed"), "cwd"),
        (scratch, "tmp"),
        (outsider, "readable"),
        (runtime.join("output"), "runtime"),
    ] {
        assert_eq!(
            fs::read_to_string(&file).unwrap_or_else(|e| panic!("read {file:?}: {e}")),
            expected
        );
    }
}
