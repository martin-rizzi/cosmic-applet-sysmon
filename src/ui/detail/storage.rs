// SPDX-License-Identifier: GPL-3.0-only

//! `StorageDetail.qml`: gráfico de lectura/escritura con escala fija, leyenda y
//! dispositivos con su uso.

use cosmic::iced::Length;
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use super::{bar_row, block_title, centered_caption, dual_view, legend};
use crate::app::{Message, SysMon};
use crate::draw::{threshold_color, DualGraph, CRITICAL, NORMAL};
use crate::format;
use crate::metrics::disk::GRAPH_SCALE;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let io = &m.storage.io;

    let mut col = Column::new()
        .spacing(6)
        .push(dual_view(
            app,
            &DualGraph {
                top: &m.disk_read_history.points(),
                bottom: &m.disk_write_history.points(),
                top_color: NORMAL,
                bottom_color: CRITICAL,
                gap: 3.0,
                top_padding: 0.0,
                grid: true,
            },
            80.0,
        ))
        .push(legend(vec![
            (NORMAL, format!("Lectura {}", format::rate(io.read_rate))),
            (CRITICAL, format!("Escritura {}", format::rate(io.write_rate))),
        ]))
        // El QML rotula el eje a la derecha; con escala fija alcanza con decirla una vez.
        .push(widget::text::caption(format!("Escala: {} por mitad", format::rate(GRAPH_SCALE))))
        .push(block_title("Dispositivos"));

    for dev in &m.storage.devices {
        let size = match &dev.usage {
            Some(u) => format!(
                "{} / {}",
                format::storage_bytes(u.used as f64),
                format::storage_bytes(u.size as f64)
            ),
            None => format::storage_bytes(dev.size as f64),
        };
        col = col
            .push(
                Row::new()
                    .width(Length::Fill)
                    .push(widget::text::body(dev.name.as_str()).font(cosmic::font::bold()).width(Length::Fill))
                    .push(widget::text::caption(size)),
            )
            .push(widget::text::caption(format!("{} • {}", dev.path, dev.detail)));
        if let Some(u) = &dev.usage {
            let fraction = u.percent as f32 / 100.0;
            col = col.push(bar_row(fraction, threshold_color(fraction), &border, format!("{}%", u.percent)));
        }
    }
    if m.storage.devices.is_empty() {
        col = col.push(centered_caption("No se encontraron dispositivos de almacenamiento".to_string()));
    }
    col.into()
}
