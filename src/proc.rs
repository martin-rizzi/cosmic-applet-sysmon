//! Muestreo de /proc/stat y /proc/meminfo.
//!
//! Las fórmulas replican las del plasmoid com.labatata.sysmonitor:
//!   - CPU: active = user+nice+system+irq+softirq, total = active+idle+iowait,
//!     y el porcentaje sale del delta contra la muestra anterior.
//!   - RAM: usada = MemTotal - (MemFree + Buffers + Cached + SReclaimable - Shmem).

use std::fs;

#[derive(Clone, Copy, Default)]
struct CpuTicks {
    total: u64,
    active: u64,
}

#[derive(Default)]
pub struct Cpu {
    /// Muestra anterior: índice 0 es el agregado "cpu", el resto son los núcleos.
    prev: Vec<CpuTicks>,
    /// Uso porcentual por núcleo, en el orden en que aparecen en /proc/stat.
    pub cores: Vec<f32>,
    /// Uso porcentual agregado.
    pub total: f32,
}

impl Cpu {
    pub fn refresh(&mut self) {
        let Ok(stat) = fs::read_to_string("/proc/stat") else {
            return;
        };

        let mut now: Vec<CpuTicks> = Vec::with_capacity(self.prev.len().max(8));
        for line in stat.lines() {
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
            now.push(CpuTicks {
                total: active + idle + iowait,
                active,
            });
        }

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
                // Primera muestra o contador reiniciado: 0, como hace el plasmoid.
                _ => 0.0,
            };
            pcts.push(pct);
        }

        self.total = pcts[0];
        self.cores = pcts[1..].to_vec();
        self.prev = now;
    }

    pub fn core_count(&self) -> usize {
        self.cores.len()
    }
}

/// Todos los valores en MiB.
#[derive(Default)]
pub struct Mem {
    pub total: f64,
    pub used: f64,
    pub cached: f64,
    pub swap_total: f64,
    pub swap_used: f64,
}

impl Mem {
    pub fn refresh(&mut self) {
        let Ok(raw) = fs::read_to_string("/proc/meminfo") else {
            return;
        };

        let field = |name: &str| -> f64 {
            raw.lines()
                .find(|l| l.starts_with(name) && l.as_bytes().get(name.len()) == Some(&b':'))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
                / 1024.0
        };

        self.total = field("MemTotal");
        self.cached = field("Cached") + field("SReclaimable") - field("Shmem");
        let free = field("MemFree") + field("Buffers") + self.cached;
        self.used = self.total - free;
        self.swap_total = field("SwapTotal");
        self.swap_used = self.swap_total - field("SwapFree");
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

/// "12,3 GiB" / "512 MiB", como el `formatMemoryMib` del plasmoid.
pub fn format_mib(mib: f64) -> String {
    if mib >= 1024.0 {
        format!("{:.1} GiB", mib / 1024.0)
    } else {
        format!("{:.0} MiB", mib)
    }
}
