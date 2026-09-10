// SPDX-License-Identifier: GPL-3.0-only

//! Muestreo de /proc/stat.
//!
//! Fórmula del plasmoid com.labatata.sysmonitor:
//!   active = user+nice+system+irq+softirq, total = active+idle+iowait,
//!   y el porcentaje sale del delta contra la muestra anterior.

use std::fs;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CpuTicks {
    pub total: u64,
    pub active: u64,
}

/// Índice 0 es el agregado "cpu"; el resto son los núcleos, en el orden del archivo.
pub fn parse_stat(raw: &str) -> Vec<CpuTicks> {
    let mut out = Vec::with_capacity(8);
    for line in raw.lines() {
        if !line.starts_with("cpu") {
            break;
        }
        let mut it = line.split_whitespace();
        it.next();
        let v: Vec<u64> = it.take(7).map(|f| f.parse().unwrap_or(0)).collect();
        if v.len() < 7 {
            continue;
        }
        let (user, nice, system, idle, iowait, irq, softirq) =
            (v[0], v[1], v[2], v[3], v[4], v[5], v[6]);
        let active = user + nice + system + irq + softirq;
        out.push(CpuTicks {
            total: active + idle + iowait,
            active,
        });
    }
    out
}

#[derive(Default)]
pub struct Cpu {
    prev: Vec<CpuTicks>,
    pub cores: Vec<f32>,
    pub total: f32,
}

impl Cpu {
    pub fn apply(&mut self, now: Vec<CpuTicks>) {
        if now.is_empty() {
            return;
        }
        let mut pcts: Vec<f32> = Vec::with_capacity(now.len());
        for (i, sample) in now.iter().enumerate() {
            let pct = match self.prev.get(i) {
                Some(prev) if sample.total > prev.total => {
                    let dt = (sample.total - prev.total) as f32;
                    let da = sample.active.saturating_sub(prev.active) as f32;
                    (da / dt * 100.0).clamp(0.0, 100.0)
                }
                // Primera muestra o contador reiniciado: 0, como el plasmoid.
                _ => 0.0,
            };
            pcts.push(pct);
        }
        self.total = pcts[0];
        self.cores = pcts[1..].to_vec();
        self.prev = now;
    }

    pub fn refresh(&mut self) {
        if let Ok(raw) = fs::read_to_string("/proc/stat") {
            self.apply(parse_stat(&raw));
        }
    }

    pub fn core_count(&self) -> usize {
        self.cores.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT_A: &str = include_str!("../../tests/fixtures/proc_stat_a");
    const STAT_B: &str = include_str!("../../tests/fixtures/proc_stat_b");

    #[test]
    fn parse_stat_corta_en_la_primera_linea_no_cpu() {
        let ticks = parse_stat(STAT_A);
        // agregado + 3 núcleos, sin `intr` ni `ctxt`
        assert_eq!(ticks.len(), 4);
    }

    #[test]
    fn parse_stat_calcula_active_y_total() {
        let ticks = parse_stat(STAT_A);
        // user+nice+system+irq+softirq
        assert_eq!(ticks[0].active, 86_355_484);
        // active + idle + iowait
        assert_eq!(ticks[0].total, 483_241_463);
    }

    #[test]
    fn primera_muestra_da_cero() {
        let mut cpu = Cpu::default();
        cpu.apply(parse_stat(STAT_A));
        assert_eq!(cpu.total, 0.0);
        assert_eq!(cpu.cores, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn segunda_muestra_calcula_el_delta() {
        let mut cpu = Cpu::default();
        cpu.apply(parse_stat(STAT_A));
        cpu.apply(parse_stat(STAT_B));
        assert_eq!(cpu.total, 25.0);
        assert_eq!(cpu.cores, vec![50.0, 0.0, 100.0]);
    }

    #[test]
    fn contador_que_no_avanza_da_cero() {
        let mut cpu = Cpu::default();
        cpu.apply(parse_stat(STAT_A));
        cpu.apply(parse_stat(STAT_A));
        assert_eq!(cpu.total, 0.0);
    }
}
