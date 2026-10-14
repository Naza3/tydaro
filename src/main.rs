#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod board_bar;
mod boards;
mod chart;
mod contents;
mod history;
mod market;
mod model;
#[cfg(feature = "test-support")]
mod smoke;
mod storage;
mod transfer;
mod workspace;
fn main() {
    workspace::run();
}
