#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod board_bar;
mod boards;
mod chart;
mod component_gallery;
mod contents;
mod desktop;
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
mod market;
mod model;
mod research_page;
#[cfg(feature = "test-support")]
mod smoke;
mod stock_import;
mod storage;
mod transfer;
mod ui_foundation;
mod watch_page;
mod workspace;
fn main() {
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
        component_gallery::run();
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
    workspace::run();
}
