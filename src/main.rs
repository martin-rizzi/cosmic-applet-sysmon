// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod config;
mod draw;
mod exec;
mod format;
mod metrics;
mod ui;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::SysMon>(())
}
