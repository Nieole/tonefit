//! Standalone, test-only console owner, compiled by stop.rs with rustc (not a
//! Cargo binary or an ignored libtest). Mirrors ctrlc 3.5.2's FreeConsole /
//! AllocConsole / GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) precedent.
//!
//! Every archive is valid and exclusively oplocked *before* tonefit starts.
//! Metadata-only discovery does not break this oplock; source::open_zip's
//! File::open does, after the real Surveying callback. Breaking the first lock
//! is the readiness mark. Subsequent locks provide bounded scheduling recourse
//! (39 x 5ms, as on Unix), not an assumed startup sleep. If stop wiring is lost,
//! all gates open and the normal report makes the parent assertion fail.
use std::ffi::c_void;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

type Handle = *mut c_void;
#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: Handle,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn FreeConsole() -> i32;
    fn AllocConsole() -> i32;
    fn GetStdHandle(which: u32) -> Handle;
    fn SetStdHandle(which: u32, handle: Handle) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn GenerateConsoleCtrlEvent(event: u32, group: u32) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16)
    -> Handle;
    fn DeviceIoControl(
        file: Handle,
        code: u32,
        input: *const c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn CancelIoEx(file: Handle, overlapped: *const Overlapped) -> i32;
    fn GetOverlappedResult(
        file: Handle,
        overlapped: *const Overlapped,
        bytes: *mut u32,
        wait: i32,
    ) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
}

fn checked(ok: i32, what: &str) {
    assert_ne!(ok, 0, "{what}: {}", std::io::Error::last_os_error());
}

// A handler, NOT SetConsoleCtrlHandler(NULL, TRUE): the latter ignore flag is
// inherited by tonefit and would suppress the very Ctrl-C being tested.
unsafe extern "system" fn keep_owner_alive(event: u32) -> i32 {
    i32::from(event == 0)
}

fn press() {
    // SAFETY: this process owns a private console shared only with its CLI child.
    checked(unsafe { GenerateConsoleCtrlEvent(0, 0) }, "CTRL_C_EVENT");
}

struct Gate {
    file: Handle,
    pending: Box<Overlapped>,
}
impl Gate {
    fn new(path: &Path) -> Self {
        let wide: Vec<_> = path
            .to_str()
            .expect("Unicode fixture path")
            .encode_utf16()
            .chain([0])
            .collect();
        // SAFETY: NUL-terminated path, no security/template pointers; handles
        // remain owned here. OVERLAPPED is boxed until the request completes.
        unsafe {
            let file = CreateFileW(
                wide.as_ptr(),
                0x8000_0000,
                7,
                null(),
                3,
                0x4000_0000,
                null_mut(),
            );
            assert_ne!(
                file as isize,
                -1,
                "oplock open: {}",
                std::io::Error::last_os_error()
            );
            let event = CreateEventW(null(), 1, 0, null());
            assert!(!event.is_null(), "oplock event");
            let mut gate = Self {
                file,
                pending: Box::new(Overlapped {
                    internal: 0,
                    internal_high: 0,
                    offset: 0,
                    offset_high: 0,
                    event,
                }),
            };
            let mut bytes = 0;
            // FSCTL_REQUEST_OPLOCK_LEVEL_1 = CTL_CODE(FILE_DEVICE_FILE_SYSTEM,0,METHOD_BUFFERED,FILE_ANY_ACCESS).
            let result = DeviceIoControl(
                file,
                0x0009_0000,
                null(),
                0,
                null_mut(),
                0,
                &mut bytes,
                &mut *gate.pending,
            );
            assert_eq!(result, 0, "oplock must pend");
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(997),
                "NTFS oplock unavailable"
            );
            gate
        }
    }
    fn broken(&self) -> bool {
        // SAFETY: event remains valid until Drop; a zero timeout never blocks.
        let result = unsafe { WaitForSingleObject(self.pending.event, 0) };
        assert!(result == 0 || result == 258, "oplock wait failed");
        result == 0
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        // SAFETY: drain/cancel before freeing OVERLAPPED/event. Closing the file
        // acknowledges an exclusive oplock break and releases the blocked open.
        unsafe {
            CancelIoEx(self.file, &*self.pending);
            let mut bytes = 0;
            GetOverlappedResult(self.file, &*self.pending, &mut bytes, 1);
            CloseHandle(self.file);
            CloseHandle(self.pending.event);
        }
    }
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn await_gate(child: &mut Child, gate: &Gate) -> bool {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if child.try_wait().unwrap().is_some() {
            return false;
        }
        if gate.broken() {
            return true;
        }
        assert!(
            Instant::now() < deadline,
            "archive open never broke the oplock"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let control = PathBuf::from(&args[0]);
    let survey = args[1] == "survey";
    // SAFETY: only this standalone owner detaches. FreeConsole may fail when
    // started without a console; AllocConsole must nevertheless succeed.
    unsafe {
        // AllocConsole may replace the standard handles. Keep the parent's
        // report pipe and diagnostic stream, not the new console screen buffer.
        let stdout = GetStdHandle(-11_i32 as u32);
        let stderr = GetStdHandle(-12_i32 as u32);
        FreeConsole();
        checked(AllocConsole(), "AllocConsole");
        checked(SetStdHandle(-11_i32 as u32, stdout), "restore stdout pipe");
        checked(SetStdHandle(-12_i32 as u32, stderr), "restore stderr");
        checked(
            SetConsoleCtrlHandler(Some(keep_owner_alive), 1),
            "owner handler",
        );
    }
    // CLI argument prefix: binary --out PATH --profile NAME --envelope INPUTS...
    let mut gates: Vec<_> = if survey {
        args[8..]
            .iter()
            .map(|path| Gate::new(Path::new(path)))
            .collect()
    } else {
        Vec::new()
    };
    let report = control.join("report");
    let stdout = std::fs::File::create(&report).unwrap();
    let mut child = Running(
        Command::new(&args[2])
            .args(&args[3..])
            // No CREATE_NEW_PROCESS_GROUP: Ctrl-C broadcasts to the shared console.
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    if survey {
        assert!(
            await_gate(&mut child.0, &gates[0]),
            "CLI exited before surveying"
        );
        press();
        for gate in gates.drain(..) {
            // The first break proves readiness; each later break proves another
            // actual archive open. Exhaustion is NOT success: parent checks report.
            if !await_gate(&mut child.0, &gate) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            drop(gate);
        }
    }
    let deadline = Instant::now() + Duration::from_secs(150);
    let mut next = 1;
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        if control.join(format!("press-{next}")).exists() {
            press();
            next += 1;
        }
        assert!(Instant::now() < deadline, "CLI exceeded 150 seconds");
        std::thread::sleep(Duration::from_millis(1));
    };
    std::io::stdout()
        .write_all(&std::fs::read(report).unwrap())
        .unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
