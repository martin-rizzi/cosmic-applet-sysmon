// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Alignment;
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::Section;

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

/// `None` para las secciones que todavía no tienen colector (Plan 2).
pub fn section_content<'a>(app: &'a SysMon, s: Section) -> Option<Element<'a, Message>> {
    let icon_size = app.core().applet.suggested_size(true).1;
    let h = f32::from(icon_size);
    let border = app.text_color_hex();
    let spacing = app.spacing();

    match s {
        Section::Cpu => {
            // Ancho de la caja: 4 px por núcleo a escala, con mínimo del 70 % de la altura.
            let scale = h / crate::app::REFERENCE_ICON_SIZE;
            let cores = app.cpu().core_count().max(1) as f32;
            let w = (h * 0.7).max(cores * crate::app::PX_PER_CORE * scale);
            let cpu = app.cpu();
            Some(app.section(
                crate::app::CPU_ICON,
                icon_size,
                crate::draw::cpu_bars(&cpu.cores, w, h, &border),
                w,
                h,
                // Como el plasmoid: un decimal sólo por debajo del 1 %.
                if cpu.total < 1.0 {
                    format!("{:.1}%", cpu.total)
                } else {
                    format!("{:.0}%", cpu.total)
                },
                spacing,
            ))
        }
        Section::Ram => {
            let w = (h * 0.7).round();
            let fraction = app.mem().fraction();
            Some(app.section(
                crate::app::RAM_ICON,
                icon_size,
                crate::draw::usage_meter(fraction, w, h, &border),
                w,
                h,
                format!("{:.0}%", fraction * 100.0),
                spacing,
            ))
        }
        // Sin colector todavía: no ocupan lugar en el panel.
        Section::Network | Section::Storage | Section::Temps | Section::Gpu => None,
    }
}
