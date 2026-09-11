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

    if buttons.is_empty() {
        // No borrar por "código muerto": sin esto, apagar `show_cpu` y `show_ram`
        // (las únicas dos secciones con colector hoy — Network/Storage/Temps/Gpu
        // siempre devuelven `None` en `section_content` porque el Plan 2 todavía no
        // les puso colector) deja el panel sin ningún botón. El engranaje que abre
        // esta misma página de ajustes vive en el pie del popup, y a ese pie sólo
        // se llega clickeando un botón de sección — que ya no existiría. Este botón
        // de resguardo es el único camino de vuelta a la configuración en ese
        // estado; `Message::OpenSettings` abre el popup si hace falta (ver
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

/// `None` para las secciones que todavía no tienen colector (Plan 2).
pub fn section_content<'a>(app: &'a SysMon, s: Section) -> Option<Element<'a, Message>> {
    let icon_size = app.core().applet.suggested_size(true).1;
    let h = f32::from(icon_size);
    let border = app.text_color_hex();
    let spacing = app.spacing();

    match s {
        Section::Cpu => {
            let scale = h / crate::app::REFERENCE_ICON_SIZE;
            let cores = app.metrics().cpu.core_count().max(1) as f32;
            let w = cpu_box_width(cores, h, scale);
            let cpu = &app.metrics().cpu;
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
            let fraction = app.metrics().mem.fraction();
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
}
