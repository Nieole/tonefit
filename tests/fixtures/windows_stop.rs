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
            let mut command = Command::new("rustc");
            command
                .args(["--edition=2024", "--crate-name", "tonefit_stop_console"])
                .arg(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/windows_stop_console.rs"),
                )
                .arg("-o")
                .arg(dir.path().join("console.exe"));
            // Git Bash puts its own GNU link ahead of MSVC on PATH. Pin the
            // native linker exported by the CI native-toolchain step instead of
            // letting rustc find `link.exe` by name.
            if cfg!(windows) {
                let linker = std::env::var_os("CC_LD")
                    .or_else(|| {
                        std::env::var_os("VCToolsInstallDir").map(|root| {
                            PathBuf::from(root).join("bin/Hostx64/x64/link.exe").into_os_string()
                        })
                    })
                    .expect("native MSVC linker: initialize VCToolsInstallDir or CC_LD");
                command.arg("-C").arg(format!("linker={}", linker.to_string_lossy()));
            }
            let result = command
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
