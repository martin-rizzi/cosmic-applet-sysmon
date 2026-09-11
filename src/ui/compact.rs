// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::Section;
use crate::draw::{self, DualGraph};

/// Arma la vista compacta del panel recorriendo `Config::ordered_sections()`: sólo
/// dibuja las secciones visibles, en el orden configurado. Cada sección es su propio
/// botón — a diferencia del botón único que envolvía todo el panel hasta la Task 4,
/// acá cada uno dispara `Message::OpenSection` con su propia sección, lo que permite
/// la semántica de toggle/cambio de panel del plasmoid (`main.qml:93-98`).
pub fn view(app: &SysMon) -> Element<'_, Message> {
    let horizontal = app.core().applet.is_horizontal();
    let spacing = app.spacing();
    let padding = if horizontal {
        [0, app.core().applet.suggested_padding(true).1]
    } else {
        [app.core().applet.suggested_padding(true).0, 0]
    };

    let mut buttons: Vec<Element<Message>> = Vec::new();
    for s in app.config().ordered_sections() {
        if let Some(content) = section_content(app, s) {
            let button = widget::button::custom(content)
                .padding(padding)
                .class(cosmic::theme::Button::AppletIcon)
                .on_press(Message::OpenSection(s));
            buttons.push(button.into());
        }
    }

    if buttons.is_empty() {
        // No borrar por "código muerto": sin esto, apagar todos los `show_*` (o dejar
        // sólo GPU en una máquina sin fuente de datos de GPU, que `section_content`
        // oculta) deja el panel sin ningún botón. El engranaje que abre esta misma
        // página de ajustes vive en el pie del popup, y a ese pie sólo se llega
        // clickeando un botón de sección — que ya no existiría. Este botón de
        // resguardo es el único camino de vuelta a la configuración en ese estado;
        // `Message::OpenSettings` abre el popup si hace falta (ver
        // `SysMon::open_popup_task` en `app.rs`) y además muestra la página.
        let boton = widget::button::icon(widget::icon::from_name("preferences-system-symbolic"))
            .padding(padding)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(Message::OpenSettings);
        buttons.push(boton.into());
    }

    let content: Element<Message> = if horizontal {
        let mut row = Row::new().spacing(spacing * 2).align_y(Alignment::Center);
        for b in buttons {
            row = row.push(b);
        }
        row.into()
    } else {
        let mut col = Column::new().spacing(spacing * 2).align_x(Alignment::Center);
        for b in buttons {
            col = col.push(b);
        }
        col.into()
    };

    app.core().applet.autosize_window(content).into()
}

/// Contenido de una sección del panel, o `None` si no hay nada que mostrar (GPU sin
/// ninguna fuente de datos, spec §3.5).
pub fn section_content<'a>(app: &'a SysMon, s: Section) -> Option<Element<'a, Message>> {
    let icon_size = app.core().applet.suggested_size(true).1;
    let h = f32::from(icon_size);
    let scale = h / crate::app::REFERENCE_ICON_SIZE;
    let border = app.text_color_hex();
    let spacing = app.spacing();
    let m = app.metrics();

    match s {
        Section::Cpu => {
            let w = cpu_box_width(m.cpu.core_count().max(1) as f32, h, scale);
            Some(app.section(
                crate::app::CPU_ICON,
                icon_size,
                draw::cpu_bars(&m.cpu.cores, w, h, &border),
                w,
                h,
                crate::format::percent(m.cpu.total),
                spacing,
            ))
        }
        Section::Ram => {
            let w = (h * 0.7).round();
            let fraction = m.mem.fraction();
            Some(app.section(
                crate::app::RAM_ICON,
                icon_size,
                draw::usage_meter(fraction, w, h, &border),
                w,
                h,
                format!("{:.0}%", fraction * 100.0),
                spacing,
            ))
        }
        Section::Storage => {
            // `storageDevices[0]`: el primer montaje. Sin porcentaje escrito, sin naranja
            // y con relleno mínimo de 3 px, como la sección de CompactView.qml.
            let fraction = m.storage.primary().map(|u| u.percent as f32 / 100.0).unwrap_or(0.0);
            let w = (h * 0.7).round();
            let svg = draw::usage_meter_with(fraction, w, h, &border, draw::critical_only_color(fraction), 3.0 * scale);
            Some(
                Row::new()
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(app.icon(crate::app::DISK_ICON, icon_size))
                    .push(meter(svg, w, h))
                    .into(),
            )
        }
        Section::Gpu => {
            if !m.gpu.available() {
                return None;
            }
            let usage = m.gpu.summary().map(|d| d.usage).unwrap_or(0.0);
            let fraction = usage / 100.0;
            let w = (h * 0.7).round();
            let svg = draw::usage_meter_with(fraction, w, h, &border, draw::critical_only_color(fraction), 0.0);
            Some(app.section(crate::app::GPU_ICON, icon_size, svg, w, h, crate::format::percent(usage), spacing))
        }
        Section::Temps => {
            // Las dos primeras lecturas, como el Repeater de `min(temperatures.length, 2)`.
            let px = two_line_px(h);
            let mut lines = Column::new();
            for i in 0..2 {
                let line: Element<'a, Message> = match m.temps.get(i) {
                    Some(r) => {
                        let text = small_text(format!("{:.1} °C", r.celsius), px);
                        match crate::metrics::temp::severity_color(r.celsius) {
                            Some(hex) => text.class(cosmic::theme::Text::Color(draw::hex_color(hex))).into(),
                            None => text.into(),
                        }
                    }
                    None => small_text("-- °C".to_string(), px)
                        .class(cosmic::theme::Text::Color(app.faint_text_color()))
                        .into(),
                };
                lines = lines.push(line);
            }
            Some(
                Row::new()
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(app.icon(crate::app::TEMP_ICON, icon_size))
                    .push(lines)
                    .into(),
            )
        }
        Section::Network => {
            let px = two_line_px(h);
            let graph_w = (36.0 * scale).round();
            let graph = draw::dual_history_graph(
                &DualGraph {
                    top: &m.net_up_history.points(),
                    bottom: &m.net_down_history.points(),
                    top_color: draw::CRITICAL,
                    bottom_color: draw::NORMAL,
                    gap: 2.0,
                    top_padding: 2.0,
                    grid: false,
                },
                graph_w,
                h,
                &border,
            );
            let rates = Column::new()
                .push(rate_line(app, crate::app::UP_ICON, m.net.tx_rate, px))
                .push(rate_line(app, crate::app::DOWN_ICON, m.net.rx_rate, px));
            Some(
                Row::new()
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(app.icon(crate::app::NET_ICON, icon_size))
                    .push(meter(graph, graph_w, h))
                    .push(rates)
                    .into(),
            )
        }
    }
}

fn meter<'a>(svg: String, w: f32, h: f32) -> Element<'a, Message> {
    widget::svg(widget::svg::Handle::from_memory(svg.into_bytes()))
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .into()
}

/// Letra de las secciones de dos renglones (temperaturas y red): 60 % del ícono, para
/// que los dos renglones entren en el alto del panel. Nunca menos de 8 px.
fn two_line_px(h: f32) -> f32 {
    (h * 0.6).round().max(8.0)
}

fn small_text<'a>(value: String, px: f32) -> widget::Text<'a, cosmic::Theme> {
    widget::text(value).size(px).font(cosmic::font::bold())
}

/// Flecha + tasa compacta, un renglón de la sección de red.
fn rate_line<'a>(app: &'a SysMon, arrow: &'static [u8], rate: f64, px: f32) -> Element<'a, Message> {
    Row::new()
        .spacing(2)
        .align_y(Alignment::Center)
        .push(app.icon(arrow, px as u16))
        .push(small_text(crate::format::compact_rate(rate), px))
        .into()
}

/// Ancho de la caja de CPU: como en el QML, `PX_PER_CORE` por núcleo a escala,
/// nunca más angosta que el 70 % de la altura ni que `MIN_CPU_BOX`.
fn cpu_box_width(cores: f32, h: f32, scale: f32) -> f32 {
    (h * 0.7)
        .max(cores * crate::app::PX_PER_CORE * scale)
        .max(crate::app::MIN_CPU_BOX * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El caso que motivó el mínimo: panel XS (ícono de 16 px) y 4 núcleos.
    /// Sin él la caja quedaba en 16 px y cada barra en 3.
    #[test]
    fn pocos_nucleos_en_panel_chico_llegan_al_minimo() {
        let w = cpu_box_width(4.0, 16.0, 1.0);
        assert_eq!(w, 24.0);
        assert!(w / 4.0 >= 5.0, "cada barra tiene que pasar los 5 px");
    }

    /// De 6 núcleos para arriba manda `PX_PER_CORE`: la geometría calcada del
    /// plasmoid queda igual que antes de agregar el mínimo.
    #[test]
    fn muchos_nucleos_conservan_la_geometria_del_plasmoid() {
        assert_eq!(cpu_box_width(8.0, 16.0, 1.0), 32.0);
        assert_eq!(cpu_box_width(16.0, 16.0, 1.0), 64.0);
    }

    /// El mínimo escala con el panel, igual que el resto de la geometría.
    #[test]
    fn el_minimo_escala_con_el_panel() {
        assert_eq!(cpu_box_width(4.0, 32.0, 2.0), 48.0);
    }

    /// Con un solo núcleo sigue mandando el mínimo, no el 70 % de la altura.
    #[test]
    fn un_solo_nucleo_no_colapsa_la_caja() {
        assert_eq!(cpu_box_width(1.0, 16.0, 1.0), 24.0);
    }

    #[test]
    fn dos_renglones_entran_en_el_alto_del_icono() {
        assert_eq!(two_line_px(16.0), 10.0);
        assert_eq!(two_line_px(32.0), 19.0);
        // nunca menos de 8 px, ilegible
        assert_eq!(two_line_px(10.0), 8.0);
    }
}
