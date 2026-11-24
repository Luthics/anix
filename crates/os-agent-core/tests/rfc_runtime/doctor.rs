//! §10.3. Exercise the installed layout and configuration through the public CLI.
use super::{Fixture, expect_code};
use std::fs;
use std::os::unix::fs::PermissionsExt;

const BINARIES: [&str; 5] = [
    ".local/bin/agent",
    ".local/libexec/agent/msh",
    ".local/libexec/agent/agent-provider-openai-responses",
    ".local/libexec/agent/tools/aview",
    ".local/libexec/agent/tools/aclr",
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
    fs::write(fx.dir.0.join("config/agent/config.toml"), conf).unwrap();
    fx
}

#[test]
fn each_missing_or_non_executable_private_binary_is_identified() {
    let fx = installation("", "");
    expect_code(&fx.run(fx.agent().arg("--doctor")), 0);
    for name in BINARIES {
        let path = fx.dir.0.join(name);
        for missing in [false, true] {
            if missing {
                fs::remove_file(&path).unwrap();
            } else {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            }
            let out = fx.run(fx.agent().arg("--doctor"));
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
    expect_code(&fx.run(fx.agent().arg("--doctor")), 0);
}

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
        let mut cmd = fx.agent();
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

#[test]
fn invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure() {
    for name in ["config/agent/config.toml", ".agent/config.toml"] {
        let fx = installation("", "");
        let path = fx.dir.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[not-a-table").unwrap();
        let out = fx.run(fx.agent().arg("--doctor"));
        expect_code(&out, 1);
        assert!(String::from_utf8(out.stderr).unwrap().contains(name));
    }
    let fx = installation("", "");
    fs::write(
        fx.dir.0.join("run"),
        "regular file prevents directory creation",
    )
    .unwrap();
    let out = fx.run(fx.agent().arg("--doctor"));
    expect_code(&out, 1);
    assert!(!out.stderr.is_empty());
}
