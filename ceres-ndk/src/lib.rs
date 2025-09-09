mod emulator;
mod ffi;
mod video;

use android_activity::AndroidApp;
pub use ffi::*;

#[expect(clippy::no_mangle_with_rust_abi)]
#[unsafe(no_mangle)]
fn android_main(_app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("CeresEmulator"),
    );

    log::info!("Ceres Android emulator started");
}
