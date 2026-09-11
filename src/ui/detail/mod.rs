// SPDX-License-Identifier: GPL-3.0-only

//! Contenido del popup: rutea por la sección elegida (`SysMon::selected`) y agrega el pie
//! con el monitor de sistema y los ajustes. Cada panel vive en su archivo; acá quedan los
//! widgets que comparten.

mod cpu;
mod gpu;
mod net;
mod ram;
mod storage;
mod temp;

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::Section;
use crate::draw::DualGraph;
use crate::metrics::history::window_label;

/// El plasmoid lanza `kstart plasma-systemmonitor`. COSMIC no trae monitor propio,
/// así que se usa el primero disponible en el PATH.
const CANDIDATES: [&str; 4] = [
    "plasma-systemmonitor",
    "gnome-system-monitor",
    "btop",
    "htop",
];

/// Primer monitor de sistema disponible en el PATH, o `None` si no hay ninguno.
pub fn system_monitor_command() -> Option<&'static str> {
    CANDIDATES.into_iter().find(|c| crate::exec::executable_in_path(c))
}

/// Ancho del contenido del popup: `gridUnit * 20` del plasmoid, menos márgenes.
pub const CONTENT_W: f32 = 300.0;

fn svg<'a>(svg: String, w: f32, h: f32) -> Element<'a, Message> {
    widget::svg(widget::svg::Handle::from_memory(svg.into_bytes()))
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .into()
}

/// `StatRow.qml`: rótulo a la izquierda, valor en negrita a la derecha.
fn stat_row<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    Row::new()
        .width(Length::Fill)
        .push(widget::text::body(label).width(Length::Fill))
        .push(widget::text::body(value).font(cosmic::font::bold()))
        .into()
}

/// Título de bloque centrado ("Procesos", "Swap", "Dispositivos").
fn block_title(text: &str) -> Element<'_, Message> {
    widget::container(widget::text::heading(text))
        .center_x(Length::Fill)
        .into()
}

fn centered_caption<'a>(text: String) -> Element<'a, Message> {
    widget::container(widget::text::caption(text))
        .center_x(Length::Fill)
        .into()
}

/// Barra horizontal con el valor escrito a la derecha.
fn bar_row<'a>(fraction: f32, fill: &str, border: &str, value: String) -> Element<'a, Message> {
    let w = CONTENT_W - 48.0;
    Row::new()
        .spacing(8)
        .align_y(Alignment::Center)
        .push(svg(crate::draw::horizontal_bar(fraction, w, 10.0, fill, border), w, 10.0))
        .push(widget::text::caption(value))
        .into()
}

/// Rótulos del eje de tiempo. El extremo izquierdo sale del intervalo configurado.
fn time_axis(app: &SysMon) -> Element<'_, Message> {
    Row::new()
        .width(Length::Fill)
        .push(widget::text::caption(window_label(app.config().update_interval)))
        .push(widget::space::horizontal())
        .push(widget::text::caption("ahora"))
        .into()
}

/// Gráfico de una serie con su eje de tiempo.
fn history_view<'a>(app: &'a SysMon, points: &[f32], h: f32) -> Element<'a, Message> {
    let graph = crate::draw::history_graph(points, CONTENT_W, h, crate::draw::NORMAL, &app.text_color_hex());
    Column::new()
        .spacing(2)
        .push(svg(graph, CONTENT_W, h))
        .push(time_axis(app))
        .into()
}

/// Gráfico de dos series con su eje de tiempo.
fn dual_view<'a>(app: &'a SysMon, graph: &DualGraph, h: f32) -> Element<'a, Message> {
    let drawn = crate::draw::dual_history_graph(graph, CONTENT_W, h, &app.text_color_hex());
    Column::new()
        .spacing(2)
        .push(svg(drawn, CONTENT_W, h))
        .push(time_axis(app))
        .into()
}

/// Muestra de color + texto: las leyendas de red y disco.
fn legend<'a>(items: Vec<(&str, String)>) -> Element<'a, Message> {
    let mut row = Row::new().spacing(6).align_y(Alignment::Center);
    for (color, text) in items {
        let swatch = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="3"><rect width="12" height="3" rx="1" fill="{color}"/></svg>"#
        );
        row = row.push(svg(swatch, 12.0, 3.0)).push(widget::text::caption(text));
    }
    row.into()
}

/// Renglones "nombre … valor" de los tops de procesos.
fn process_rows<'a>(rows: impl IntoIterator<Item = (String, String)>) -> Element<'a, Message> {
    let mut col = Column::new().spacing(2);
    for (name, value) in rows {
        col = col.push(
            Row::new()
                .width(Length::Fill)
                .push(widget::text::body(name).font(cosmic::font::bold()).width(Length::Fill))
                .push(widget::text::body(value)),
        );
    }
    col.into()
}

/// Contenido del popup: el panel de la sección elegida, o los ajustes si se abrió el
/// engranaje, más el pie.
pub fn view(app: &SysMon) -> Element<'_, Message> {
    if app.showing_settings() {
        return app
            .core()
            .applet
            .popup_container(crate::ui::settings::view(app))
            .into();
    }

    let section = app.selected();
    let body: Element<Message> = match section {
        Section::Cpu => cpu::view(app),
        Section::Ram => ram::view(app),
        Section::Network => net::view(app),
        Section::Storage => storage::view(app),
        Section::Temps => temp::view(app),
        Section::Gpu => gpu::view(app),
    };

    let mut footer = Row::new().spacing(8).align_y(Alignment::Center);
    if system_monitor_command().is_some() {
        footer = footer.push(
            widget::button::icon(widget::icon::from_name("utilities-system-monitor-symbolic"))
                .on_press(Message::OpenSystemMonitor),
        );
    }
    footer = footer.push(
        widget::button::icon(widget::icon::from_name("preferences-system-symbolic"))
            .on_press(Message::OpenSettings),
    );

    let content = Column::new()
        .spacing(8)
        .padding(12)
        .width(Length::Fixed(CONTENT_W + 24.0))
        .push(widget::container(widget::text::title4(section.label())).center_x(Length::Fill))
        .push(body)
        .push(widget::divider::horizontal::default())
        .push(widget::container(footer).center_x(Length::Fill));

    app.core().applet.popup_container(content).into()
}
