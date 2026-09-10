// SPDX-License-Identifier: GPL-3.0-only

//! Página de ajustes del popup, detrás del engranaje del pie. Equivale a
//! `ConfigGeneral.qml` del plasmoid: seis switches de "Mostrar", el intervalo de
//! actualización y el orden de las secciones en el panel.

use cosmic::iced::Alignment;
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::{normalize_section_order, Section};

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let cfg = app.config();

    let mut mostrar = Column::new()
        .spacing(4)
        .push(widget::text::heading("Mostrar"));
    for s in Section::all() {
        mostrar = mostrar.push(widget::settings::item(
            s.label(),
            widget::toggler(cfg.shown(s)).on_toggle(move |v| Message::ToggleSection(s, v)),
        ));
    }

    let intervalo = widget::settings::item(
        "Intervalo de actualización",
        Row::new()
            .spacing(8)
            .align_y(Alignment::Center)
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic")).on_press(
                    Message::SetInterval(cfg.update_interval.saturating_sub(500).max(500)),
                ),
            )
            .push(widget::text::body(format!("{} ms", cfg.update_interval)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic")).on_press(
                    Message::SetInterval((cfg.update_interval + 500).min(10_000)),
                ),
            ),
    );

    let mut orden = Column::new()
        .spacing(4)
        .push(widget::text::heading("Orden en el panel"));
    for s in normalize_section_order(&cfg.section_order) {
        orden = orden.push(
            Row::new()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(s.label()).width(cosmic::iced::Length::Fill))
                .push(
                    widget::button::icon(widget::icon::from_name("go-up-symbolic"))
                        .on_press(Message::MoveSection(s, -1)),
                )
                .push(
                    widget::button::icon(widget::icon::from_name("go-down-symbolic"))
                        .on_press(Message::MoveSection(s, 1)),
                ),
        );
    }

    Column::new()
        .spacing(12)
        .push(
            Row::new()
                .push(widget::text::title4("Configuración"))
                .push(widget::space::horizontal())
                .push(
                    widget::button::icon(widget::icon::from_name("window-close-symbolic"))
                        .on_press(Message::CloseSettings),
                ),
        )
        .push(mostrar)
        .push(intervalo)
        .push(orden)
        .into()
}
