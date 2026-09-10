// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{self, Alignment, Length, Subscription};
use cosmic::widget::{self, Row};
use cosmic::Element;

use std::time::Duration;

use crate::config::{Config, Section};
use crate::metrics::cpu::Cpu;
use crate::metrics::mem::Mem;

pub const CPU_ICON: &[u8] = include_bytes!("../res/icons/am-cpu-symbolic.svg");
pub const RAM_ICON: &[u8] = include_bytes!("../res/icons/am-memory-symbolic.svg");

/// Tamaño de referencia del plasmoid: `Kirigami.Units.iconSizes.small`.
pub const REFERENCE_ICON_SIZE: f32 = 16.0;
/// Ancho por núcleo a ese tamaño de referencia.
pub const PX_PER_CORE: f32 = 4.0;

pub struct SysMon {
    core: Core,
    popup: Option<Id>,
    config: Config,
    cpu: Cpu,
    mem: Mem,
    /// Sección que muestra el popup (o que mostraría si estuviera abierto).
    selected: Section,
    /// Si el popup está mostrando el panel de ajustes en vez del detalle de una
    /// sección; el panel en sí lo arma la Task 6, acá sólo se guarda el estado.
    showing_settings: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    /// Click en una sección del panel: abre el popup en esa sección, lo cierra si ya
    /// estaba mostrando la misma, o cambia de panel sin cerrarlo. Ver `main.qml:93-98`
    /// del plasmoid.
    OpenSection(Section),
    /// Click en el ícono de monitor de sistema del pie del popup.
    OpenSystemMonitor,
    /// Click en el engranaje del pie del popup.
    OpenSettings,
    /// Click en la cruz de la página de ajustes: vuelve al detalle.
    CloseSettings,
    /// Switch de "Mostrar" de una sección, en la página de ajustes.
    ToggleSection(Section, bool),
    /// Botones +/- del intervalo de actualización, en la página de ajustes.
    SetInterval(u32),
    /// Flechas ↑/↓ del orden de secciones, en la página de ajustes.
    MoveSection(Section, i32),
    PopupClosed(Id),
    ConfigChanged(Config),
}

impl SysMon {
    pub fn core(&self) -> &Core {
        &self.core
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn mem(&self) -> &Mem {
        &self.mem
    }

    pub fn selected(&self) -> Section {
        self.selected
    }

    pub fn showing_settings(&self) -> bool {
        self.showing_settings
    }

    /// `smallSpacing` de Kirigami escalado al tamaño de ícono del panel.
    pub fn spacing(&self) -> u16 {
        let h = f32::from(self.core.applet.suggested_size(true).1);
        (4.0 * (h / REFERENCE_ICON_SIZE)).round() as u16
    }

    /// Color del texto del panel en hexadecimal, para los bordes de los medidores.
    pub fn text_color_hex(&self) -> String {
        let theme = self
            .core
            .applet
            .theme()
            .unwrap_or_else(cosmic::theme::active);
        let c = theme.cosmic().on_bg_color();
        format!(
            "#{:02x}{:02x}{:02x}",
            (c.red * 255.0).round() as u8,
            (c.green * 255.0).round() as u8,
            (c.blue * 255.0).round() as u8
        )
    }

    /// Arma la `Task` que abre el popup, sin tocar `self.selected` ni
    /// `showing_settings` — eso lo decide quien la llama. Compartida por
    /// `OpenSection` (cuando no hay popup) y `OpenSettings` (cuando el botón de
    /// resguardo del panel lo dispara con el popup cerrado).
    fn open_popup_task() -> Task<Message> {
        cosmic::surface::surface_task(cosmic::surface::action::app_popup(
            |_| Default::default(),
            |app: &mut SysMon| {
                let new_id = Id::unique();
                app.popup.replace(new_id);
                app.core.applet.get_popup_settings(
                    app.core.main_window_id().unwrap(),
                    new_id,
                    None,
                    None,
                    None,
                )
            },
            None,
        ))
    }

    fn label(&self, value: String) -> Element<'_, Message> {
        self.core
            .applet
            .text(value)
            .font(cosmic::font::bold())
            .into()
    }

    fn icon<'a>(&self, bytes: &'static [u8], size: u16) -> Element<'a, Message> {
        let mut handle = widget::icon::from_svg_bytes(bytes);
        handle.symbolic = true;
        widget::icon(handle).size(size).into()
    }

    fn tick_duration(&self) -> Duration {
        // Mismo rango que el spinner del plasmoid.
        Duration::from_millis(self.config.update_interval.clamp(500, 10_000) as u64)
    }

    /// Una sección del panel: ícono + medidor + porcentaje.
    pub fn section<'a>(
        &'a self,
        icon: &'static [u8],
        icon_size: u16,
        meter_svg: String,
        meter_w: f32,
        meter_h: f32,
        value: String,
        spacing: u16,
    ) -> Element<'a, Message> {
        let meter = widget::svg(widget::svg::Handle::from_memory(meter_svg.into_bytes()))
            .width(Length::Fixed(meter_w))
            .height(Length::Fixed(meter_h));

        Row::new()
            .push(self.icon(icon, icon_size))
            .push(meter)
            .push(self.label(value))
            .spacing(spacing)
            .align_y(Alignment::Center)
            .into()
    }
}

impl cosmic::Application for SysMon {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "io.github.martin_rizzi.CosmicSysMon";

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut app = SysMon {
            core,
            popup: None,
            config: Config::load(Self::APP_ID),
            cpu: Cpu::default(),
            mem: Mem::default(),
            selected: Section::Cpu,
            showing_settings: false,
        };
        // Primera lectura: deja la línea base de /proc/stat y la RAM ya poblada.
        app.cpu.refresh();
        app.mem.refresh();
        (app, Task::none())
    }

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(self.tick_duration()).map(|_| Message::Tick),
            self.core
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::ConfigChanged(update.config)),
        ])
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Tick => {
                self.cpu.refresh();
                self.mem.refresh();
                Task::none()
            }
            Message::ConfigChanged(config) => {
                self.config = config;
                Task::none()
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
                Task::none()
            }
            Message::OpenSection(section) => {
                // Mismo comportamiento que el plasmoid: la misma sección cierra,
                // otra cambia de panel sin cerrar.
                if self.popup.is_some() && self.selected == section {
                    if let Some(p) = self.popup.take() {
                        cosmic::surface::surface_task(cosmic::surface::action::destroy_popup(p))
                    } else {
                        Task::none()
                    }
                } else {
                    self.selected = section;
                    if self.popup.is_some() {
                        Task::none()
                    } else {
                        Self::open_popup_task()
                    }
                }
            }
            Message::OpenSystemMonitor => {
                if let Some(cmd) = crate::ui::detail::system_monitor_command() {
                    // Único fork del applet, y sólo por acción explícita del usuario.
                    let _ = std::process::Command::new(cmd).spawn();
                }
                Task::none()
            }
            Message::OpenSettings => {
                self.showing_settings = true;
                // El engranaje del pie llega acá con el popup ya abierto (Task::none
                // alcanza). Pero el botón de resguardo de `ui::compact` (cuando
                // ningún `show_*` deja botones de sección en el panel) dispara este
                // mismo mensaje con el popup cerrado: si no lo abriéramos acá, el
                // flag `showing_settings` quedaría prendido sin que `view_window` se
                // llegue a invocar nunca, porque sólo se invoca sobre un popup vivo.
                if self.popup.is_some() {
                    Task::none()
                } else {
                    Self::open_popup_task()
                }
            }
            Message::CloseSettings => {
                self.showing_settings = false;
                Task::none()
            }
            Message::ToggleSection(section, value) => {
                match section {
                    Section::Cpu => self.config.show_cpu = value,
                    Section::Ram => self.config.show_ram = value,
                    Section::Network => self.config.show_network = value,
                    Section::Storage => self.config.show_storage = value,
                    Section::Temps => self.config.show_temps = value,
                    Section::Gpu => self.config.show_gpu = value,
                }
                self.config.save(Self::APP_ID);
                // Si se apagó la sección que el popup está mostrando, cae a la
                // primera todavía visible. Si no queda ninguna visible, `selected`
                // conserva el último valor: no hay panic, sólo el detalle
                // muestra una sección oculta hasta que se vuelva a encender algo.
                if !self.config.shown(self.selected) {
                    if let Some(first) = self.config.ordered_sections().first() {
                        self.selected = *first;
                    }
                }
                Task::none()
            }
            Message::SetInterval(ms) => {
                self.config.update_interval = ms.clamp(500, 10_000);
                self.config.save(Self::APP_ID);
                Task::none()
            }
            Message::MoveSection(section, delta) => {
                self.config.section_order =
                    crate::config::move_in_order(&self.config.section_order, section, delta);
                self.config.save(Self::APP_ID);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        crate::ui::compact::view(self)
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        crate::ui::detail::view(self)
    }
}
