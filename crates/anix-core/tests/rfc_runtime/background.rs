use super::Fixture;
use std::fs::{self, File};
use std::io;
use std::process::{Child, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct Agent(pub(super) Child);

impl Agent {
    pub(super) fn start(fx: &Fixture, args: &[&str]) -> Self {
        fs::write(fx.dir.0.join("case.anix"), "frozen-test-text").unwrap();
        let c = fx
            .anix()
            .args(args)
            .stdout(Stdio::null())
            .stderr(File::create(fx.dir.0.join("anix-stderr")).unwrap())
            .spawn()
            .unwrap();
        Self(c)
    }
    pub(super) fn signal(&self, signo: i32) {
        // SAFETY: Only target the positive PID returned by our own Child. It has
        // not been reaped so the kernel cannot reuse it.
        let rc = unsafe { libc::kill(self.0.id() as libc::pid_t, signo) };
        assert_eq!(rc, 0, "kill: {}", io::Error::last_os_error());
    }
    pub(super) fn finish(&mut self, expected: i32) {
        until("agent termination", || self.0.try_wait().unwrap().is_some());
        assert_eq!(self.0.wait().unwrap().code(), Some(expected));
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

pub(super) fn until(what: &str, mut ready: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(12);
    while !ready() {
        assert!(Instant::now() < end, "deadline waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}
