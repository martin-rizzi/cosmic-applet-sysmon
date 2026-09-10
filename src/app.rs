// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{self, Alignment, Length, Subscription};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use std::time::Duration;

use crate::config::{Config, Section};
use crate::draw;
use crate::metrics::cpu::Cpu;
use crate::metrics::mem::{format_mib, Mem};

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
    /// Sección elegida en la vista compacta; la consume la Task 5 (click por sección).
    selected: Section,
    /// Si el popup está mostrando el panel de ajustes; la consume la Task 6.
    showing_settings: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    TogglePopup,
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
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    cosmic::surface::surface_task(cosmic::surface::action::destroy_popup(p))
                } else {
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
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        crate::ui::compact::view(self)
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let border = self.text_color_hex();
        let mut cores = Column::new().spacing(4);
        for (i, usage) in self.cpu.cores.iter().enumerate() {
            let bar = draw::horizontal_bar(
                usage / 100.0,
                140.0,
                8.0,
                draw::CORE_COLORS[i % draw::CORE_COLORS.len()],
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
                    .push(
                        widget::text::caption(format!("{usage:.0}%")).width(Length::Fixed(40.0)),
                    )
                    .spacing(8)
                    .align_y(Alignment::Center),
            );
        }

        let mem = &self.mem;
        let ram_summary = format!(
            "{} / {} ({:.0}%)",
            format_mib(mem.used),
            format_mib(mem.total),
            mem.fraction() * 100.0
        );

        let mut content = Column::new()
            .spacing(8)
            .padding(12)
            .push(widget::text::title4(format!("CPU {:.0}%", self.cpu.total)))
            .push(cores)
            .push(widget::divider::horizontal::default())
            .push(widget::text::title4("Memoria"))
            .push(widget::text::body(ram_summary))
            .push(widget::text::caption(format!(
                "En caché: {}",
                format_mib(mem.cached)
            )));

        if mem.swap_total > 0.0 {
            content = content.push(widget::text::caption(format!(
                "Swap: {} / {}",
                format_mib(mem.swap_used),
                format_mib(mem.swap_total)
            )));
        }

        self.core.applet.popup_container(content).into()
    }
}
