// SPDX-License-Identifier: GPL-3.0-only

//! Tasa por delta entre dos lecturas de contadores acumulados (red y disco, spec §3.3).

/// Unidades por segundo entre dos lecturas de un par de contadores. Sin lectura previa o
/// con intervalo nulo da 0: la primera lectura sólo fija la base. Un contador que
/// retrocede (interfaz que desaparece, disco que se desconecta) también da 0, como el
/// `Math.max(0, …)` del plasmoid.
pub fn per_second(prev: Option<(u64, u64)>, now: (u64, u64), elapsed_secs: f64) -> (f64, f64) {
    match prev {
        Some((a, b)) if elapsed_secs > 0.0 => (
            now.0.saturating_sub(a) as f64 / elapsed_secs,
            now.1.saturating_sub(b) as f64 / elapsed_secs,
        ),
        _ => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_primera_lectura_solo_fija_la_base() {
        assert_eq!(per_second(None, (1000, 1000), 2.0), (0.0, 0.0));
    }

    #[test]
    fn divide_el_delta_por_el_intervalo() {
        assert_eq!(per_second(Some((1000, 500)), (3000, 1500), 2.0), (1000.0, 500.0));
    }

    #[test]
    fn contador_que_retrocede_o_intervalo_nulo_dan_cero() {
        assert_eq!(per_second(Some((5000, 5000)), (1000, 6000), 1.0), (0.0, 1000.0));
        assert_eq!(per_second(Some((0, 0)), (1000, 1000), 0.0), (0.0, 0.0));
    }
}
