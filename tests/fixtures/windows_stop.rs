//! Host side of the isolated console protocol. No console API runs in libtest.
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::fixtures::{BASELINE_DEVICE, Workspace};

fn helper() -> &'static Path {
    static BUILT: OnceLock<tempfile::TempDir> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let dir = tempfile::tempdir().expect("helper build directory");
            let result = Command::new("rustc")
                .args(["--edition=2024", "--crate-name", "tonefit_stop_console"])
                .arg(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/windows_stop_console.rs"),
                )
                .arg("-o")
                .arg(dir.path().join("console.exe"))
                .output()
                .expect("compile test-only console owner");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            dir
        })
        .path()
}

pub struct Child {
    process: Option<std::process::Child>,
    control: tempfile::TempDir,
    presses: Mutex<usize>,
    started: Instant,
}

pub fn spawn(space: &Workspace, inputs: &[PathBuf], survey: bool) -> Child {
    let control = tempfile::tempdir().expect("console control directory");
    let process = Command::new(helper().join("console.exe"))
        .arg(control.path())
        .arg(if survey { "survey" } else { "pages" })
        .arg(env!("CARGO_BIN_EXE_tonefit"))
        .arg("--out")
        .arg(space.out())
        .args(["--profile", BASELINE_DEVICE, "--envelope"])
        .args(inputs)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("launch console owner");
    Child {
        process: Some(process),
        control,
        presses: Mutex::new(0),
        started: Instant::now(),
    }
}

impl Child {
    pub fn ctrl_c(&self) {
        let mut presses = self.presses.lock().unwrap();
        *presses += 1;
        std::fs::write(self.control.path().join(format!("press-{presses}")), []).unwrap();
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        assert!(
            self.started.elapsed() < Duration::from_secs(180),
            "console owner exceeded 180 seconds"
        );
        self.process.as_mut().unwrap().try_wait()
    }

    pub fn wait_with_output(mut self) -> std::io::Result<Output> {
        let deadline = Instant::now() + Duration::from_secs(180);
        while self.try_wait()?.is_none() {
            assert!(
                Instant::now() < deadline,
                "console owner exceeded 180 seconds"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        self.process.take().unwrap().wait_with_output()
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            // Kill descendants too if a host assertion failed while tonefit was blocked.
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &process.id().to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}
