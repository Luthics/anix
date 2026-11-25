//! §10.3. Exercise the installed layout and configuration through the public CLI.
use super::{Fixture, expect_code};
use std::fs;
use std::os::unix::fs::PermissionsExt;

const BINARIES: [&str; 5] = [
    ".local/bin/anix",
    ".local/libexec/anix/msh",
    ".local/libexec/anix/anix-provider-openai-responses",
    ".local/libexec/anix/tools/aview",
    ".local/libexec/anix/tools/aclr",
];

fn installation(key_env: &str, key: &str) -> Fixture {
    let fx = Fixture::new();
    for name in BINARIES {
        let path = fx.dir.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let conf = format!(
        "[defaults]\nprovider='openai-responses'\n[providers.openai-responses]\napi_key_env='{key_env}'\napi_key='{key}'\n"
    );
    fs::write(fx.dir.0.join("config/anix/config.toml"), conf).unwrap();
    fx
}

// RFC v1.1.1 §10.3, §2, §8.1
#[test]
fn each_missing_or_non_executable_private_binary_is_identified() {
    let fx = installation("", "");
    expect_code(&fx.run(fx.anix().arg("--doctor")), 0);
    assert_eq!(
        fs::metadata(fx.dir.0.join("run/anix"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    for name in BINARIES {
        let path = fx.dir.0.join(name);
        for missing in [false, true] {
            if missing {
                fs::remove_file(&path).unwrap();
            } else {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            }
            let out = fx.run(fx.anix().arg("--doctor"));
            expect_code(&out, 1);
            let stderr = String::from_utf8(out.stderr).unwrap();
            assert!(
                stderr.contains(name),
                "missing diagnostic for {name}: {stderr}"
            );
            assert!(
                stderr.contains("chmod") || stderr.contains("install"),
                "{stderr}"
            );
        }
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    expect_code(&fx.run(fx.anix().arg("--doctor")), 0);
}

// RFC v1.1.1 §10.3
#[test]
fn doctor_distinguishes_required_missing_empty_and_fallback_credentials() {
    for (key_env, key, value, expected) in [
        ("TEST_DOCTOR_KEY", "", None, 1),
        ("TEST_DOCTOR_KEY", "", Some(""), 1),
        ("TEST_DOCTOR_KEY", "", Some("env-placeholder"), 0),
        ("TEST_DOCTOR_KEY", "config-placeholder", Some(""), 0),
        ("", "config-placeholder", None, 0),
        ("", "", None, 0),
    ] {
        let fx = installation(key_env, key);
        let mut cmd = fx.anix();
        cmd.arg("--doctor");
        if let Some(value) = value {
            cmd.env("TEST_DOCTOR_KEY", value);
        }
        let out = fx.run(&mut cmd);
        expect_code(&out, expected);
        if expected == 1 {
            assert!(
                String::from_utf8(out.stderr)
                    .unwrap()
                    .contains("TEST_DOCTOR_KEY")
            );
        }
    }
}

// RFC v1.1.1 §10.3
#[test]
fn invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure() {
    for name in ["config/anix/config.toml", ".anix/config.toml"] {
        let fx = installation("", "");
        let path = fx.dir.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[not-a-table").unwrap();
        let out = fx.run(fx.anix().arg("--doctor"));
        expect_code(&out, 1);
        assert!(String::from_utf8(out.stderr).unwrap().contains(name));
    }
    let fx = installation("", "");
    fs::write(
        fx.dir.0.join("run"),
        "regular file prevents directory creation",
    )
    .unwrap();
    let out = fx.run(fx.anix().arg("--doctor"));
    expect_code(&out, 1);
    assert!(!out.stderr.is_empty());
}

// RFC v1.1.1 §8.1, §8.2, §10.3
#[test]
fn doctor_checks_the_selected_project_provider_and_its_credentials() {
    let fx = installation("UNSET_DEFAULT_KEY", "");
    let root = &fx.dir.0;
    let project = root.join(".anix");
    fs::create_dir(&project).unwrap();
    let config = "[defaults]\nprovider='project-default'\n[providers.project-default]\nbin='project-backend'\napi_key_env='PROJECT_FIXTURE_KEY'\n";
    fs::write(project.join("config.toml"), config).unwrap();
    let selected = root.join(".local/libexec/anix/project-backend");
    super::executable(&selected, "#!/bin/sh\nexit 0\n");
    let mut cmd = fx.anix();
    cmd.arg("--doctor");
    let missing = fx.run(&mut cmd);
    expect_code(&missing, 1);
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("PROJECT_FIXTURE_KEY")
    );
    cmd.env("PROJECT_FIXTURE_KEY", "fixture-value");
    expect_code(&fx.run(&mut cmd), 0);
    fs::remove_file(&selected).unwrap();
    let absent = fx.run(&mut cmd);
    expect_code(&absent, 1);
    assert!(
        String::from_utf8(absent.stderr)
            .unwrap()
            .contains("project-backend")
    );
}

// RFC v1.1.1 §10.3
#[test]
fn doctor_reports_missing_msh_before_a_malformed_global_config() {
    let fx = installation("", "");
    let root = &fx.dir.0;
    let msh = root.join(".local/libexec/anix/msh");
    fs::remove_file(&msh).unwrap();
    fs::write(root.join("config/anix/config.toml"), "[invalid-table\n").unwrap();
    // Merge the output streams at process launch to preserve publication order.
    let out = fx.run(fx.command_for("/bin/sh").args([
        "-c",
        r#"exec "$1" --doctor 2>&1"#,
        "fixture",
        env!("CARGO_BIN_EXE_anix"),
    ]));
    expect_code(&out, 1);
    let report = String::from_utf8(out.stdout).unwrap();
    let first = report
        .find(".local/libexec/anix/msh")
        .expect("missing binary should be diagnosed");
    // Returning on the first failed check is allowed. If both problems are
    // reported, the required binary check comes before the TOML diagnosis.
    if let Some(second) = report.find("config.toml") {
        assert!(
            first < second,
            "doctor reported config before missing msh:\n{report}"
        );
    }
}
