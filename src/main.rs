#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod board_bar;
mod boards;
mod chart;
mod contents;
mod desktop;
#[cfg(feature = "test-support")]
mod desktop_smoke;
mod desktop_state;
mod desktop_ui;
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
mod watch_page;
mod workspace;
fn main() {
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
