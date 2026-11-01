// Reduced unrar-ng-sys 0.7.7 Linux binding: build.rs uses cpp_link_stdlib(None).
// Keep the native link attribute: removing it hides the modifier-override failure.
#[link(name = "stdc++")]
unsafe extern "C" {
    fn tonefit_cpp_probe() -> i32;
}
pub fn probe() -> i32 {
    unsafe { tonefit_cpp_probe() }
}
