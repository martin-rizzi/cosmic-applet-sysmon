// SPDX-License-Identifier: GPL-3.0-only

//! `GpuDetail.qml`: por placa, nombre, reloj/memoria/temperatura/uso, barra e historial;
//! después el top de procesos por VRAM.

use cosmic::iced::Length;
use cosmic::widget::{self, Column};
use cosmic::Element;

use super::{bar_row, block_title, centered_caption, history_view, process_rows, stat_row};
use crate::app::{Message, SysMon};
use crate::draw::{CRITICAL, NORMAL};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let unavailable = || "no disponible".to_string();
    let devices = m.gpu.devices();

    let mut col = Column::new().spacing(6);
    if devices.is_empty() {
        col = col.push(centered_caption("No se detectó ninguna GPU con datos de uso".to_string()));
    }
    for d in devices {
        let clock = format::clock(d.clock_mhz);
        let memory = if d.memory_total_mib > 0.0 {
            format!("{} / {}", format::memory_mib(d.memory_used_mib), format::memory_mib(d.memory_total_mib))
        } else {
            unavailable()
        };
        let history = m.gpu.history(&d.id).map(|h| h.points()).unwrap_or_default();
        col = col
            .push(widget::container(widget::text::heading(d.name.clone())).center_x(Length::Fill))
            .push(stat_row("Reloj:", if clock.is_empty() { unavailable() } else { clock }))
            .push(stat_row("Memoria:", memory))
            .push(stat_row(
                "Temperatura:",
                if d.temperature > 0.0 { format!("{:.0} °C", d.temperature) } else { unavailable() },
            ))
            .push(stat_row("Uso:", format::percent(d.usage)))
            .push(bar_row(
                d.usage / 100.0,
                if d.usage > 85.0 { CRITICAL } else { NORMAL },
                &border,
                format!("{:.0}%", d.usage),
            ))
            .push(history_view(app, &history, 80.0));
    }
    col.push(block_title("Procesos de GPU"))
        .push(process_rows(m.procs.vram.iter().map(|p| (p.name.clone(), format::memory_mib(p.mib)))))
        .into()
}
