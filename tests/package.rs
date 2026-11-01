//! Deterministic delivery fixtures; ordinary gates explicitly test Cargo's production binary.
//! Final archive verification is performed separately by scripts/ci/package.py.
use std::io::Write;
use std::path::Path;
use std::process::Command;

#[test]
fn delivery_fixtures_and_explicit_binary_smoke() {
    let temporary = tempfile::tempdir().unwrap();
    let exported = std::env::var_os("TONEFIT_PACKAGE_FIXTURES");
    let root = exported
        .as_deref()
        .map(Path::new)
        .unwrap_or(temporary.path());
    std::fs::create_dir_all(root.join("images")).unwrap();
    let page = image::RgbImage::from_fn(32, 48, |x, y| {
        image::Rgb([(x * 7) as u8, (y * 5) as u8, ((x + y) * 3) as u8])
    });
    for extension in ["avif", "png", "jpg", "bmp", "tiff", "webp", "gif"] {
        page.save(root.join(format!("images/page.{extension}")))
            .unwrap();
    }
    let mut cbz = zip::ZipWriter::new(std::fs::File::create(root.join("images.cbz")).unwrap());
    for extension in ["avif", "png", "jpg", "bmp", "tiff", "webp", "gif"] {
        // Distinct stems prevent converted page names from colliding.
        cbz.start_file(
            format!("{extension}.{extension}"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        cbz.write_all(&std::fs::read(root.join(format!("images/page.{extension}"))).unwrap())
            .unwrap();
    }
    cbz.finish().unwrap();
    std::fs::copy("tests/fixtures/rar/solid.rar", root.join("comic.rar")).unwrap();
    // No implicit fallback from a requested external binary to CARGO_BIN_EXE.
    let binary = std::env::var_os("TONEFIT_TEST_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tonefit").into());
    let result = Command::new(&binary).arg("--version").output().unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().trim(),
        concat!("tonefit ", env!("CARGO_PKG_VERSION"))
    );
    for input in ["images.cbz", "comic.rar"] {
        let result = Command::new(&binary)
            .args(["--profile", "kobo-libra-2"])
            .arg(root.join(input))
            .arg("--out")
            .arg(root.join(format!("out-{input}")))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}: {}",
            input,
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
