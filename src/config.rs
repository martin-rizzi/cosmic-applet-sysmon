// SPDX-License-Identifier: GPL-3.0-only

//! Las ocho opciones del plasmoid com.labatata.sysmonitor, con los mismos nombres y
//! defaults que `contents/config/main.xml`.

use cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic_config::{Config as RawConfig, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

pub const DEFAULT_SECTION_ORDER: &str = "temps,network,storage,cpu,gpu,ram";

/// Los índices son los del plasmoid (`FullView.qml`) y no deben cambiar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Cpu,
    Ram,
    Network,
    Storage,
    Temps,
    Gpu,
}

impl Section {
    pub fn all() -> [Section; 6] {
        [
            Section::Cpu,
            Section::Ram,
            Section::Network,
            Section::Storage,
            Section::Temps,
            Section::Gpu,
        ]
    }

    pub fn key(&self) -> &'static str {
        match self {
            Section::Cpu => "cpu",
            Section::Ram => "ram",
            Section::Network => "network",
            Section::Storage => "storage",
            Section::Temps => "temps",
            Section::Gpu => "gpu",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Section::Cpu => "CPU",
            Section::Ram => "RAM",
            Section::Network => "Red",
            Section::Storage => "Disco",
            Section::Temps => "Temperaturas",
            Section::Gpu => "GPU",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Section::Cpu => 0,
            Section::Ram => 1,
            Section::Network => 2,
            Section::Storage => 3,
            Section::Temps => 4,
            Section::Gpu => 5,
        }
    }

    fn from_key(key: &str) -> Option<Section> {
        Section::all().into_iter().find(|s| s.key() == key)
    }
}

/// Misma lógica que `normalizedSectionOrder` en `ConfigGeneral.qml`: descarta claves
/// desconocidas y duplicadas, y agrega al final las que falten en el orden default.
pub fn normalize_section_order(raw: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::with_capacity(6);
    for part in raw.split(',') {
        if let Some(s) = Section::from_key(part.trim()) {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    for s in DEFAULT_SECTION_ORDER
        .split(',')
        .filter_map(|k| Section::from_key(k.trim()))
    {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    pub update_interval: u32,
    pub show_cpu: bool,
    pub show_ram: bool,
    pub show_network: bool,
    pub show_storage: bool,
    pub show_temps: bool,
    pub show_gpu: bool,
    pub section_order: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            update_interval: 2000,
            show_cpu: true,
            show_ram: true,
            show_network: true,
            show_storage: true,
            show_temps: true,
            show_gpu: true,
            section_order: DEFAULT_SECTION_ORDER.to_string(),
        }
    }
}

impl Config {
    pub fn shown(&self, s: Section) -> bool {
        match s {
            Section::Cpu => self.show_cpu,
            Section::Ram => self.show_ram,
            Section::Network => self.show_network,
            Section::Storage => self.show_storage,
            Section::Temps => self.show_temps,
            Section::Gpu => self.show_gpu,
        }
    }

    /// Orden normalizado, ya filtrado por los `show_*`.
    pub fn ordered_sections(&self) -> Vec<Section> {
        normalize_section_order(&self.section_order)
            .into_iter()
            .filter(|s| self.shown(*s))
            .collect()
    }

    /// Carga desde cosmic-config; ante error devuelve los defaults.
    pub fn load(app_id: &str) -> Self {
        match RawConfig::new(app_id, <Self as CosmicConfigEntry>::VERSION) {
            Ok(raw) => match Self::get_entry(&raw) {
                Ok(c) => c,
                Err((_errors, fallback)) => fallback,
            },
            Err(_) => Self::default(),
        }
    }

    /// Persiste una clave suelta. Escribe un archivo por clave, atómicamente.
    /// Si falla, se avisa por stderr en vez de tragarse el error en silencio.
    pub fn save(&self, app_id: &str) {
        match RawConfig::new(app_id, <Self as CosmicConfigEntry>::VERSION) {
            Ok(raw) => {
                if let Err(err) = self.write_entry(&raw) {
                    eprintln!("sysmon: no se pudo guardar la configuración: {err}");
                }
            }
            Err(err) => {
                eprintln!("sysmon: no se pudo abrir cosmic-config para guardar: {err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_default_es_el_del_plasmoid() {
        let c = Config::default();
        assert_eq!(c.update_interval, 2000);
        assert!(c.show_cpu && c.show_ram && c.show_network);
        assert!(c.show_storage && c.show_temps && c.show_gpu);
        assert_eq!(c.section_order, DEFAULT_SECTION_ORDER);
    }

    #[test]
    fn normaliza_el_orden_por_defecto() {
        let order = normalize_section_order(DEFAULT_SECTION_ORDER);
        assert_eq!(
            order,
            vec![
                Section::Temps,
                Section::Network,
                Section::Storage,
                Section::Cpu,
                Section::Gpu,
                Section::Ram,
            ]
        );
    }

    #[test]
    fn descarta_claves_desconocidas() {
        let order = normalize_section_order("cpu,inventada,ram");
        assert_eq!(order[0], Section::Cpu);
        assert_eq!(order[1], Section::Ram);
        assert_eq!(order.len(), 6);
    }

    #[test]
    fn descarta_duplicados() {
        let order = normalize_section_order("cpu,cpu,ram");
        assert_eq!(order.len(), 6);
        assert_eq!(order.iter().filter(|s| **s == Section::Cpu).count(), 1);
    }

    #[test]
    fn agrega_al_final_las_que_faltan() {
        let order = normalize_section_order("ram");
        assert_eq!(order[0], Section::Ram);
        // las otras cinco entran en el orden default
        assert_eq!(order.len(), 6);
        assert!(order.contains(&Section::Gpu));
    }

    #[test]
    fn tolera_espacios_y_cadena_vacia() {
        assert_eq!(normalize_section_order(" cpu , ram ")[0], Section::Cpu);
        assert_eq!(normalize_section_order("").len(), 6);
    }

    #[test]
    fn ordered_sections_filtra_las_apagadas() {
        let mut c = Config::default();
        c.show_temps = false;
        c.show_network = false;
        let visible = c.ordered_sections();
        assert!(!visible.contains(&Section::Temps));
        assert!(!visible.contains(&Section::Network));
        assert_eq!(visible.len(), 4);
        // conserva el orden relativo del default
        assert_eq!(visible[0], Section::Storage);
    }

    #[test]
    fn los_indices_son_los_del_plasmoid() {
        assert_eq!(Section::Cpu.index(), 0);
        assert_eq!(Section::Ram.index(), 1);
        assert_eq!(Section::Network.index(), 2);
        assert_eq!(Section::Storage.index(), 3);
        assert_eq!(Section::Temps.index(), 4);
        assert_eq!(Section::Gpu.index(), 5);
    }
}
