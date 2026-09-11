// SPDX-License-Identifier: GPL-3.0-only

//! Muestreo de /proc/meminfo.
//!
//! Fórmula del plasmoid:
//!   usada = MemTotal - (MemFree + Buffers + Cached + SReclaimable - Shmem)

use std::fs;

/// Todos los valores ya convertidos a MiB.
#[derive(Default, Clone, Copy)]
pub struct MemFields {
    pub total: f64,
    pub free: f64,
    pub buffers: f64,
    pub cached: f64,
    pub sreclaimable: f64,
    pub shmem: f64,
    pub swap_total: f64,
    pub swap_free: f64,
}

pub fn parse_meminfo(raw: &str) -> MemFields {
    let field = |name: &str| -> f64 {
        raw.lines()
            .find(|l| l.starts_with(name) && l.as_bytes().get(name.len()) == Some(&b':'))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1024.0
    };
    MemFields {
        total: field("MemTotal"),
        free: field("MemFree"),
        buffers: field("Buffers"),
        cached: field("Cached"),
        sreclaimable: field("SReclaimable"),
        shmem: field("Shmem"),
        swap_total: field("SwapTotal"),
        swap_free: field("SwapFree"),
    }
}

/// Todos los valores en MiB.
#[derive(Default)]
pub struct Mem {
    pub total: f64,
    pub used: f64,
    /// Libre + buffers + caché: lo que el QML llama `ramFree`.
    pub free: f64,
    pub cached: f64,
    pub swap_total: f64,
    pub swap_used: f64,
}

impl Mem {
    pub fn from_fields(f: MemFields) -> Self {
        let cached = f.cached + f.sreclaimable - f.shmem;
        let free = f.free + f.buffers + cached;
        Self {
            total: f.total,
            used: f.total - free,
            free,
            cached,
            swap_total: f.swap_total,
            swap_used: f.swap_total - f.swap_free,
        }
    }

    pub fn refresh(&mut self) {
        if let Ok(raw) = fs::read_to_string("/proc/meminfo") {
            *self = Self::from_fields(parse_meminfo(&raw));
        }
    }

    /// Fracción usada, 0.0..=1.0.
    pub fn fraction(&self) -> f32 {
        if self.total > 0.0 {
            (self.used / self.total).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = include_str!("../../tests/fixtures/proc_meminfo");

    #[test]
    fn parsea_los_campos_en_mib() {
        let f = parse_meminfo(MEMINFO);
        assert!((f.total - 31_931.277).abs() < 0.01);
        assert!((f.free - 2_866.769).abs() < 0.01);
    }

    #[test]
    fn cached_suma_sreclaimable_y_resta_shmem() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        // (22150232 + 669284 - 2286264) / 1024
        assert!((m.cached - 20_052.003).abs() < 0.01);
    }

    #[test]
    fn usada_es_total_menos_free_buffers_y_cached() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!((m.used - 9_010.636).abs() < 0.01);
    }

    #[test]
    fn swap_usada_es_total_menos_free() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!((m.swap_used - 2_093.140).abs() < 0.01);
    }

    #[test]
    fn no_confunde_cached_con_swapcached() {
        // SwapCached está en el fixture con valor 0; si el matcher fuera laxo,
        // `cached` cambiaría.
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!(m.cached > 20_000.0);
    }

    #[test]
    fn fraction_se_mantiene_en_rango() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!(m.fraction() > 0.0 && m.fraction() < 1.0);
    }

    #[test]
    fn libre_es_free_mas_buffers_mas_cached() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        // (2935564 + 1912) / 1024 + 20052.004
        assert!((m.free - 22_920.633).abs() < 0.01);
        assert!((m.total - m.free - m.used).abs() < 0.001);
    }
}
