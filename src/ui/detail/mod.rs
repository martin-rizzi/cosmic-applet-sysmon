// SPDX-License-Identifier: GPL-3.0-only

//! Contenido del popup: rutea por la sección elegida (`SysMon::selected`) y agrega el
//! pie con el botón del monitor de sistema y el engranaje de ajustes. Reemplaza a lo
//! que hasta la Task 4 armaba `SysMon::view_window` a mano.

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::Section;
use crate::metrics::mem::format_mib;

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
    CANDIDATES.into_iter().find(|c| {
        std::env::var_os("PATH")
            .map(|paths| std::env::split_paths(&paths).any(|dir| is_executable_file(&dir.join(c))))
            .unwrap_or(false)
    })
}

/// `is_file()` no alcanza: también hay que chequear el bit de ejecución, porque un
/// archivo regular sin permiso de ejecución (por ejemplo un `btop` no ejecutable
/// dejado por error en algún directorio del PATH) haría que el pie del popup ofrezca
/// un botón cuyo `spawn()` falla en silencio.
fn is_executable_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Detalle de CPU: uso total y una barra horizontal por núcleo.
fn cpu_detail(app: &SysMon) -> Element<'_, Message> {
    let border = app.text_color_hex();
    let mut cores = Column::new().spacing(4);
    for (i, usage) in app.cpu().cores.iter().enumerate() {
        let bar = crate::draw::horizontal_bar(
            usage / 100.0,
            140.0,
            8.0,
            crate::draw::CORE_COLORS[i % crate::draw::CORE_COLORS.len()],
            &border,
        );
        cores = cores.push(
            Row::new()
                .push(widget::text::caption(format!("cpu{i}")).width(Length::Fixed(48.0)))
                .push(
                    widget::svg(widget::svg::Handle::from_memory(bar.into_bytes()))
                        .width(Length::Fixed(140.0))
                        .height(Length::Fixed(8.0)),
                )
                .push(widget::text::caption(format!("{usage:.0}%")).width(Length::Fixed(40.0)))
                .spacing(8)
                .align_y(Alignment::Center),
        );
    }

    Column::new()
        .spacing(8)
        .push(widget::text::title4(format!("CPU {:.0}%", app.cpu().total)))
        .push(cores)
        .into()
}

/// Detalle de RAM: resumen de uso, caché y swap si existe.
fn ram_detail(app: &SysMon) -> Element<'_, Message> {
    let mem = app.mem();
    let resumen = format!(
        "{} / {} ({:.0}%)",
        format_mib(mem.used),
        format_mib(mem.total),
        mem.fraction() * 100.0
    );

    let mut col = Column::new()
        .spacing(8)
        .push(widget::text::title4("Memoria"))
        .push(widget::text::body(resumen))
        .push(widget::text::caption(format!(
            "En caché: {}",
            format_mib(mem.cached)
        )));

    if mem.swap_total > 0.0 {
        col = col.push(widget::text::caption(format!(
            "Swap: {} / {}",
            format_mib(mem.swap_used),
            format_mib(mem.swap_total)
        )));
    }

    col.into()
}

/// Contenido del popup: la sección elegida, más el pie con monitor de sistema y
/// ajustes. Las secciones sin colector todavía (Plan 2) muestran un texto de aviso.
pub fn view(app: &SysMon) -> Element<'_, Message> {
    let body: Element<Message> = match app.selected() {
        Section::Cpu => cpu_detail(app),
        Section::Ram => ram_detail(app),
        Section::Network | Section::Storage | Section::Temps | Section::Gpu => {
            widget::text::body("Sin datos todavía").into()
        }
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
        .push(body)
        .push(widget::divider::horizontal::default())
        .push(footer);

    app.core().applet.popup_container(content).into()
}
