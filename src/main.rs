#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod adapters;
mod app;
mod gallery;
mod pages;
mod panels;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod workspace;

// Stable local names for the independent business library, not wrapper modules.
use tydaro_core::{market, model, stock_import};

fn main() {
    #[cfg(feature = "test-support")]
    if let Some(path) = std::env::args()
        .skip_while(|arg| arg != "--architecture-smoke-test")
        .nth(1)
    {
        if let Err(error) = test_support::run_architecture(&path) {
            eprintln!("Architecture interaction test failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(feature = "test-support")]
    if let Some(path) = std::env::args()
        .skip_while(|arg| arg != "--gallery-smoke-test")
        .nth(1)
    {
        if let Err(error) = test_support::gallery::run(&path) {
            eprintln!("Gallery smoke test failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--gallery") {
        app::run_gallery();
        return;
    }
    #[cfg(feature = "test-support")]
    if let Some(path) = std::env::args()
        .skip_while(|arg| arg != "--desktop-smoke-test")
        .nth(1)
    {
        if let Err(error) = test_support::run_desktop(&path) {
            eprintln!("Desktop smoke test failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(feature = "test-support")]
    if let Some(path) = std::env::args()
        .skip_while(|arg| arg != "--smoke-test")
        .nth(1)
    {
        if let Err(error) = test_support::run(&path) {
            eprintln!("Smoke test failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    app::run();
}
