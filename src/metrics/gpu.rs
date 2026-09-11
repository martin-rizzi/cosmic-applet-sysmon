// SPDX-License-Identifier: GPL-3.0-only

//! GPU: sysfs de amdgpu (barato, en cada tick) y `nvidia-smi` (el único fork del
//! muestreo, sólo con el panel de GPU a la vista, spec §3.5). Intel no expone
//! `gpu_busy_percent`: en una máquina sólo-Intel no hay dispositivos y la sección se
//! oculta.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::history::History;
use super::read_trimmed;

/// Donde lo dejan Fedora (hwdata) y Debian (pciutils).
const PCI_IDS: [&str; 2] = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"];

const NVIDIA_GPU_QUERY: [&str; 2] = [
    "--query-gpu=name,utilization.gpu,clocks.current.graphics,temperature.gpu,memory.used,memory.total",
    "--format=csv,noheader,nounits",
];
const NVIDIA_APPS_QUERY: [&str; 2] = [
    "--query-compute-apps=pid,process_name,used_memory",
    "--format=csv,noheader,nounits",
];

#[derive(Clone, Debug, PartialEq)]
pub struct GpuDevice {
    pub id: String,
    pub name: String,
    /// 0–100.
    pub usage: f32,
    /// 0 si el driver no lo expone.
    pub clock_mhz: f64,
    /// °C; 0 sin sensor.
    pub temperature: f32,
    pub memory_used_mib: f64,
    pub memory_total_mib: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VramProcess {
    pub pid: u32,
    pub name: String,
    pub mib: f64,
}

fn parse_hex_id(raw: &str) -> Option<u16> {
    u16::from_str_radix(raw.trim().trim_start_matches("0x"), 16).ok()
}

/// "Fabricante Dispositivo" desde pci.ids, que es de donde lo saca `lspci`.
pub fn pci_name(pci_ids: &str, vendor: u16, device: u16) -> Option<String> {
    let vendor_hex = format!("{vendor:04x}");
    let device_hex = format!("{device:04x}");
    let mut vendor_name: Option<&str> = None;
    for line in pci_ids.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match vendor_name {
            None => {
                if let Some(rest) = line.strip_prefix(vendor_hex.as_str()) {
                    if rest.starts_with("  ") {
                        vendor_name = Some(rest.trim());
                    }
                }
            }
            Some(v) => {
                // Dos tabs: subsistema, no dispositivo.
                if line.starts_with("\t\t") {
                    continue;
                }
                // Sin tab: empezó el fabricante siguiente.
                let entry = line.strip_prefix('\t')?;
                if let Some(rest) = entry.strip_prefix(device_hex.as_str()) {
                    if rest.starts_with("  ") {
                        return Some(format!("{v} {}", rest.trim()));
                    }
                }
            }
        }
    }
    None
}

/// Reloj del renglón marcado con `*` en `pp_dpm_sclk`: `1: 1200Mhz *` → 1200.
pub fn parse_pp_dpm_sclk(raw: &str) -> f64 {
    raw.lines()
        .find(|l| l.contains('*'))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.replace('*', "").trim().to_ascii_lowercase())
        .and_then(|v| v.trim_end_matches("mhz").trim().parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// `cardN/device`, en orden numérico. Los conectores (`card0-DP-1`) no son placas.
fn card_dirs(drm: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(drm) else {
        return Vec::new();
    };
    let mut cards: Vec<(u32, String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let n = name.strip_prefix("card")?.parse::<u32>().ok()?;
            Some((n, name, e.path().join("device")))
        })
        .collect();
    cards.sort_by_key(|(n, _, _)| *n);
    cards.into_iter().map(|(_, name, dev)| (name, dev)).collect()
}

fn first_hwmon_temp(hwmon: &Path) -> Option<f32> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(hwmon).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.iter()
        .find_map(|d| read_trimmed(&d.join("temp1_input"))?.parse::<i64>().ok())
        .map(|milli| (milli / 1000) as f32)
}

/// Lo que junta `_sysfsGpuCommand`, una placa por `gpu_busy_percent`. `names` cachea el
/// nombre por id: pci.ids pesa 1,6 MB y no tiene sentido releerlo en cada tick.
pub fn read_sysfs_gpus(
    drm: &Path,
    names: &mut HashMap<String, String>,
    pci_ids: Option<&Path>,
) -> Vec<GpuDevice> {
    let mut out = Vec::new();
    for (card, dev) in card_dirs(drm) {
        let Some(busy) = read_trimmed(&dev.join("gpu_busy_percent")).and_then(|v| v.parse::<f32>().ok())
        else {
            continue;
        };
        // El slot PCI si `device` es el symlink real de sysfs; si no, el nombre de la placa.
        let id = fs::canonicalize(&dev)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .filter(|n| n.contains(':'))
            .unwrap_or_else(|| card.clone());
        let name = names
            .entry(id.clone())
            .or_insert_with(|| {
                let vendor = read_trimmed(&dev.join("vendor")).and_then(|v| parse_hex_id(&v));
                let device = read_trimmed(&dev.join("device")).and_then(|v| parse_hex_id(&v));
                let ids = pci_ids.and_then(|p| fs::read_to_string(p).ok());
                match (vendor, device, ids) {
                    (Some(v), Some(d), Some(ids)) => pci_name(&ids, v, d).unwrap_or_else(|| card.clone()),
                    _ => card.clone(),
                }
            })
            .clone();
        let clock_mhz = read_trimmed(&dev.join("gt_cur_freq_mhz"))
            .and_then(|v| v.parse::<f64>().ok())
            .or_else(|| fs::read_to_string(dev.join("pp_dpm_sclk")).ok().map(|raw| parse_pp_dpm_sclk(&raw)))
            .unwrap_or(0.0);
        let mib = |file: &str| {
            read_trimmed(&dev.join(file))
                .and_then(|v| v.parse::<f64>().ok())
                .map(|bytes| bytes / 1_048_576.0)
                .unwrap_or(0.0)
        };
        out.push(GpuDevice {
            id,
            name,
            usage: busy.clamp(0.0, 100.0),
            clock_mhz,
            temperature: first_hwmon_temp(&dev.join("hwmon")).unwrap_or(0.0),
            memory_used_mib: mib("mem_info_vram_used"),
            memory_total_mib: mib("mem_info_vram_total"),
        });
    }
    out
}

fn csv_fields(line: &str) -> Vec<&str> {
    line.split(',').map(str::trim).collect()
}

/// `nvidia-smi --query-gpu=…`. Un valor `[N/A]` cuenta como 0, igual que `parseFloat || 0`.
pub fn parse_nvidia_smi(raw: &str) -> Vec<GpuDevice> {
    let num = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    raw.lines()
        .map(csv_fields)
        .filter(|f| f.len() >= 6)
        .enumerate()
        .map(|(i, f)| GpuDevice {
            id: format!("nvidia{i}"),
            name: f[0].to_string(),
            usage: (num(f[1]) as f32).clamp(0.0, 100.0),
            clock_mhz: num(f[2]),
            temperature: num(f[3]) as f32,
            memory_used_mib: num(f[4]),
            memory_total_mib: num(f[5]),
        })
        .collect()
}

/// `nvidia-smi --query-compute-apps=…`.
pub fn parse_nvidia_apps(raw: &str) -> Vec<VramProcess> {
    raw.lines()
        .map(csv_fields)
        .filter(|f| f.len() >= 3)
        .filter_map(|f| {
            let pid = f[0].parse::<u32>().ok().filter(|&p| p > 0)?;
            if f[1].is_empty() {
                return None;
            }
            Some(VramProcess {
                pid,
                name: f[1].to_string(),
                mib: f[2].parse().unwrap_or(0.0),
            })
        })
        .collect()
}

/// NVIDIA primero y después las de sysfs que no repitan nombre ni id: `syncGpuDevices`.
pub fn merge_devices(nvidia: &[GpuDevice], sysfs: &[GpuDevice]) -> Vec<GpuDevice> {
    let mut out = nvidia.to_vec();
    for d in sysfs {
        if !out.iter().any(|m| m.name == d.name || m.id == d.id) {
            out.push(d.clone());
        }
    }
    out
}

fn run_nvidia_smi(args: &[&str]) -> Option<String> {
    let out = Command::new("nvidia-smi").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub struct Gpu {
    /// Se mira una sola vez, al arrancar.
    has_nvidia_smi: bool,
    pci_ids: Option<PathBuf>,
    names: HashMap<String, String>,
    sysfs: Vec<GpuDevice>,
    nvidia: Vec<GpuDevice>,
    /// Última lectura de procesos de `nvidia-smi`; la consume `Procs::refresh_vram`.
    pub nvidia_processes: Vec<VramProcess>,
    histories: HashMap<String, History>,
}

impl Default for Gpu {
    fn default() -> Self {
        Self {
            has_nvidia_smi: crate::exec::executable_in_path("nvidia-smi"),
            pci_ids: PCI_IDS.iter().map(PathBuf::from).find(|p| p.is_file()),
            names: HashMap::new(),
            sysfs: Vec::new(),
            nvidia: Vec::new(),
            nvidia_processes: Vec::new(),
            histories: HashMap::new(),
        }
    }
}

impl Gpu {
    /// Barato, en cada tick.
    pub fn refresh_sysfs(&mut self) {
        self.sysfs = read_sysfs_gpus(Path::new("/sys/class/drm"), &mut self.names, self.pci_ids.as_deref());
        for d in &self.sysfs {
            self.histories.entry(d.id.clone()).or_default().push(d.usage / 100.0);
        }
    }

    /// Caro: dos llamadas a `nvidia-smi`. Sólo con el panel de GPU a la vista.
    pub fn refresh_nvidia(&mut self) {
        if !self.has_nvidia_smi {
            return;
        }
        if let Some(raw) = run_nvidia_smi(&NVIDIA_GPU_QUERY) {
            self.nvidia = parse_nvidia_smi(&raw);
            for d in &self.nvidia {
                self.histories.entry(d.id.clone()).or_default().push(d.usage / 100.0);
            }
        }
        if let Some(raw) = run_nvidia_smi(&NVIDIA_APPS_QUERY) {
            self.nvidia_processes = parse_nvidia_apps(&raw);
        }
    }

    pub fn devices(&self) -> Vec<GpuDevice> {
        merge_devices(&self.nvidia, &self.sysfs)
    }

    /// La de más uso: es la que resume la sección en el panel.
    pub fn summary(&self) -> Option<GpuDevice> {
        self.devices().into_iter().max_by(|a, b| a.usage.total_cmp(&b.usage))
    }

    /// Sin ninguna fuente de datos la sección se oculta (spec §3.5).
    pub fn available(&self) -> bool {
        self.has_nvidia_smi || !self.sysfs.is_empty()
    }

    pub fn history(&self, id: &str) -> Option<&History> {
        self.histories.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn ids() -> String {
        fs::read_to_string(fixtures().join("pci.ids")).unwrap()
    }

    fn device(id: &str, name: &str, usage: f32) -> GpuDevice {
        GpuDevice {
            id: id.to_string(),
            name: name.to_string(),
            usage,
            clock_mhz: 0.0,
            temperature: 0.0,
            memory_used_mib: 0.0,
            memory_total_mib: 0.0,
        }
    }

    #[test]
    fn pci_name_arma_fabricante_y_dispositivo() {
        assert_eq!(
            pci_name(&ids(), 0x1002, 0x73ff).as_deref(),
            Some("Advanced Micro Devices, Inc. [AMD/ATI] Navi 23 [Radeon RX 6600/6600 XT/6600M]")
        );
        assert_eq!(
            pci_name(&ids(), 0x8086, 0x9bc8).as_deref(),
            Some("Intel Corporation CometLake-S GT2 [UHD Graphics 630]")
        );
    }

    #[test]
    fn pci_name_no_confunde_subsistemas_ni_inventa() {
        assert_eq!(pci_name(&ids(), 0x1002, 0x1458), None);
        assert_eq!(pci_name(&ids(), 0x10de, 0x2503), None);
    }

    #[test]
    fn reloj_de_pp_dpm_sclk() {
        assert_eq!(parse_pp_dpm_sclk("0: 500Mhz\n1: 1200Mhz *\n2: 1600Mhz\n"), 1200.0);
        assert_eq!(parse_pp_dpm_sclk("0: 500Mhz\n"), 0.0);
    }

    #[test]
    fn lee_solo_las_placas_con_gpu_busy_percent() {
        let mut names = HashMap::new();
        let ids_path = fixtures().join("pci.ids");
        let gpus = read_sysfs_gpus(&fixtures().join("drm"), &mut names, Some(ids_path.as_path()));
        assert_eq!(
            gpus,
            vec![GpuDevice {
                id: "card0".to_string(),
                name: "Advanced Micro Devices, Inc. [AMD/ATI] Navi 23 [Radeon RX 6600/6600 XT/6600M]".to_string(),
                usage: 37.0,
                clock_mhz: 1200.0,
                temperature: 54.0,
                memory_used_mib: 2048.0,
                memory_total_mib: 8176.0,
            }]
        );
        // El nombre queda cacheado: pci.ids no se relee en cada tick.
        assert!(names.contains_key("card0"));
    }

    #[test]
    fn parsea_nvidia_smi_y_tolera_na() {
        let raw = "NVIDIA GeForce RTX 3060, 12, 810, 48, 1024, 12288\nNVIDIA T400, [N/A], [N/A], 40, 100, 2048\nbasura\n";
        let gpus = parse_nvidia_smi(raw);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].id, "nvidia0");
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 3060");
        assert_eq!((gpus[0].usage, gpus[0].clock_mhz, gpus[0].temperature), (12.0, 810.0, 48.0));
        assert_eq!((gpus[0].memory_used_mib, gpus[0].memory_total_mib), (1024.0, 12288.0));
        assert_eq!((gpus[1].id.as_str(), gpus[1].usage), ("nvidia1", 0.0));
    }

    #[test]
    fn parsea_procesos_de_nvidia() {
        let raw = "4321, /usr/bin/steam, 512\n0, fantasma, 10\n77, , 5\n";
        assert_eq!(
            parse_nvidia_apps(raw),
            vec![VramProcess { pid: 4321, name: "/usr/bin/steam".to_string(), mib: 512.0 }]
        );
    }

    #[test]
    fn nvidia_primero_y_sin_duplicados() {
        let nvidia = vec![device("nvidia0", "NVIDIA RTX", 10.0)];
        let sysfs = vec![device("card0", "AMD", 50.0), device("card1", "NVIDIA RTX", 5.0)];
        let merged = merge_devices(&nvidia, &sysfs);
        let ids: Vec<&str> = merged.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["nvidia0", "card0"]);
    }
}
