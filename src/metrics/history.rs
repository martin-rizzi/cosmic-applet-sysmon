// SPDX-License-Identifier: GPL-3.0-only

//! Buffer circular del historial. 60 puntos, como `main.qml` del plasmoid.

use std::collections::VecDeque;

pub const CAPACITY: usize = 60;

#[derive(Default)]
pub struct History {
    points: VecDeque<f32>,
}

impl History {
    pub fn push(&mut self, v: f32) {
        if self.len() == CAPACITY {
            self.points.pop_front();
        }
        self.points.push_back(v);
    }

    pub fn points(&self) -> Vec<f32> {
        self.points.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }
}

/// Rótulo del extremo izquierdo del eje.
///
/// El plasmoid lo tiene hardcodeado y además inconsistente entre paneles ("5 mins ago"
/// en CpuDetail, "2 mins ago" en RamDetail) cuando el buffer real son 60 puntos. Acá se
/// calcula, así que sigue siendo correcto si se cambia el intervalo.
pub fn window_label(update_interval_ms: u32) -> String {
    let secs = (CAPACITY as u32 * update_interval_ms) / 1000;
    if secs >= 60 {
        format!("hace {} min", secs / 60)
    } else {
        format!("hace {secs} s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranca_vacio() {
        assert_eq!(History::default().points().len(), 0);
    }

    #[test]
    fn conserva_el_orden_de_llegada() {
        let mut h = History::default();
        h.push(0.1);
        h.push(0.2);
        assert_eq!(h.points(), vec![0.1, 0.2]);
    }

    #[test]
    fn descarta_el_mas_viejo_pasada_la_capacidad() {
        let mut h = History::default();
        for i in 0..(CAPACITY + 10) {
            h.push(i as f32);
        }
        let p = h.points();
        assert_eq!(p.len(), CAPACITY);
        // el más viejo que sobrevive es el 10
        assert_eq!(p[0], 10.0);
        assert_eq!(p[CAPACITY - 1], (CAPACITY + 9) as f32);
    }

    #[test]
    fn el_rotulo_sale_del_intervalo() {
        // 60 puntos × 2000 ms = 2 minutos
        assert_eq!(window_label(2000), "hace 2 min");
        // 60 × 500 ms = 30 s
        assert_eq!(window_label(500), "hace 30 s");
    }
}
