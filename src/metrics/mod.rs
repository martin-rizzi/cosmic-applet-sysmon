// SPDX-License-Identifier: GPL-3.0-only

//! Colectores y el agregado que decide qué se refresca en cada tick.

pub mod cpu;
pub mod cpuinfo;
pub mod disk;
pub mod history;
pub mod mem;
pub mod net;
pub mod rate;
pub mod temp;

use std::time::Instant;

use crate::config::Section;
use cpu::Cpu;
use cpuinfo::CpuInfo;
use disk::Storage;
use history::History;
use mem::Mem;
use net::Net;

/// Contenido de un archivo chico de /proc o /sys, sin espacios ni salto final.
pub(crate) fn read_trimmed(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Qué parte cara del muestreo corre (spec §3.1). Lo que no figura acá es barato y corre
/// en cada tick, porque lo usa la vista del panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Expensive {
    /// Top de procesos por CPU, modelo y reloj de /proc/cpuinfo, uptime.
    pub cpu_detail: bool,
    /// Top de procesos por memoria.
    pub ram_detail: bool,
    /// Todos los puntos de montaje e inventario de /sys/block.
    pub storage_detail: bool,
    /// `nvidia-smi` y VRAM por proceso.
    pub gpu_detail: bool,
}

/// Lo caro corre sólo mientras su panel de detalle está a la vista.
pub fn expensive_for(detail: Option<Section>) -> Expensive {
    let mut e = Expensive::default();
    match detail {
        Some(Section::Cpu) => e.cpu_detail = true,
        Some(Section::Ram) => e.ram_detail = true,
        Some(Section::Storage) => e.storage_detail = true,
        Some(Section::Gpu) => e.gpu_detail = true,
        Some(Section::Network) | Some(Section::Temps) | None => {}
    }
    e
}

#[derive(Default)]
pub struct Metrics {
    pub cpu: Cpu,
    pub mem: Mem,
    /// Modelo y reloj; sólo se actualizan con el panel de CPU a la vista.
    pub cpu_info: CpuInfo,
    /// Segundos desde el arranque; ídem.
    pub uptime_secs: u64,
    /// Un renglón por sensor, en el orden de hwmon.
    pub temps: Vec<temp::Reading>,
    pub net: Net,
    /// Subida normalizada por muestra (ver `Net::normalized`).
    pub net_up_history: History,
    /// Bajada normalizada por muestra.
    pub net_down_history: History,
    pub storage: Storage,
    /// Lectura sobre `disk::GRAPH_SCALE`, saturada en 1.
    pub disk_read_history: History,
    /// Escritura, ídem.
    pub disk_write_history: History,
    /// Últimos 60 usos totales de CPU (0–1), uno por tick.
    pub cpu_history: History,
    /// Últimas 60 fracciones de RAM usada, una por tick.
    pub ram_history: History,
    last_tick: Option<Instant>,
}

impl Metrics {
    /// Un tick de muestreo. `detail` es la sección cuyo panel está a la vista, si hay alguna.
    pub fn tick(&mut self, now: Instant, detail: Option<Section>) {
        let elapsed = self.elapsed_since_last(now);
        self.cpu.refresh();
        self.mem.refresh();
        self.temps = temp::read_hwmon(std::path::Path::new("/sys/class/hwmon"));
        self.net.refresh(elapsed);
        let (down, up) = self.net.normalized();
        self.net_down_history.push(down);
        self.net_up_history.push(up);
        self.storage.io.refresh(elapsed);
        self.disk_read_history.push(disk::graph_fraction(self.storage.io.read_rate));
        self.disk_write_history.push(disk::graph_fraction(self.storage.io.write_rate));
        if !expensive_for(detail).storage_detail {
            self.storage.refresh_primary();
        }
        self.cpu_history.push(self.cpu.total / 100.0);
        self.ram_history.push(self.mem.fraction());
        self.refresh_expensive(detail);
    }

    /// La parte cara, sola. `SysMon` la llama también al abrir un panel, para que no
    /// muestre la lista vacía hasta el próximo tick.
    pub fn refresh_expensive(&mut self, detail: Option<Section>) {
        let expensive = expensive_for(detail);
        if expensive.cpu_detail {
            self.cpu_info.refresh();
            self.uptime_secs = cpuinfo::read_uptime();
        }
        if expensive.storage_detail {
            self.storage.refresh_full();
        }
    }

    /// Segundos desde el tick anterior; 0 en el primero.
    fn elapsed_since_last(&mut self, now: Instant) -> f64 {
        let elapsed = self
            .last_tick
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.0);
        self.last_tick = Some(now);
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_panel_a_la_vista_no_corre_nada_caro() {
        assert_eq!(expensive_for(None), Expensive::default());
    }

    #[test]
    fn cada_panel_prende_solo_lo_suyo() {
        assert_eq!(
            expensive_for(Some(Section::Cpu)),
            Expensive { cpu_detail: true, ..Expensive::default() }
        );
        assert_eq!(
            expensive_for(Some(Section::Ram)),
            Expensive { ram_detail: true, ..Expensive::default() }
        );
        assert_eq!(
            expensive_for(Some(Section::Storage)),
            Expensive { storage_detail: true, ..Expensive::default() }
        );
        assert_eq!(
            expensive_for(Some(Section::Gpu)),
            Expensive { gpu_detail: true, ..Expensive::default() }
        );
    }

    #[test]
    fn red_y_temperaturas_no_tienen_parte_cara() {
        assert_eq!(expensive_for(Some(Section::Network)), Expensive::default());
        assert_eq!(expensive_for(Some(Section::Temps)), Expensive::default());
    }
}
