// SPDX-License-Identifier: GPL-3.0-only

//! Formateadores de `main.qml` del plasmoid. Los cortes de unidad usan las mismas
//! constantes que el QML (`1.04858e+06`, `1.07374e+09`, `1.09951e+12`) en vez de potencias
//! exactas de 1024, para que cambien de unidad en el mismo valor.

const KIB: f64 = 1024.0;
const MIB: f64 = 1.04858e6;
const GIB: f64 = 1.07374e9;
const TIB: f64 = 1.09951e12;

/// `formatRate`.
pub fn rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < KIB {
        format!("{bytes_per_sec:.0} B/s")
    } else if bytes_per_sec < MIB {
        format!("{:.1} KB/s", bytes_per_sec / KIB)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / MIB)
    }
}

/// `formatCompactRate`: los bytes se rellenan a cuatro caracteres para que el panel no
/// cambie de ancho en cada tick.
pub fn compact_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < KIB {
        let value = format!("{bytes_per_sec:.0}");
        format!("{value:<4}B/s")
    } else if bytes_per_sec < MIB {
        format!("{:.0} KB/s", bytes_per_sec / KIB)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / MIB)
    }
}

/// `formatMemoryKib`.
pub fn memory_kib(kib: f64) -> String {
    if kib < KIB {
        format!("{kib:.0} KB")
    } else if kib < MIB {
        if kib < 10_240.0 {
            format!("{:.1} MB", kib / KIB)
        } else {
            format!("{:.0} MB", kib / KIB)
        }
    } else {
        format!("{:.1} GB", kib / MIB)
    }
}

/// `formatMemoryMib`.
pub fn memory_mib(mib: f64) -> String {
    if mib < KIB {
        if mib < 100.0 {
            format!("{mib:.1} MB")
        } else {
            format!("{mib:.0} MB")
        }
    } else {
        format!("{:.1} GB", mib / KIB)
    }
}

/// Renglones de RAM y swap de `RamDetail.qml`: `(mib / 1024).toFixed(2) + " GB"`.
pub fn gb2(mib: f64) -> String {
    format!("{:.2} GB", mib / KIB)
}

/// `formatStorageBytes`.
pub fn storage_bytes(bytes: f64) -> String {
    if bytes < KIB {
        format!("{bytes:.0} B")
    } else if bytes < MIB {
        format!("{:.1} KB", bytes / KIB)
    } else if bytes < GIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes < TIB {
        format!("{:.1} GB", bytes / GIB)
    } else {
        format!("{:.1} TB", bytes / TIB)
    }
}

/// `formatPercent`: un decimal sólo por debajo del 1 %.
pub fn percent(p: f32) -> String {
    if p < 1.0 {
        format!("{p:.1}%")
    } else {
        format!("{p:.0}%")
    }
}

/// `formatCpuClock`. Vacío sin dato, para que quien llama elija el reemplazo.
pub fn clock(mhz: f64) -> String {
    if mhz <= 0.0 {
        String::new()
    } else if mhz < 1000.0 {
        format!("{mhz:.0} MHz")
    } else {
        format!("{:.2} GHz", mhz / 1000.0)
    }
}

/// Lo que muestra `uptime -p` sin el "up", en español: las unidades en cero se omiten.
pub fn uptime(secs: u64) -> String {
    const UNITS: [(u64, &str, &str); 4] = [
        (604_800, "semana", "semanas"),
        (86_400, "día", "días"),
        (3_600, "hora", "horas"),
        (60, "minuto", "minutos"),
    ];
    let mut rest = secs;
    let mut parts = Vec::new();
    for (size, one, many) in UNITS {
        let n = rest / size;
        rest %= size;
        if n > 0 {
            parts.push(format!("{n} {}", if n == 1 { one } else { many }));
        }
    }
    if parts.is_empty() {
        "0 minutos".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_cambia_de_unidad_como_el_qml() {
        assert_eq!(rate(512.0), "512 B/s");
        assert_eq!(rate(1_024_000.0), "1000.0 KB/s");
        assert_eq!(rate(3_145_728.0), "3.0 MB/s");
    }

    #[test]
    fn compact_rate_rellena_los_bytes_a_cuatro() {
        assert_eq!(compact_rate(0.0), "0   B/s");
        assert_eq!(compact_rate(512.0), "512 B/s");
        assert_eq!(compact_rate(20_480.0), "20 KB/s");
        assert_eq!(compact_rate(3_145_728.0), "3.0 MB/s");
    }

    #[test]
    fn memory_kib_usa_un_decimal_solo_debajo_de_10_mb() {
        assert_eq!(memory_kib(512.0), "512 KB");
        assert_eq!(memory_kib(2048.0), "2.0 MB");
        assert_eq!(memory_kib(20_480.0), "20 MB");
        assert_eq!(memory_kib(2_097_152.0), "2.0 GB");
    }

    #[test]
    fn memory_mib_usa_un_decimal_solo_debajo_de_100_mb() {
        assert_eq!(memory_mib(50.0), "50.0 MB");
        assert_eq!(memory_mib(512.0), "512 MB");
        assert_eq!(memory_mib(2048.0), "2.0 GB");
    }

    #[test]
    fn gb2_es_el_renglon_de_ram() {
        assert_eq!(gb2(31_931.277), "31.18 GB");
    }

    #[test]
    fn storage_bytes_llega_a_terabytes() {
        assert_eq!(storage_bytes(500.0), "500 B");
        assert_eq!(storage_bytes(500_107_862_016.0), "465.8 GB");
        assert_eq!(storage_bytes(2_199_023_255_552.0), "2.0 TB");
    }

    #[test]
    fn percent_usa_un_decimal_solo_debajo_de_uno() {
        assert_eq!(percent(0.5), "0.5%");
        assert_eq!(percent(42.4), "42%");
    }

    #[test]
    fn clock_vacio_sin_dato() {
        assert_eq!(clock(0.0), "");
        assert_eq!(clock(800.0), "800 MHz");
        assert_eq!(clock(3222.964), "3.22 GHz");
    }

    #[test]
    fn uptime_omite_las_unidades_en_cero() {
        assert_eq!(uptime(30), "0 minutos");
        assert_eq!(uptime(1417), "23 minutos");
        assert_eq!(uptime(90_061), "1 día, 1 hora, 1 minuto");
        assert_eq!(uptime(1_300_000), "2 semanas, 1 día, 1 hora, 6 minutos");
    }
}
