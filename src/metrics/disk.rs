// SPDX-License-Identifier: GPL-3.0-only

//! Disco: E/S desde /proc/diskstats, uso por punto de montaje con `statvfs` (en vez de
//! `df`) e inventario desde /sys/block (en vez de `lsblk`).

use std::fs;
use std::path::Path;

use super::rate::per_second;
use super::read_trimmed;

/// Mismo criterio que `isWholeDiskDevice` del plasmoid:
/// `^(sd[a-z]+|vd[a-z]+|xvd[a-z]+|hd[a-z]+|nvme\d+n\d+|mmcblk\d+|md\d+)$`.
pub fn is_whole_disk(name: &str) -> bool {
    fn letters(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase())
    }
    fn digits(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
    if let Some(rest) = name.strip_prefix("xvd") {
        return letters(rest);
    }
    for prefix in ["sd", "vd", "hd"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return letters(rest);
        }
    }
    if let Some(rest) = name.strip_prefix("nvme") {
        return rest
            .split_once('n')
            .is_some_and(|(controller, ns)| digits(controller) && digits(ns));
    }
    if let Some(rest) = name.strip_prefix("mmcblk") {
        return digits(rest);
    }
    if let Some(rest) = name.strip_prefix("md") {
        return digits(rest);
    }
    false
}

/// (bytes leídos, bytes escritos) acumulados de los discos enteros. Columnas 6 y 10 de
/// diskstats, en sectores de 512 bytes.
pub fn parse_diskstats(raw: &str) -> (u64, u64) {
    let (mut read, mut write) = (0u64, 0u64);
    for line in raw.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 || !is_whole_disk(f[2]) {
            continue;
        }
        read += f[5].parse::<u64>().unwrap_or(0) * 512;
        write += f[9].parse::<u64>().unwrap_or(0) * 512;
    }
    (read, write)
}

#[derive(Default)]
pub struct DiskIo {
    prev: Option<(u64, u64)>,
    pub read_rate: f64,
    pub write_rate: f64,
}

impl DiskIo {
    pub fn apply(&mut self, sample: (u64, u64), elapsed_secs: f64) {
        (self.read_rate, self.write_rate) = per_second(self.prev, sample, elapsed_secs);
        self.prev = Some(sample);
    }

    pub fn refresh(&mut self, elapsed_secs: f64) {
        if let Ok(raw) = fs::read_to_string("/proc/diskstats") {
            self.apply(parse_diskstats(&raw), elapsed_secs);
        }
    }
}

/// Escala fija del gráfico de E/S: `axisScale` de `StorageDetail.qml`, 10 MB/s por mitad.
pub const GRAPH_SCALE: f64 = 1.04858e7;

pub fn graph_fraction(rate: f64) -> f32 {
    (rate / GRAPH_SCALE).clamp(0.0, 1.0) as f32
}

#[derive(Clone, Debug, PartialEq)]
pub struct MountUsage {
    pub device: String,
    pub mount: String,
    /// Todo en bytes.
    pub size: u64,
    pub used: u64,
    pub avail: u64,
    /// Como la columna `Uso%` de df.
    pub percent: u32,
}

/// /proc/self/mounts escapa espacio, tab, salto y barra invertida como `\ooo`.
pub fn unescape_mount(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let octal = b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..=i + 3].iter().all(|c| (b'0'..=b'7').contains(c));
        if octal {
            let v = u32::from(b[i + 1] - b'0') * 64
                + u32::from(b[i + 2] - b'0') * 8
                + u32::from(b[i + 3] - b'0');
            out.push(v as u8);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// El filtro de `parseDf`: raíz y lo que cuelgue de /home, /mnt, /media o /run/media.
fn wanted_mount(mount: &str) -> bool {
    mount == "/"
        || ["/home", "/mnt", "/media", "/run/media"]
            .iter()
            .any(|p| mount.starts_with(p))
}

/// (dispositivo, punto de montaje) de los dispositivos de bloque que mostraría
/// `df | grep '^/dev'`, en el orden del archivo. Como df, no deduplica: dos subvolúmenes
/// btrfs del mismo disco aparecen los dos.
pub fn parse_mounts(raw: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in raw.lines() {
        let mut f = line.split_whitespace();
        let (Some(device), Some(mount)) = (f.next(), f.next()) else {
            continue;
        };
        if !device.starts_with("/dev/") {
            continue;
        }
        let entry = (device.to_string(), unescape_mount(mount));
        if wanted_mount(&entry.1) && !out.contains(&entry) {
            out.push(entry);
        }
    }
    out
}

/// (tamaño, usado, disponible, porcentaje) como df: el porcentaje es usado sobre usado +
/// disponible para el usuario, redondeado para arriba.
pub fn usage_from(blocks: u64, bfree: u64, bavail: u64, frsize: u64) -> (u64, u64, u64, u32) {
    let size = blocks * frsize;
    let used = blocks.saturating_sub(bfree) * frsize;
    let avail = bavail * frsize;
    let denom = u128::from(used) + u128::from(avail);
    let percent = if denom == 0 {
        0
    } else {
        (u128::from(used) * 100).div_ceil(denom) as u32
    };
    (size, used, avail, percent)
}

fn read_usage(device: &str, mount: &str) -> Option<MountUsage> {
    let st = rustix::fs::statvfs(mount).ok()?;
    let (size, used, avail, percent) = usage_from(st.f_blocks, st.f_bfree, st.f_bavail, st.f_frsize);
    Some(MountUsage {
        device: device.to_string(),
        mount: mount.to_string(),
        size,
        used,
        avail,
        percent,
    })
}

fn read_mounts() -> Vec<(String, String)> {
    fs::read_to_string("/proc/self/mounts")
        .map(|raw| parse_mounts(&raw))
        .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockDevice {
    /// Modelo, o el nombre del kernel si no tiene.
    pub name: String,
    pub path: String,
    /// Bytes.
    pub size: u64,
    /// "SATA", "USB • Extraíble", "Disco"…
    pub detail: String,
    pub usage: Option<MountUsage>,
}

/// La columna TRAN de lsblk, deducida de la ruta real del dispositivo en sysfs. USB va
/// antes que ATA porque los puentes USB-SATA pasan por las dos.
pub fn transport(resolved: &str, name: &str) -> Option<&'static str> {
    if name.starts_with("nvme") {
        Some("NVME")
    } else if name.starts_with("mmcblk") {
        Some("MMC")
    } else if resolved.contains("/usb") {
        Some("USB")
    } else if resolved.contains("/virtio") {
        Some("VIRTIO")
    } else if resolved.contains("/ata") {
        Some("SATA")
    } else {
        None
    }
}

/// Uso del disco entero o de su primera partición con uso, en orden de partición: lo que
/// hace `usageForNode` recorriendo los hijos de lsblk.
pub fn usage_for<'a>(disk: &str, partitions: &[String], usages: &'a [MountUsage]) -> Option<&'a MountUsage> {
    std::iter::once(disk)
        .chain(partitions.iter().map(String::as_str))
        .find_map(|dev| {
            let path = format!("/dev/{dev}");
            usages.iter().find(|u| u.device == path)
        })
}

fn read_block_devices(sys_block: &Path, usages: &[MountUsage]) -> Vec<BlockDevice> {
    let Ok(entries) = fs::read_dir(sys_block) else {
        return Vec::new();
    };
    // lsblk reporta los md como TYPE raid*, no disk: el plasmoid no los lista.
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_whole_disk(n) && !n.starts_with("md"))
        .collect();
    names.sort();

    names
        .into_iter()
        .map(|name| {
            let dir = sys_block.join(&name);
            let resolved = fs::canonicalize(&dir)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let size = read_trimmed(&dir.join("size"))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
                * 512;
            let model = read_trimmed(&dir.join("device/model")).unwrap_or_default();
            let removable = read_trimmed(&dir.join("removable")).as_deref() == Some("1");
            let mut partitions: Vec<String> = fs::read_dir(&dir)
                .map(|it| {
                    it.flatten()
                        .filter_map(|e| e.file_name().into_string().ok())
                        .filter(|p| p.starts_with(&name) && *p != name)
                        .collect()
                })
                .unwrap_or_default();
            // sda2 antes que sda10.
            partitions.sort_by_key(|p| (p.len(), p.clone()));
            let mut detail = transport(&resolved, &name).unwrap_or("Disco").to_string();
            if removable {
                detail.push_str(" • Extraíble");
            }
            BlockDevice {
                name: if model.is_empty() { name.clone() } else { model },
                path: format!("/dev/{name}"),
                size,
                detail,
                usage: usage_for(&name, &partitions, usages).cloned(),
            }
        })
        .collect()
}

#[derive(Default)]
pub struct Storage {
    pub io: DiskIo,
    /// En el orden de /proc/self/mounts. Con el panel de disco cerrado trae sólo el
    /// primero, que es el que muestra la vista compacta.
    pub mounts: Vec<MountUsage>,
    /// Vacío mientras el panel de disco no está a la vista.
    pub devices: Vec<BlockDevice>,
}

impl Storage {
    /// Barato: un solo `statvfs`, el del primer punto de montaje.
    pub fn refresh_primary(&mut self) {
        self.mounts = read_mounts()
            .into_iter()
            .take(1)
            .filter_map(|(d, m)| read_usage(&d, &m))
            .collect();
        self.devices.clear();
    }

    /// Caro: todos los puntos de montaje y el inventario de /sys/block.
    pub fn refresh_full(&mut self) {
        self.mounts = read_mounts()
            .into_iter()
            .filter_map(|(d, m)| read_usage(&d, &m))
            .collect();
        self.devices = read_block_devices(Path::new("/sys/block"), &self.mounts);
    }

    pub fn primary(&self) -> Option<&MountUsage> {
        self.mounts.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISKSTATS: &str = include_str!("../../tests/fixtures/proc_diskstats");
    const MOUNTS: &str = include_str!("../../tests/fixtures/proc_mounts");

    #[test]
    fn reconoce_discos_enteros_como_el_regex_del_qml() {
        for name in ["sda", "sdab", "vda", "xvda", "hdc", "nvme0n1", "nvme12n3", "mmcblk0", "md127"] {
            assert!(is_whole_disk(name), "{name} es disco entero");
        }
    }

    #[test]
    fn descarta_particiones_y_dispositivos_virtuales() {
        for name in ["sda1", "nvme0n1p1", "mmcblk0p1", "dm-0", "zram0", "loop0", "sr0", "sd", "nvme0"] {
            assert!(!is_whole_disk(name), "{name} no es disco entero");
        }
    }

    #[test]
    fn suma_sectores_de_discos_enteros_en_bytes() {
        // sda + nvme0n1, sectores de 512 bytes.
        assert_eq!(parse_diskstats(DISKSTATS), (6_299_484_160, 3_117_011_968));
    }

    #[test]
    fn tasa_de_lectura_y_escritura() {
        let mut io = DiskIo::default();
        io.apply((0, 0), 0.0);
        io.apply((10_485_760, 5_242_880), 2.0);
        assert_eq!(io.read_rate, 5_242_880.0);
        assert_eq!(io.write_rate, 2_621_440.0);
    }

    #[test]
    fn el_grafico_satura_en_diez_megas() {
        assert_eq!(graph_fraction(0.0), 0.0);
        assert!((graph_fraction(5.24290e6) - 0.5).abs() < 0.001);
        assert_eq!(graph_fraction(1.0e9), 1.0);
    }

    #[test]
    fn des_escapa_los_octales_de_mounts() {
        assert_eq!(unescape_mount("/run/media/tincho/USB\\040DISK"), "/run/media/tincho/USB DISK");
        assert_eq!(unescape_mount("/mnt/a\\134b"), "/mnt/a\\b");
        assert_eq!(unescape_mount("/sin/escapes"), "/sin/escapes");
    }

    #[test]
    fn filtra_montajes_como_el_parse_df_del_qml() {
        assert_eq!(
            parse_mounts(MOUNTS),
            vec![
                ("/dev/sda3".to_string(), "/".to_string()),
                ("/dev/sda3".to_string(), "/home".to_string()),
                ("/dev/sdb1".to_string(), "/run/media/tincho/USB DISK".to_string()),
                ("/dev/nvme0n1p2".to_string(), "/mnt/datos".to_string()),
            ]
        );
    }

    #[test]
    fn uso_como_df() {
        // 600 bloques usados de 900 disponibles para el usuario: df redondea para arriba.
        assert_eq!(usage_from(1000, 400, 300, 4096), (4_096_000, 2_457_600, 1_228_800, 67));
        assert_eq!(usage_from(0, 0, 0, 4096), (0, 0, 0, 0));
    }

    #[test]
    fn transporte_desde_la_ruta_de_sysfs() {
        let ata = "/sys/devices/pci0000:00/0000:00:17.0/ata6/host5/target5:0:0/5:0:0:0/block/sda";
        let usb = "/sys/devices/pci0000:00/0000:00:14.0/usb2/2-1/2-1:1.0/host6/target6:0:0/6:0:0:0/block/sdb";
        let virtio = "/sys/devices/pci0000:00/0000:00:04.0/virtio1/block/vda";
        assert_eq!(transport(ata, "sda"), Some("SATA"));
        assert_eq!(transport(usb, "sdb"), Some("USB"));
        assert_eq!(transport(virtio, "vda"), Some("VIRTIO"));
        assert_eq!(transport("/sys/devices/pci0000:00/0000:01:00.0/nvme/nvme0/nvme0n1", "nvme0n1"), Some("NVME"));
        assert_eq!(transport("/sys/devices/virtual/block/xyz", "xyz"), None);
    }

    #[test]
    fn uso_del_disco_sale_de_la_primera_particion_montada() {
        let u = |device: &str, mount: &str| MountUsage {
            device: device.to_string(),
            mount: mount.to_string(),
            size: 1,
            used: 1,
            avail: 0,
            percent: 100,
        };
        let usages = vec![u("/dev/sda3", "/"), u("/dev/sda3", "/home"), u("/dev/nvme0n1p2", "/mnt/datos")];
        let parts = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(usage_for("sda", &parts(&["sda1", "sda2", "sda3"]), &usages).map(|x| x.mount.as_str()), Some("/"));
        assert_eq!(usage_for("nvme0n1", &parts(&["nvme0n1p1", "nvme0n1p2"]), &usages).map(|x| x.mount.as_str()), Some("/mnt/datos"));
        assert_eq!(usage_for("sdb", &parts(&["sdb1"]), &usages), None);
    }
}
