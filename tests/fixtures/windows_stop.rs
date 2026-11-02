//! Host side of the isolated console protocol. No console API runs in libtest.
use std::ffi::OsString;
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
            if let Some(linker) = pinned_linker(|name| std::env::var_os(name)) {
                command
                    .arg("-C")
                    .arg(format!("linker={}", linker.to_string_lossy()));
            }
            let result = command.output().expect("compile test-only console owner");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            dir
        })
        .path()
}

/// The linker to pin for the helper build, if the environment names one.
///
/// Git Bash puts its own GNU `link` ahead of MSVC on PATH, so CI exports the
/// native linker (`CC_LD`, or the `VCToolsInstallDir` of a developer prompt).
/// With neither set, rustc keeps its own MSVC discovery, which finds an
/// installed toolchain from a plain PowerShell.
fn pinned_linker(var: impl Fn(&str) -> Option<OsString>) -> Option<OsString> {
    var("CC_LD").or_else(|| {
        var("VCToolsInstallDir").map(|root| {
            PathBuf::from(root)
                .join("bin/Hostx64/x64/link.exe")
                .into_os_string()
        })
    })
}

#[test]
fn the_helper_build_pins_only_a_linker_the_environment_names() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    };
    assert_eq!(pinned_linker(env(&[])), None);
    assert_eq!(
        pinned_linker(env(&[("VCToolsInstallDir", r"C:\VC\")])),
        Some(
            PathBuf::from(r"C:\VC\")
                .join("bin/Hostx64/x64/link.exe")
                .into_os_string()
        )
    );
    assert_eq!(
        pinned_linker(env(&[
            ("CC_LD", r"C:\VC\link.exe"),
            ("VCToolsInstallDir", r"C:\Other\"),
        ])),
        Some(OsString::from(r"C:\VC\link.exe"))
    );
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
