// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod draw;
mod proc;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::SysMon>(())
}
