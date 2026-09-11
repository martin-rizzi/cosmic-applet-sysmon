// SPDX-License-Identifier: GPL-3.0-only

//! Temperaturas desde /sys/class/hwmon, que es lo que lee `sensors` por debajo.
//!
//! El rótulo es `chip/sensor` como en `parseSensors` del plasmoid, con una diferencia:
//! `sensors` le agrega al chip el bus ("coretemp-isa-0000") y hwmon expone sólo el
//! nombre ("coretemp").

use std::fs;
use std::path::{Path, PathBuf};

use super::read_trimmed;

#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub label: String,
    pub celsius: f32,
}

/// El número entre un prefijo y un sufijo: (`hwmon12`, "hwmon", "") → 12.
fn index_between(name: &str, prefix: &str, suffix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.strip_suffix(suffix)?.parse().ok()
}

/// Entradas de `dir` cuyo nombre es `prefix<N>suffix`, ordenadas por N.
fn numbered(dir: &Path, prefix: &str, suffix: &str) -> Vec<(u32, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(u32, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let n = index_between(e.file_name().to_str()?, prefix, suffix)?;
            Some((n, e.path()))
        })
        .collect();
    out.sort_by_key(|(n, _)| *n);
    out
}

pub fn read_hwmon(root: &Path) -> Vec<Reading> {
    let mut out = Vec::new();
    for (_, chip) in numbered(root, "hwmon", "") {
        let chip_name = read_trimmed(&chip.join("name")).unwrap_or_else(|| "hwmon".into());
        for (i, input) in numbered(&chip, "temp", "_input") {
            let Some(milli) = read_trimmed(&input).and_then(|v| v.parse::<i64>().ok()) else {
                continue;
            };
            let sensor = read_trimmed(&chip.join(format!("temp{i}_label")))
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| format!("temp{i}"));
            out.push(Reading {
                label: format!("{chip_name}/{sensor}"),
                celsius: milli as f32 / 1000.0,
            });
        }
    }
    out
}

/// Umbrales de `TempDetail.qml` y `CompactView.qml`: más de 90 °C rojo, más de 75 °C
/// naranja; si no, el color normal del texto.
pub fn severity_color(celsius: f32) -> Option<&'static str> {
    if celsius > 90.0 {
        Some(crate::draw::CRITICAL)
    } else if celsius > 75.0 {
        Some(crate::draw::WARNING)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hwmon")
    }

    #[test]
    fn recorre_chips_y_sensores_en_orden_numerico() {
        let r = read_hwmon(&fixture());
        let labels: Vec<&str> = r.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "acpitz/temp1",
                "pch_cometlake/temp1",
                "coretemp/Package id 0",
                "coretemp/Core 0",
                "coretemp/Core 9",
                "nvme/Composite",
            ]
        );
        let celsius: Vec<f32> = r.iter().map(|x| x.celsius).collect();
        for (got, want) in celsius.iter().zip([27.8, 41.0, 33.0, 31.0, 30.0, 38.85]) {
            assert!((got - want).abs() < 0.001, "{got} != {want}");
        }
    }

    #[test]
    fn sin_hwmon_no_hay_lecturas() {
        assert!(read_hwmon(&fixture().join("no-existe")).is_empty());
    }

    #[test]
    fn colores_por_umbral_como_el_qml() {
        assert_eq!(severity_color(90.1), Some(crate::draw::CRITICAL));
        assert_eq!(severity_color(90.0), Some(crate::draw::WARNING));
        assert_eq!(severity_color(75.1), Some(crate::draw::WARNING));
        assert_eq!(severity_color(75.0), None);
    }
}
