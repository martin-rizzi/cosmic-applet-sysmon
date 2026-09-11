// SPDX-License-Identifier: GPL-3.0-only

//! Modelo y reloj de /proc/cpuinfo, y uptime de /proc/uptime. Sólo los muestra el panel
//! de CPU, así que sólo se leen con ese panel a la vista.

use std::fs;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CpuInfo {
    /// Primer `model name`; vacío donde no existe (ARM).
    pub model: String,
    /// Promedio de los `cpu MHz` positivos; 0 si no hay ninguno.
    pub mhz: f64,
}

pub fn parse_cpuinfo(raw: &str) -> CpuInfo {
    let mut info = CpuInfo::default();
    let (mut sum, mut count) = (0.0, 0u32);
    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "model name" if info.model.is_empty() => info.model = value.trim().to_string(),
            "cpu MHz" => {
                if let Ok(mhz) = value.trim().parse::<f64>() {
                    if mhz > 0.0 {
                        sum += mhz;
                        count += 1;
                    }
                }
            }
            _ => {}
        }
    }
    if count > 0 {
        info.mhz = sum / f64::from(count);
    }
    info
}

impl CpuInfo {
    pub fn refresh(&mut self) {
        if let Ok(raw) = fs::read_to_string("/proc/cpuinfo") {
            *self = parse_cpuinfo(&raw);
        }
    }
}

/// Segundos enteros del primer campo de /proc/uptime.
pub fn parse_uptime(raw: &str) -> u64 {
    raw.split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v as u64)
        .unwrap_or(0)
}

pub fn read_uptime() -> u64 {
    fs::read_to_string("/proc/uptime")
        .map(|raw| parse_uptime(&raw))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPUINFO: &str = include_str!("../../tests/fixtures/proc_cpuinfo");

    #[test]
    fn toma_el_primer_modelo_y_promedia_el_reloj() {
        let info = parse_cpuinfo(CPUINFO);
        assert_eq!(info.model, "Intel(R) Core(TM) i5-10400T CPU @ 2.00GHz");
        assert!((info.mhz - 3222.964).abs() < 0.001);
    }

    #[test]
    fn sin_model_name_queda_vacio() {
        // ARM expone `processor` y `BogoMIPS`, sin `model name` ni `cpu MHz`.
        let info = parse_cpuinfo("processor\t: 0\nBogoMIPS\t: 108.00\n");
        assert_eq!(info, CpuInfo::default());
    }

    #[test]
    fn uptime_toma_los_segundos_enteros() {
        assert_eq!(parse_uptime("1417.16 14830.28\n"), 1417);
        assert_eq!(parse_uptime(""), 0);
    }
}
