// SPDX-License-Identifier: GPL-3.0-only

//! Tasa de red desde /proc/net/dev. Suma todas las interfaces menos `lo`, como
//! `parseNetDev` del plasmoid: incluye tailscale0, docker0 y compañía.

use std::fs;

use super::rate::per_second;

/// (bytes recibidos, bytes enviados) acumulados, sin `lo`.
pub fn parse_net_dev(raw: &str) -> (u64, u64) {
    let (mut rx, mut tx) = (0u64, 0u64);
    for line in raw.lines() {
        let Some((iface, rest)) = line.split_once(':') else {
            continue;
        };
        if iface.trim() == "lo" {
            continue;
        }
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 9 {
            continue;
        }
        rx += fields[0].parse::<u64>().unwrap_or(0);
        tx += fields[8].parse::<u64>().unwrap_or(0);
    }
    (rx, tx)
}

#[derive(Default)]
pub struct Net {
    prev: Option<(u64, u64)>,
    /// Bytes por segundo recibidos.
    pub rx_rate: f64,
    /// Bytes por segundo enviados.
    pub tx_rate: f64,
}

impl Net {
    pub fn apply(&mut self, sample: (u64, u64), elapsed_secs: f64) {
        (self.rx_rate, self.tx_rate) = per_second(self.prev, sample, elapsed_secs);
        self.prev = Some(sample);
    }

    pub fn refresh(&mut self, elapsed_secs: f64) {
        if let Ok(raw) = fs::read_to_string("/proc/net/dev") {
            self.apply(parse_net_dev(&raw), elapsed_secs);
        }
    }

    /// (rx, tx) para el historial. Cada punto se divide por el mayor de los dos en ese
    /// instante, con piso de 1 KB/s — la misma normalización por muestra de
    /// `parseNetDev`: el gráfico muestra la proporción entre subida y bajada, no la
    /// magnitud.
    pub fn normalized(&self) -> (f32, f32) {
        let max = self.rx_rate.max(self.tx_rate).max(1024.0);
        ((self.rx_rate / max) as f32, (self.tx_rate / max) as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV_A: &str = include_str!("../../tests/fixtures/proc_net_dev_a");
    const DEV_B: &str = include_str!("../../tests/fixtures/proc_net_dev_b");

    #[test]
    fn suma_todas_las_interfaces_menos_lo() {
        // wlp2s0 + tailscale0; enp1s0 y docker0 en cero; lo afuera.
        assert_eq!(parse_net_dev(DEV_A), (149_128_038, 20_469_037));
    }

    #[test]
    fn tasa_entre_dos_lecturas() {
        let mut net = Net::default();
        net.apply(parse_net_dev(DEV_A), 0.0);
        net.apply(parse_net_dev(DEV_B), 2.0);
        assert_eq!(net.rx_rate, 1_024_000.0);
        assert_eq!(net.tx_rate, 256.0);
    }

    #[test]
    fn normaliza_contra_el_mayor_con_piso_de_un_kb() {
        let mut net = Net::default();
        net.apply(parse_net_dev(DEV_A), 0.0);
        net.apply(parse_net_dev(DEV_B), 2.0);
        assert_eq!(net.normalized(), (1.0, 0.00025));
        // Sin tráfico: el piso evita dividir por cero.
        assert_eq!(Net::default().normalized(), (0.0, 0.0));
    }
}
