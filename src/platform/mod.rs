//! プラットフォーム抽象化モジュール

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::{ensure_capslock_off, run_hook, stop_hook};

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::{ensure_capslock_off, run_hook, stop_hook};

#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("capsnav は現在 Windows および macOS のみをサポートしています。");
