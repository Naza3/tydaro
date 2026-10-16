#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod app_runtime;
mod board_bar;
mod boards;
mod chart;
mod component_gallery;
mod contents;
mod desktop;
mod desktop_repository;
#[cfg(feature = "test-support")]
mod desktop_smoke;
mod desktop_state;
mod desktop_ui;
mod gallery_controls;
mod gallery_display;
mod gallery_preferences;
mod gallery_selection;
#[cfg(feature = "test-support")]
mod gallery_smoke;
mod history;
mod journal_page;
mod layout_persistence;
mod market;
mod market_data;
mod model;
mod panel_snapshot;
mod research_page;
#[cfg(feature = "test-support")]
mod smoke;
mod stock_import;
mod storage;
mod transfer;
mod ui_foundation;
mod watch_page;
mod workspace;
mod workspace_snapshot;
fn main() {
    #[cfg(feature = "test-support")]
    if let Some(path) = std::env::args()
        .skip_while(|arg| arg != "--architecture-smoke-test")
        .nth(1)
    {
        if let Err(error) = smoke::run_architecture(&path) {
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
        if let Err(error) = gallery_smoke::run(&path) {
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
        if let Err(error) = smoke::run_desktop(&path) {
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
        if let Err(error) = smoke::run(&path) {
            eprintln!("Smoke test failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    app::run();
}
