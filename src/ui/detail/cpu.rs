// SPDX-License-Identifier: GPL-3.0-only

//! `CpuDetail.qml`: modelo y reloj, total/usuario/sistema, barra, historial, núcleos, top
//! de procesos y tiempo encendido.

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use super::{bar_row, block_title, centered_caption, history_view, process_rows, stat_row, svg};
use crate::app::{Message, SysMon};
use crate::draw::{CORE_COLORS, CRITICAL, NORMAL};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let model = if m.cpu_info.model.is_empty() { "CPU" } else { m.cpu_info.model.as_str() };
    let clock = format::clock(m.cpu_info.mhz);
    let header = if clock.is_empty() { model.to_string() } else { format!("{model} @ {clock}") };
    let total = m.cpu.total;

    Column::new()
        .spacing(6)
        .push(centered_caption(header))
        .push(stat_row("Total:", format!("{total:.0}%")))
        .push(stat_row("Usuario:", format!("{:.0}%", m.cpu.user)))
        .push(stat_row("Sistema:", format!("{:.0}%", m.cpu.system)))
        .push(bar_row(total / 100.0, if total > 80.0 { CRITICAL } else { NORMAL }, &border, format!("{total:.0}%")))
        .push(history_view(app, &m.cpu_history.points(), 58.0))
        .push(cores(app, &border))
        .push(block_title("Procesos"))
        .push(process_rows(m.procs.cpu.iter().map(|p| (p.name.clone(), format!("{:.1}%", p.percent)))))
        .push(block_title("Tiempo encendido"))
        .push(centered_caption(format::uptime(m.uptime_secs)))
        .into()
}

/// Un renglón por núcleo con el color que tiene en el panel. El plasmoid los muestra en un
/// popup flotante al pasar el mouse por el gráfico (`CpuCoreInfo.qml`); un applet de
/// COSMIC no tiene esos tooltips, así que van en línea.
fn cores<'a>(app: &'a SysMon, border: &str) -> Element<'a, Message> {
    const BAR_W: f32 = 190.0;
    let mut col = Column::new().spacing(4);
    for (i, usage) in app.metrics().cpu.cores.iter().enumerate() {
        let bar = crate::draw::horizontal_bar(usage / 100.0, BAR_W, 8.0, CORE_COLORS[i % CORE_COLORS.len()], border);
        col = col.push(
            Row::new()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::caption(format!("cpu{i}")).width(Length::Fixed(48.0)))
                .push(svg(bar, BAR_W, 8.0))
                .push(widget::text::caption(format!("{usage:.0}%"))),
        );
    }
    col.into()
}
