// SPDX-License-Identifier: GPL-3.0-only

//! `TempDetail.qml`: un renglón por sensor con barra sobre 110 °C y color por umbral.

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use super::{centered_caption, svg};
use crate::app::{Message, SysMon, TEMP_ICON};
use crate::draw::{hex_color, horizontal_bar, NORMAL};
use crate::metrics::temp::severity_color;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let mut col = Column::new().spacing(4);
    for r in &m.temps {
        let color = severity_color(r.celsius);
        let bar = horizontal_bar((r.celsius / 110.0).min(1.0), 60.0, 12.0, color.unwrap_or(NORMAL), &border);
        let value = widget::text::body(format!("{:.1} °C", r.celsius)).font(cosmic::font::bold());
        let value: Element<Message> = match color {
            Some(hex) => value.class(cosmic::theme::Text::Color(hex_color(hex))).into(),
            None => value.into(),
        };
        col = col.push(
            Row::new()
                .spacing(6)
                .align_y(Alignment::Center)
                .push(app.icon(TEMP_ICON, 12))
                .push(widget::text::caption(r.label.as_str()).width(Length::Fill))
                .push(svg(bar, 60.0, 12.0))
                .push(widget::container(value).width(Length::Fixed(64.0))),
        );
    }
    // El QML pide instalar lm-sensors; acá se lee hwmon directo, así que lo único que puede
    // faltar son sensores expuestos por el kernel.
    if m.temps.is_empty() {
        col = col.push(centered_caption("No hay sensores de temperatura en /sys/class/hwmon".to_string()));
    }
    col.into()
}
