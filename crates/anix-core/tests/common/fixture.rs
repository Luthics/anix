use std::fs::{self, File, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::TempDir;

pub struct Fixture {
    pub dir: TempDir,
    pub capture: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = TempDir::new();
        // v1.2 validates the configured shell even for requests with no tool calls.
        let private = dir.0.join(".local/libexec/anix");
        fs::create_dir_all(&private).unwrap();
        let msh = private.join("msh");
        fs::write(&msh, "#!/bin/sh\nexec /bin/sh \"$@\"\n").unwrap();
        fs::set_permissions(msh, Permissions::from_mode(0o700)).unwrap();
        let config = dir.0.join("config/anix");
        let hooks = config.join("hooks/pre-context.d");
        fs::create_dir_all(&hooks).unwrap();
        // Mask built-ins using §8.3 so the first system message is just the script body.
        for name in ["00-posix-runtime", "20-skills", "30-tips"] {
            fs::write(hooks.join(name), "").unwrap();
        }
        fs::write(
            config.join("config.toml"),
            r#"
[defaults]
model = "fixture-model"
provider = "fixture"

[providers.fixture]
bin = "record-provider"
mode = "oneshot"
timeout_sec = 15
"#,
        )
        .unwrap();
        let provider = dir.0.join("record-provider");
        fs::write(
            &provider,
            r#"#!/bin/sh
set -eu
/bin/cat > "$HOME/request.jsonl"
printf '%s' "$ANIX_MODEL" > "$HOME/model"
printf '%s\n' '{"type":"text_delta","text":"ok"}' '{"type":"done","finish_reason":"stop"}'
"#,
        )
        .unwrap();
        fs::set_permissions(&provider, Permissions::from_mode(0o700)).unwrap();
        let capture = dir.0.join("request.jsonl");
        Self { dir, capture }
    }

    pub fn command_for(&self, program: &str) -> Command {
        let root = &self.dir.0;
        let mut cmd = Command::new(program);
        cmd.env_clear()
            .env("HOME", root)
            .env("PATH", root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_RUNTIME_DIR", root.join("run"))
            .current_dir(root)
            .stdin(Stdio::null());
        cmd
    }

    pub fn anix(&self) -> Command {
        self.command_for(env!("CARGO_BIN_EXE_anix"))
    }

    pub fn run(&self, cmd: &mut Command) -> std::process::Output {
        let root = &self.dir.0;
        let stdout = root.join("stdout");
        let stderr = root.join("stderr");
        let mut child = cmd
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("command timed out: {cmd:?}");
            }
            thread::sleep(Duration::from_millis(10));
        };
        std::process::Output {
            status,
            stdout: fs::read(stdout).unwrap(),
            stderr: fs::read(stderr).unwrap(),
        }
    }
}
