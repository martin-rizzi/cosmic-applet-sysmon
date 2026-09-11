// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{self, Alignment, Length, Subscription};
use cosmic::widget::{self, Row};
use cosmic::Element;

use std::time::{Duration, Instant};

use crate::config::{Config, Section};
use crate::metrics::Metrics;

pub const CPU_ICON: &[u8] = include_bytes!("../res/icons/am-cpu-symbolic.svg");
pub const RAM_ICON: &[u8] = include_bytes!("../res/icons/am-memory-symbolic.svg");
pub const TEMP_ICON: &[u8] = include_bytes!("../res/icons/am-temperature-symbolic.svg");
pub const NET_ICON: &[u8] = include_bytes!("../res/icons/am-network-symbolic.svg");
pub const DISK_ICON: &[u8] = include_bytes!("../res/icons/am-harddisk-symbolic.svg");
pub const GPU_ICON: &[u8] = include_bytes!("../res/icons/am-gpu-symbolic.svg");
pub const UP_ICON: &[u8] = include_bytes!("../res/icons/am-up-symbolic.svg");
pub const DOWN_ICON: &[u8] = include_bytes!("../res/icons/am-down-symbolic.svg");

/// Tamaño de referencia del plasmoid: `Kirigami.Units.iconSizes.small`.
pub const REFERENCE_ICON_SIZE: f32 = 16.0;
/// Ancho por núcleo a ese tamaño de referencia.
pub const PX_PER_CORE: f32 = 4.0;
/// Ancho mínimo de la caja de CPU al tamaño de referencia. Los 4 px por núcleo
/// del plasmoid asumen bastantes núcleos: con 4 y el panel en XS la caja queda
/// en 16 px y cada barra en 3, ilegible. El mínimo sólo actúa en ese caso —
/// de 6 núcleos para arriba manda `PX_PER_CORE` y la geometría no cambia.
pub const MIN_CPU_BOX: f32 = 24.0;

pub struct SysMon {
    core: Core,
    popup: Option<Id>,
    config: Config,
    /// Todos los colectores y sus historiales.
    metrics: Metrics,
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

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// La sección cuyo panel de detalle se está viendo: popup abierto y no en ajustes.
    pub fn visible_detail(&self) -> Option<Section> {
        if self.popup.is_some() && !self.showing_settings {
            Some(self.selected)
        } else {
            None
        }
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

    /// Color del texto del panel (RGB 0–1), del tema del applet.
    fn on_bg(&self) -> [f32; 3] {
        let theme = self
            .core
            .applet
            .theme()
            .unwrap_or_else(cosmic::theme::active);
        let c = theme.cosmic().on_bg_color();
        [c.red, c.green, c.blue]
    }

    /// Color del texto del panel en hexadecimal, para los bordes de los medidores.
    pub fn text_color_hex(&self) -> String {
        let [r, g, b] = self.on_bg();
        let byte = |v: f32| (v * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
    }

    /// Texto al 35 %: `themePlaceholderTextColor` del plasmoid.
    pub fn faint_text_color(&self) -> cosmic::iced::Color {
        let [r, g, b] = self.on_bg();
        cosmic::iced::Color::from_rgba(r, g, b, 0.35)
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
                // El popup recién existe acá: que el panel no espere al próximo tick
                // para tener sus datos caros.
                let detail = app.visible_detail();
                app.metrics.refresh_expensive(detail);
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

    pub fn icon<'a>(&self, bytes: &'static [u8], size: u16) -> Element<'a, Message> {
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

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.metrics.tick(Instant::now(), self.visible_detail());
                Task::none()
            }
            Message::ConfigChanged(config) => {
                self.config = config;
                Task::none()
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                    // El popup también se cierra "desde afuera" (click en otro
                    // lado), sin pasar por la rama de misma-sección de
                    // `OpenSection` — este es el único lugar por el que pasa ese
                    // camino. Si no resetéamos acá, la próxima vez que se abra en
                    // cualquier sección seguiría mostrando ajustes en vez del
                    // detalle pedido.
                    self.showing_settings = false;
                }
                Task::none()
            }
            Message::OpenSection(section) => {
                // Mismo comportamiento que el plasmoid: la misma sección cierra,
                // otra cambia de panel sin cerrar. Si el popup estaba mostrando
                // ajustes y el usuario clickea la sección que había quedado
                // seleccionada antes de abrir el engranaje, esto la trata como
                // "la misma sección": cierra, no vuelve a mostrar el detalle. Es
                // el mismo toggle que ya existía para cualquier click repetido en
                // el mismo ícono — no hacía falta un caso especial para settings,
                // y evita que el usuario tenga que clickear dos veces (una para
                // salir de ajustes, otra para cerrar) para lograr lo mismo.
                if self.popup.is_some() && self.selected == section {
                    if let Some(p) = self.popup.take() {
                        // El popup deja de existir: que la próxima apertura
                        // muestre la sección pedida, no la página de ajustes que
                        // pudo haber quedado abierta antes de cerrar (ver
                        // `PopupClosed`, que cubre el cierre "desde afuera").
                        self.showing_settings = false;
                        cosmic::surface::surface_task(cosmic::surface::action::destroy_popup(p))
                    } else {
                        Task::none()
                    }
                } else {
                    self.selected = section;
                    // Clickear una sección expresa la intención de verla: si el
                    // popup estaba mostrando ajustes, este click saca de esa
                    // página tanto al abrirlo por primera vez (popup cerrado)
                    // como al cambiar de panel con el popup ya abierto.
                    self.showing_settings = false;
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
                self.config.save(<Self as cosmic::Application>::APP_ID);
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
                self.config.save(<Self as cosmic::Application>::APP_ID);
                Task::none()
            }
            Message::MoveSection(section, delta) => {
                self.config.section_order =
                    crate::config::move_in_order(&self.config.section_order, section, delta);
                self.config.save(<Self as cosmic::Application>::APP_ID);
                Task::none()
            }
        }
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
            metrics: Metrics::default(),
            selected: Section::Cpu,
            showing_settings: false,
        };
        // Primera lectura: deja las líneas base de los contadores.
        app.metrics.tick(Instant::now(), None);
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
        let before = self.visible_detail();
        let task = self.handle(message);
        let after = self.visible_detail();
        // Cambio de sección con el popup abierto, o salida de ajustes: el panel nuevo
        // carga sus datos caros ya, no en el próximo tick.
        if after.is_some() && after != before {
            self.metrics.refresh_expensive(after);
        }
        task
    }

    fn view(&self) -> Element<'_, Self::Message> {
        crate::ui::compact::view(self)
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        crate::ui::detail::view(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SysMon` con un popup ya abierto en `selected`, para probar el reset de
    /// `showing_settings` sin pasar por el runtime real (que necesitaría una
    /// conexión Wayland). Los campos son privados pero este módulo es hijo de
    /// `app`, así que puede construir el struct directo.
    fn con_popup_abierto(selected: Section, showing_settings: bool) -> (SysMon, Id) {
        let id = Id::unique();
        let app = SysMon {
            core: Core::default(),
            popup: Some(id),
            config: Config::default(),
            metrics: Metrics::default(),
            selected,
            showing_settings,
        };
        (app, id)
    }

    /// Variante sin popup, para probar el camino de apertura en frío.
    fn con_popup_cerrado(selected: Section, showing_settings: bool) -> SysMon {
        SysMon {
            core: Core::default(),
            popup: None,
            config: Config::default(),
            metrics: Metrics::default(),
            selected,
            showing_settings,
        }
    }

    #[test]
    fn cerrar_la_misma_seccion_apaga_showing_settings() {
        let (mut app, _id) = con_popup_abierto(Section::Cpu, true);
        let _ = cosmic::Application::update(&mut app, Message::OpenSection(Section::Cpu));
        assert!(app.popup.is_none());
        assert!(!app.showing_settings);
    }

    #[test]
    fn popup_closed_con_id_que_coincide_apaga_showing_settings() {
        let (mut app, id) = con_popup_abierto(Section::Cpu, true);
        let _ = cosmic::Application::update(&mut app, Message::PopupClosed(id));
        assert!(app.popup.is_none());
        assert!(!app.showing_settings);
    }

    #[test]
    fn popup_closed_con_id_que_no_coincide_no_toca_nada() {
        // La guarda de id: un `PopupClosed` de un popup viejo/ajeno no debe
        // cerrar el popup actual ni apagar `showing_settings`.
        let (mut app, _id) = con_popup_abierto(Section::Cpu, true);
        let otro_id = Id::unique();
        let _ = cosmic::Application::update(&mut app, Message::PopupClosed(otro_id));
        assert!(app.popup.is_some());
        assert!(app.showing_settings);
    }

    #[test]
    fn cambiar_de_seccion_con_popup_abierto_no_lo_destruye() {
        // No romper lo que ya pasó revisión: la rama de sección-distinta no debe
        // tocar `popup` (ni destruirlo ni recrearlo).
        let (mut app, id) = con_popup_abierto(Section::Cpu, false);
        let _ = cosmic::Application::update(&mut app, Message::OpenSection(Section::Ram));
        assert_eq!(app.popup, Some(id));
        assert_eq!(app.selected, Section::Ram);
    }

    #[test]
    fn cambiar_de_seccion_con_settings_abierto_lo_saca_de_ajustes() {
        // Ronda 3: con el popup mostrando ajustes, clickear OTRA sección tiene
        // que mostrar esa sección, no dejar la página de configuración puesta.
        let (mut app, id) = con_popup_abierto(Section::Cpu, true);
        let _ = cosmic::Application::update(&mut app, Message::OpenSection(Section::Ram));
        assert_eq!(app.popup, Some(id)); // sigue sin destruirse ni recrearse
        assert_eq!(app.selected, Section::Ram);
        assert!(!app.showing_settings);
    }

    #[test]
    fn abrir_con_el_popup_cerrado_no_deja_settings_prendido() {
        // Defensivo: si por algún motivo `showing_settings` quedó en `true` con
        // el popup ya cerrado (hoy no debería pasar, `PopupClosed` y el cierre
        // por misma-sección ya lo apagan), abrir una sección desde cero también
        // tiene que mostrar esa sección.
        let mut app = con_popup_cerrado(Section::Cpu, true);
        let _ = cosmic::Application::update(&mut app, Message::OpenSection(Section::Ram));
        assert_eq!(app.selected, Section::Ram);
        assert!(!app.showing_settings);
    }

    #[test]
    fn misma_seccion_con_settings_abierto_cierra_en_vez_de_volver_al_detalle() {
        // Decisión de diseño (ronda 3): si el popup muestra ajustes y se clickea
        // la sección que había quedado seleccionada antes de abrir el engranaje,
        // se trata igual que cualquier click repetido en el mismo ícono — cierra
        // el popup — en vez de agregar un caso especial que vuelva a mostrar el
        // detalle. Es el mismo test que `cerrar_la_misma_seccion_...` de la
        // ronda 2; queda repetido acá con nombre explícito para dejar registrada
        // la decisión.
        let (mut app, _id) = con_popup_abierto(Section::Cpu, true);
        let _ = cosmic::Application::update(&mut app, Message::OpenSection(Section::Cpu));
        assert!(app.popup.is_none());
        assert!(!app.showing_settings);
    }

    #[test]
    fn visible_detail_sin_popup_es_none() {
        let app = con_popup_cerrado(Section::Cpu, false);
        assert_eq!(app.visible_detail(), None);
    }

    #[test]
    fn visible_detail_con_ajustes_es_none() {
        let (app, _id) = con_popup_abierto(Section::Ram, true);
        assert_eq!(app.visible_detail(), None);
    }

    #[test]
    fn visible_detail_con_popup_es_la_seccion_elegida() {
        let (app, _id) = con_popup_abierto(Section::Ram, false);
        assert_eq!(app.visible_detail(), Some(Section::Ram));
    }
}
