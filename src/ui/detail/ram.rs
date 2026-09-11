// SPDX-License-Identifier: GPL-3.0-only

//! `RamDetail.qml`: total/usada/libre/en caché, historial, barra, top de procesos y swap.

use cosmic::widget::Column;
use cosmic::Element;

use super::{bar_row, block_title, history_view, process_rows, stat_row};
use crate::app::{Message, SysMon};
use crate::draw::{CRITICAL, NORMAL, WARNING};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let mem = &m.mem;
    let border = app.text_color_hex();
    let fraction = mem.fraction();
    let swap_fraction = if mem.swap_total > 0.0 {
        (mem.swap_used / mem.swap_total).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    // "..." mientras no hay lectura, como el QML.
    let gb = |mib: f64| if mib > 0.0 { format::gb2(mib) } else { "...".to_string() };

    Column::new()
        .spacing(6)
        .push(stat_row("Total:", gb(mem.total)))
        .push(stat_row("Usada:", gb(mem.used)))
        .push(stat_row("Libre:", gb(mem.free)))
        .push(stat_row("En caché:", gb(mem.cached)))
        .push(history_view(app, &m.ram_history.points(), 58.0))
        .push(bar_row(
            fraction,
            if fraction > 0.85 { CRITICAL } else { NORMAL },
            &border,
            format!("{:.0}%", fraction * 100.0),
        ))
        .push(block_title("Procesos"))
        .push(process_rows(m.procs.mem.iter().map(|p| {
            (p.name.clone(), format!("{}  {:.1}%", format::memory_kib(p.kib), p.percent))
        })))
        .push(block_title("Swap"))
        .push(stat_row(
            "Total:",
            if mem.swap_total > 0.0 { format::gb2(mem.swap_total) } else { "Ninguna".to_string() },
        ))
        .push(stat_row(
            "Usada:",
            if mem.swap_used > 0.0 { format::gb2(mem.swap_used) } else { "0 GB".to_string() },
        ))
        .push(bar_row(swap_fraction, WARNING, &border, format!("{:.0}%", swap_fraction * 100.0)))
        .into()
}
