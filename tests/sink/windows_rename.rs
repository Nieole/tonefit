//! Private policy seam approved for deterministic bounds and real Windows handle controls.
use super::*;
use std::os::windows::fs::OpenOptionsExt;
use std::time::Duration;

fn deny_delete(path: &Path) -> File {
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3) // FILE_SHARE_READ | FILE_SHARE_WRITE, deliberately not DELETE.
        .open(path)
        .unwrap()
}

#[test]
fn directory_publication_recovers_after_an_observed_real_handle_denial() {
    for isolated in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join(if isolated {
            "_isolated/volume"
        } else {
            "volume"
        });
        let mut sink = Sink::create(&target, Container::Directory, Lodgers::default()).unwrap();
        sink.write_page(Path::new("001.png"), b"unchanged page bytes")
            .unwrap();
        sink.write_extra(Path::new("ComicInfo.xml"), b"unchanged extra bytes")
            .unwrap();
        let partial = partial_path(&target);
        let mut handle = Some(deny_delete(&partial.join("001.png")));
        let mut denials = 0;
        // Release only after the actual OS rename has been denied, not after a guessed sleep.
        retry_output_directory_rename(
            || {
                let result = std::fs::rename(&partial, &target);
                if handle.is_some() {
                    assert_eq!(result.as_ref().unwrap_err().raw_os_error(), Some(5));
                    denials += 1;
                    drop(handle.take());
                }
                result
            },
            std::thread::sleep,
        )
        .unwrap();
        assert_eq!(denials, 1);
        assert!(!partial.exists());
        assert_eq!(
            std::fs::read(target.join("001.png")).unwrap(),
            b"unchanged page bytes"
        );
        assert_eq!(
            std::fs::read(target.join("ComicInfo.xml")).unwrap(),
            b"unchanged extra bytes"
        );
        drop(sink);
    }
}

#[test]
fn persistent_denial_has_a_finite_scheduled_wait_budget() {
    let mut attempts = 0;
    let mut waits = Vec::new();
    let error = retry_output_directory_rename(
        || {
            attempts += 1;
            Err(std::io::Error::from_raw_os_error(5))
        },
        |delay| waits.push(delay.as_millis()),
    )
    .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(5));
    assert_eq!(waits, [1, 2, 4, 8, 16, 32, 50, 50, 50, 37]);
    assert_eq!(attempts, 11);
}

#[test]
fn non_target_errors_are_returned_without_waiting_or_retrying() {
    // Including sharing violation: only ACCESS_DENIED has baseline evidence.
    for code in [2, 3, 17, 32, 80] {
        let mut attempts = 0;
        let error = retry_output_directory_rename(
            || {
                attempts += 1;
                Err(std::io::Error::from_raw_os_error(code))
            },
            |_| panic!("non-target error must not wait"),
        )
        .unwrap_err();
        assert_eq!(attempts, 1);
        assert_eq!(error.raw_os_error(), Some(code));
    }
}

#[test]
fn persistent_child_handle_fails_sink_finish_with_paths_and_native_cause() {
    for isolated in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join(if isolated {
            "_isolated/volume"
        } else {
            "volume"
        });
        let mut sink = Sink::create(&target, Container::Directory, Lodgers::default()).unwrap();
        sink.write_page(Path::new("001.png"), b"page bytes")
            .unwrap();
        let partial = partial_path(&target);
        let handle = deny_delete(&partial.join("001.png"));
        let started = std::time::Instant::now();
        let error = sink.finish().unwrap_err();
        assert!(started.elapsed() >= Duration::from_millis(250));
        let message = format!("{error:#}");
        assert!(
            message.contains(&partial.display().to_string()),
            "{message}"
        );
        assert!(message.contains(&target.display().to_string()), "{message}");
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(5)
        );
        assert!(!target.exists());
        // The external lock can also prevent best-effort Drop cleanup, as before.
        drop(handle);
    }
}

#[test]
fn clean_and_isolated_sink_finish_preserve_page_and_extra_bytes() {
    for isolated in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join(if isolated {
            "_isolated/volume"
        } else {
            "volume"
        });
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("stale.png"), b"stale").unwrap();
        let mut sink = Sink::create(&target, Container::Directory, Lodgers::default()).unwrap();
        sink.write_page(Path::new("001.png"), b"page bytes")
            .unwrap();
        sink.write_extra(Path::new("ComicInfo.xml"), b"extra bytes")
            .unwrap();
        assert_eq!(std::fs::read(target.join("stale.png")).unwrap(), b"stale");
        sink.finish().unwrap();
        assert!(!partial_path(&target).exists());
        assert!(!target.join("stale.png").exists());
        assert_eq!(
            std::fs::read(target.join("001.png")).unwrap(),
            b"page bytes"
        );
        assert_eq!(
            std::fs::read(target.join("ComicInfo.xml")).unwrap(),
            b"extra bytes"
        );
    }
}

#[test]
fn discard_does_not_publish_or_replace_existing_output() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("volume");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("001.png"), b"previous bytes").unwrap();
    let mut sink = Sink::create(&target, Container::Directory, Lodgers::default()).unwrap();
    sink.write_page(Path::new("001.png"), b"discarded bytes")
        .unwrap();
    drop(sink);
    assert!(!partial_path(&target).exists());
    assert_eq!(
        std::fs::read(target.join("001.png")).unwrap(),
        b"previous bytes"
    );
}
