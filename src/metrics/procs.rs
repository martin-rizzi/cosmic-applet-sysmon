// SPDX-License-Identifier: GPL-3.0-only

//! Top de procesos por CPU, memoria y VRAM, leyendo /proc/N directo. Recorre cientos de
//! directorios, así que sólo corre con su panel a la vista (spec §3.1).
//!
//! La CPU es instantánea, por delta entre dos muestras, y no el promedio de vida del
//! proceso que da `ps -eo pcpu` en el plasmoid (spec §3.7).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::gpu::VramProcess;
use super::read_trimmed;

/// Cinco por lista, como `count == 5` y `head -5` en los comandos del plasmoid.
pub const TOP: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct ProcStat {
    pub pid: u32,
    pub name: String,
    /// utime + stime, en jiffies.
    pub ticks: u64,
    /// En páginas.
    pub rss_pages: u64,
}

/// Una línea de /proc/N/stat. El nombre va del primer `(` al **último** `)`: puede tener
/// espacios y paréntesis adentro.
pub fn parse_pid_stat(raw: &str) -> Option<ProcStat> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    let pid = raw[..open].trim().parse().ok()?;
    let name = raw.get(open + 1..close)?.to_string();
    let fields: Vec<&str> = raw[close + 1..].split_whitespace().collect();
    // fields[0] es el campo 3 (state): utime es el 14, stime el 15 y rss el 24.
    let num = |i: usize| fields.get(i).and_then(|v| v.parse::<u64>().ok());
    Some(ProcStat {
        pid,
        name,
        ticks: num(11)? + num(12)?,
        rss_pages: num(21)?,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcCpu {
    pub name: String,
    /// De **un** núcleo, como top y ps: puede pasar de 100.
    pub percent: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcMem {
    pub name: String,
    pub kib: f64,
    /// Sobre MemTotal, como la columna `pmem` de ps.
    pub percent: f32,
}

/// El tiempo por núcleo sale del delta del agregado de /proc/stat dividido por la
/// cantidad de núcleos. Los procesos sin muestra previa esperan al tick siguiente.
pub fn top_cpu(prev: &HashMap<u32, u64>, now: &[ProcStat], total_delta: u64, cores: usize) -> Vec<ProcCpu> {
    if total_delta == 0 || cores == 0 {
        return Vec::new();
    }
    let per_core = total_delta as f32 / cores as f32;
    let mut out: Vec<ProcCpu> = now
        .iter()
        .filter_map(|p| {
            let before = *prev.get(&p.pid)?;
            Some(ProcCpu {
                name: p.name.clone(),
                percent: p.ticks.saturating_sub(before) as f32 / per_core * 100.0,
            })
        })
        .collect();
    out.sort_by(|a, b| b.percent.total_cmp(&a.percent));
    out.truncate(TOP);
    out
}

pub fn top_mem(now: &[ProcStat], page_size: u64, mem_total_kib: f64) -> Vec<ProcMem> {
    let mut out: Vec<ProcMem> = now
        .iter()
        .filter(|p| p.rss_pages > 0)
        .map(|p| {
            let kib = (p.rss_pages * page_size) as f64 / 1024.0;
            let percent = if mem_total_kib > 0.0 {
                (kib / mem_total_kib * 100.0) as f32
            } else {
                0.0
            };
            ProcMem { name: p.name.clone(), kib, percent }
        })
        .collect();
    out.sort_by(|a, b| b.kib.total_cmp(&a.kib));
    out.truncate(TOP);
    out
}

/// `drm-memory-vram` de los fdinfo de **un** proceso, en MiB, contando una sola vez cada
/// `drm-client-id` (varios fd pueden ser el mismo cliente). El awk de
/// `_drmGpuProcessesCommand`.
pub fn vram_from_fdinfo<'a>(files: impl IntoIterator<Item = &'a str>) -> f64 {
    let mut seen = HashSet::new();
    let mut total_kib = 0.0;
    for file in files {
        let mut client: Option<&str> = None;
        let mut vram = 0.0;
        for line in file.lines() {
            if let Some(v) = line.strip_prefix("drm-client-id:") {
                client = Some(v.trim());
            } else if let Some(v) = line.strip_prefix("drm-memory-vram:") {
                vram += v
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<f64>().ok())
                    .unwrap_or(0.0);
            }
        }
        if vram > 0.0 && client.map_or(true, |c| seen.insert(c.to_string())) {
            total_kib += vram;
        }
    }
    total_kib / 1024.0
}

/// Un proceso puede aparecer en nvidia-smi y en DRM: se queda el mayor, como
/// `syncGpuProcesses`.
pub fn merge_vram(a: &[VramProcess], b: &[VramProcess]) -> Vec<VramProcess> {
    let mut out: Vec<VramProcess> = Vec::new();
    for p in a.iter().chain(b) {
        match out.iter_mut().find(|q| q.pid == p.pid) {
            Some(q) => q.mib = q.mib.max(p.mib),
            None => out.push(p.clone()),
        }
    }
    out.sort_by(|x, y| y.mib.total_cmp(&x.mib));
    out.truncate(TOP);
    out
}

fn pids(proc_root: &Path) -> Vec<u32> {
    fs::read_dir(proc_root)
        .map(|it| it.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).collect())
        .unwrap_or_default()
}

fn scan(proc_root: &Path) -> Vec<ProcStat> {
    pids(proc_root)
        .into_iter()
        .filter_map(|pid| {
            let raw = fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
            parse_pid_stat(&raw)
        })
        .collect()
}

fn scan_vram(proc_root: &Path) -> Vec<VramProcess> {
    let mut out = Vec::new();
    for pid in pids(proc_root) {
        let dir = proc_root.join(pid.to_string());
        // Procesos de otros usuarios: sin permiso, se saltean.
        let Ok(fds) = fs::read_dir(dir.join("fdinfo")) else {
            continue;
        };
        let contents: Vec<String> = fds.flatten().filter_map(|e| fs::read_to_string(e.path()).ok()).collect();
        let mib = vram_from_fdinfo(contents.iter().map(String::as_str));
        if mib > 0.0 {
            let name = read_trimmed(&dir.join("comm")).unwrap_or_default();
            out.push(VramProcess { pid, name, mib });
        }
    }
    out
}

#[derive(Default)]
pub struct Procs {
    prev_ticks: HashMap<u32, u64>,
    prev_total: Option<u64>,
    pub cpu: Vec<ProcCpu>,
    pub mem: Vec<ProcMem>,
    pub vram: Vec<VramProcess>,
}

impl Procs {
    /// `total_jiffies` es el agregado actual de /proc/stat.
    pub fn refresh_cpu(&mut self, total_jiffies: u64, cores: usize) {
        let now = scan(Path::new("/proc"));
        if let Some(prev_total) = self.prev_total {
            self.cpu = top_cpu(&self.prev_ticks, &now, total_jiffies.saturating_sub(prev_total), cores);
        }
        self.prev_ticks = now.iter().map(|p| (p.pid, p.ticks)).collect();
        self.prev_total = Some(total_jiffies);
    }

    /// Con el panel de CPU cerrado se descarta la base: al reabrirlo, el primer delta
    /// sería el promedio de todo el rato que estuvo cerrado, no el uso de ahora.
    pub fn forget_cpu(&mut self) {
        self.prev_ticks.clear();
        self.prev_total = None;
        self.cpu.clear();
    }

    pub fn refresh_mem(&mut self, page_size: u64, mem_total_mib: f64) {
        self.mem = top_mem(&scan(Path::new("/proc")), page_size, mem_total_mib * 1024.0);
    }

    pub fn refresh_vram(&mut self, nvidia: &[VramProcess]) {
        self.vram = merge_vram(nvidia, &scan_vram(Path::new("/proc")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATS: &str = include_str!("../../tests/fixtures/proc_pid_stat");

    fn stat(pid: u32, name: &str, ticks: u64, rss_pages: u64) -> ProcStat {
        ProcStat { pid, name: name.to_string(), ticks, rss_pages }
    }

    #[test]
    fn parsea_utime_stime_y_rss() {
        let all: Vec<ProcStat> = STATS.lines().filter_map(parse_pid_stat).collect();
        assert_eq!(all[0], stat(11742, "cat", 10, 474));
        assert_eq!(all[1], stat(4321, "tmux: server", 1800, 2500));
    }

    #[test]
    fn el_nombre_llega_hasta_el_ultimo_parentesis() {
        let p = parse_pid_stat(STATS.lines().nth(2).unwrap()).unwrap();
        assert_eq!(p, stat(777, "a) b)", 15, 42));
        assert_eq!(parse_pid_stat("basura"), None);
    }

    #[test]
    fn cpu_instantanea_por_nucleo_y_sin_recien_nacidos() {
        let prev = HashMap::from([(1, 100), (2, 50)]);
        let now = vec![stat(1, "uno", 150, 0), stat(2, "dos", 50, 0), stat(3, "nuevo", 999, 0)];
        // 400 jiffies de agregado entre 4 núcleos: 100 por núcleo.
        assert_eq!(
            top_cpu(&prev, &now, 400, 4),
            vec![
                ProcCpu { name: "uno".to_string(), percent: 50.0 },
                ProcCpu { name: "dos".to_string(), percent: 0.0 },
            ]
        );
        assert!(top_cpu(&prev, &now, 0, 4).is_empty());
    }

    #[test]
    fn memoria_por_rss_con_porcentaje_y_tope_de_cinco() {
        let now: Vec<ProcStat> = (1..=7).map(|i| stat(i, &format!("p{i}"), 0, u64::from(i) * 250)).collect();
        let top = top_mem(&now, 4096, 1_000_000.0);
        assert_eq!(top.len(), TOP);
        assert_eq!(top[0], ProcMem { name: "p7".to_string(), kib: 7000.0, percent: 0.7 });
        assert_eq!(top[4].name, "p3");
        // Los hilos del kernel (RSS 0) no entran.
        assert!(top_mem(&[stat(9, "kworker", 0, 0)], 4096, 1_000_000.0).is_empty());
    }

    #[test]
    fn vram_cuenta_cada_cliente_drm_una_vez() {
        let files = [
            "drm-client-id:\t5\ndrm-memory-vram:\t204800 KiB\n",
            "drm-client-id:\t5\ndrm-memory-vram:\t204800 KiB\n",
            "drm-client-id:\t6\ndrm-memory-vram:\t102400 KiB\n",
            "pos:\t0\nflags:\t02\n",
            "drm-driver:\ti915\ndrm-client-id:\t16\ndrm-total-system0:\t259992 KiB\n",
        ];
        assert_eq!(vram_from_fdinfo(files), 300.0);
    }

    #[test]
    fn nvidia_y_drm_se_unen_por_pid_con_el_mayor() {
        let v = |pid: u32, mib: f64| VramProcess { pid, name: format!("p{pid}"), mib };
        let merged = merge_vram(&[v(1, 100.0), v(2, 50.0)], &[v(1, 300.0), v(3, 10.0)]);
        assert_eq!(merged, vec![v(1, 300.0), v(2, 50.0), v(3, 10.0)]);
    }
}
