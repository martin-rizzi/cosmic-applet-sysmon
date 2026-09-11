//! Generación de los medidores como SVG.
//!
//! Geometría y colores calcados de CompactView.qml / VerticalUsageMeter.qml del
//! plasmoid: caja con borde de 1 px y radio 2, barras ancladas abajo dentro de un
//! margen de 1 px, y para la RAM el relleno cambia de color por umbral.

use std::fmt::Write;

/// Colores que el plasmoid rota entre los núcleos (`coreColors`).
pub const CORE_COLORS: [&str; 6] = [
    "#00aaff", "#22cc66", "#ffaa00", "#aa66ff", "#ff6688", "#00ccbb",
];

pub const NORMAL: &str = "#00aaff";
pub const WARNING: &str = "#ffaa00";
pub const CRITICAL: &str = "#ff4444";

/// Opacidad del borde: `withAlpha(Kirigami.Theme.textColor, 0.35)`.
const BORDER_OPACITY: f32 = 0.35;

const WARNING_THRESHOLD: f32 = 0.7;
const CRITICAL_THRESHOLD: f32 = 0.85;

pub fn threshold_color(fraction: f32) -> &'static str {
    if fraction > CRITICAL_THRESHOLD {
        CRITICAL
    } else if fraction > WARNING_THRESHOLD {
        WARNING
    } else {
        NORMAL
    }
}

/// Disco y GPU en el panel: `warningThreshold: 1`, así que nunca pasan por naranja.
pub fn critical_only_color(fraction: f32) -> &'static str {
    if fraction > CRITICAL_THRESHOLD {
        CRITICAL
    } else {
        NORMAL
    }
}

/// `#rrggbb` a color de iced, para pintar texto con los colores de umbral.
pub fn hex_color(hex: &str) -> cosmic::iced::Color {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x00ff_ffff);
    cosmic::iced::Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn header(svg: &mut String, w: f32, h: f32, border: &str) {
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
    );
    let _ = write!(
        svg,
        r#"<rect x="0.5" y="0.5" width="{}" height="{}" rx="2" ry="2" fill="none" stroke="{border}" stroke-opacity="{BORDER_OPACITY}" stroke-width="1"/>"#,
        w - 1.0,
        h - 1.0
    );
}

/// Una barra vertical por núcleo, pegadas entre sí, dentro de la caja con borde.
pub fn cpu_bars(cores: &[f32], w: f32, h: f32, border: &str) -> String {
    let mut svg = String::with_capacity(160 + cores.len() * 90);
    header(&mut svg, w, h, border);

    let inner_h = (h - 2.0).max(0.0);
    let inner_w = (w - 2.0).max(0.0);
    let n = cores.len().max(1) as f32;
    let cell = inner_w / n;

    for (i, usage) in cores.iter().enumerate() {
        let usage = usage.clamp(0.0, 100.0);
        if usage <= 0.0 {
            continue;
        }
        // El plasmoid nunca dibuja menos de 1 px cuando hay uso.
        let bar_h = (inner_h * usage / 100.0).max(1.0);
        let x = 1.0 + cell * i as f32;
        let y = 1.0 + inner_h - bar_h;
        let _ = write!(
            svg,
            r#"<rect x="{x:.2}" y="{y:.2}" width="{cell:.2}" height="{bar_h:.2}" fill="{}"/>"#,
            CORE_COLORS[i % CORE_COLORS.len()]
        );
    }

    svg.push_str("</svg>");
    svg
}

/// Medidor vertical único (RAM), con color por umbral.
pub fn usage_meter(fraction: f32, w: f32, h: f32, border: &str) -> String {
    let fraction = fraction.clamp(0.0, 1.0);
    usage_meter_with(fraction, w, h, border, threshold_color(fraction), 0.0)
}

/// Medidor vertical con color y relleno mínimo explícitos: `VerticalUsageMeter.qml`.
/// `min_fill` sólo actúa si hay uso — en cero no se dibuja nada.
pub fn usage_meter_with(fraction: f32, w: f32, h: f32, border: &str, fill: &str, min_fill: f32) -> String {
    let mut svg = String::with_capacity(240);
    header(&mut svg, w, h, border);

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let inner_h = (h - 2.0).max(0.0);
        let bar_h = (inner_h * fraction).max(min_fill).min(inner_h);
        let _ = write!(
            svg,
            r#"<rect x="1" y="{:.2}" width="{:.2}" height="{bar_h:.2}" rx="1" ry="1" fill="{fill}"/>"#,
            1.0 + inner_h - bar_h,
            (w - 2.0).max(0.0),
        );
    }

    svg.push_str("</svg>");
    svg
}

/// Barra horizontal para el popup: mismo lenguaje visual que el panel.
pub fn horizontal_bar(fraction: f32, w: f32, h: f32, fill: &str, border: &str) -> String {
    let mut svg = String::with_capacity(240);
    header(&mut svg, w, h, border);

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let inner_w = (w - 2.0).max(0.0);
        let _ = write!(
            svg,
            r#"<rect x="1" y="1" width="{:.2}" height="{:.2}" rx="1" ry="1" fill="{fill}"/>"#,
            (inner_w * fraction).max(1.0),
            (h - 2.0).max(0.0)
        );
    }

    svg.push_str("</svg>");
    svg
}

/// Gráfico de área con grilla, calcado del Canvas de CpuDetail.qml: fondo tenue,
/// tres líneas horizontales y el área rellena bajo la curva.
pub fn history_graph(points: &[f32], w: f32, h: f32, fill: &str, border: &str) -> String {
    let mut svg = String::with_capacity(200 + points.len() * 16);
    header(&mut svg, w, h, border);

    for i in 1..=3 {
        let y = h * (i as f32 / 4.0);
        let _ = write!(
            svg,
            r#"<line x1="1" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{border}" stroke-opacity="0.12" stroke-width="1"/>"#,
            w - 1.0
        );
    }

    if points.len() >= 2 {
        let inner_w = (w - 2.0).max(0.0);
        let inner_h = (h - 2.0).max(0.0);
        let step = inner_w / (points.len() - 1) as f32;
        let mut poly = String::with_capacity(points.len() * 14);
        for (i, p) in points.iter().enumerate() {
            let x = 1.0 + step * i as f32;
            let y = 1.0 + inner_h * (1.0 - p.clamp(0.0, 1.0));
            let _ = write!(poly, "{x:.2},{y:.2} ");
        }
        // Cierra el polígono contra la base para que quede un área.
        let _ = write!(poly, "{:.2},{:.2} 1.00,{:.2}", 1.0 + inner_w, 1.0 + inner_h, 1.0 + inner_h);
        let _ = write!(
            svg,
            r#"<polygon points="{poly}" fill="{fill}" fill-opacity="0.2" stroke="{fill}" stroke-width="1"/>"#
        );
    }

    svg.push_str("</svg>");
    svg
}

/// Dos series en mitades separadas, las dos creciendo hacia arriba: el Canvas de
/// `NetworkDetail.qml` (subida arriba, bajada abajo), `StorageDetail.qml` (lectura
/// arriba, escritura abajo) y el mini gráfico de red del panel.
pub struct DualGraph<'a> {
    /// Valores 0–1.
    pub top: &'a [f32],
    pub bottom: &'a [f32],
    pub top_color: &'a str,
    pub bottom_color: &'a str,
    /// Separación de cada área respecto del divisor central.
    pub gap: f32,
    /// Margen superior del área de arriba.
    pub top_padding: f32,
    /// Tres líneas de grilla por mitad, como los paneles de detalle. El panel no la usa.
    pub grid: bool,
}

pub fn dual_history_graph(g: &DualGraph, w: f32, h: f32, border: &str) -> String {
    let mut svg = String::with_capacity(400 + (g.top.len() + g.bottom.len()) * 16);
    header(&mut svg, w, h, border);

    let half = h / 2.0;
    let top_base = half - g.gap;
    let top_h = (top_base - g.top_padding).max(0.0);
    let bottom_h = (h - (half + g.gap)).max(0.0);

    if g.grid {
        for q in [0.25f32, 0.5, 0.75] {
            grid_line(&mut svg, top_base - top_h * q, w, border);
            grid_line(&mut svg, h - bottom_h * q, w, border);
        }
    }
    let _ = write!(
        svg,
        r#"<line x1="0" y1="{half:.2}" x2="{w:.2}" y2="{half:.2}" stroke="{border}" stroke-opacity="{BORDER_OPACITY}" stroke-width="1"/>"#
    );
    area(&mut svg, g.top, w, top_base, top_h, g.top_color);
    area(&mut svg, g.bottom, w, h, bottom_h, g.bottom_color);

    svg.push_str("</svg>");
    svg
}

fn grid_line(svg: &mut String, y: f32, w: f32, border: &str) {
    let _ = write!(
        svg,
        r#"<line x1="1" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{border}" stroke-opacity="0.12" stroke-width="1"/>"#,
        w - 1.0
    );
}

/// Área bajo la curva con base en `base` y `height` de alto. Con menos de dos puntos no
/// hay curva que dibujar.
fn area(svg: &mut String, points: &[f32], w: f32, base: f32, height: f32, color: &str) {
    if points.len() < 2 {
        return;
    }
    let step = w / (points.len() - 1) as f32;
    let mut poly = String::with_capacity(points.len() * 14 + 32);
    for (i, p) in points.iter().enumerate() {
        let _ = write!(poly, "{:.2},{:.2} ", step * i as f32, base - height * p.clamp(0.0, 1.0));
    }
    let _ = write!(poly, "{w:.2},{base:.2} 0.00,{base:.2}");
    let _ = write!(
        svg,
        r#"<polygon points="{poly}" fill="{color}" fill-opacity="0.2" stroke="{color}" stroke-width="1"/>"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_grafico_vacio_sigue_siendo_svg_valido() {
        let svg = history_graph(&[], 100.0, 40.0, NORMAL, "#ffffff");
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn dibuja_la_grilla_y_el_area() {
        let svg = history_graph(&[0.0, 0.5, 1.0], 100.0, 40.0, NORMAL, "#ffffff");
        // tres líneas de grilla horizontales, como el Canvas del plasmoid
        assert_eq!(svg.matches("<line").count(), 3);
        assert!(svg.contains("<polygon"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn un_solo_punto_no_divide_por_cero() {
        let svg = history_graph(&[0.7], 100.0, 40.0, NORMAL, "#ffffff");
        assert!(!svg.contains("NaN"));
        assert!(!svg.contains("inf"));
    }

    #[test]
    fn hex_color_lee_rrggbb() {
        assert_eq!(hex_color("#ff4444"), cosmic::iced::Color::from_rgb8(0xff, 0x44, 0x44));
    }

    #[test]
    fn critical_only_nunca_da_naranja() {
        // `warningThreshold: 1` en las secciones de disco y GPU del QML.
        assert_eq!(critical_only_color(0.8), NORMAL);
        assert_eq!(critical_only_color(0.86), CRITICAL);
    }

    #[test]
    fn el_relleno_minimo_se_respeta_con_uso_chico() {
        let svg = usage_meter_with(0.01, 10.0, 16.0, "#ffffff", NORMAL, 3.0);
        assert!(svg.contains(r#"height="3.00""#), "{svg}");
    }

    #[test]
    fn sin_uso_no_hay_relleno_aunque_haya_minimo() {
        let svg = usage_meter_with(0.0, 10.0, 16.0, "#ffffff", NORMAL, 3.0);
        assert_eq!(svg.matches("<rect").count(), 1, "sólo el borde");
    }

    fn dual<'a>(top: &'a [f32], bottom: &'a [f32], grid: bool) -> DualGraph<'a> {
        DualGraph {
            top,
            bottom,
            top_color: CRITICAL,
            bottom_color: NORMAL,
            gap: 3.0,
            top_padding: 3.0,
            grid,
        }
    }

    #[test]
    fn grafico_doble_con_grilla_divisor_y_dos_areas() {
        let svg = dual_history_graph(&dual(&[0.0, 1.0], &[1.0, 0.0], true), 100.0, 80.0, "#ffffff");
        // tres líneas de grilla por mitad + el divisor
        assert_eq!(svg.matches("<line").count(), 7);
        assert_eq!(svg.matches("<polygon").count(), 2);
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn cada_area_queda_en_su_mitad() {
        let svg = dual_history_graph(&dual(&[1.0, 1.0], &[1.0, 1.0], false), 100.0, 80.0, "#ffffff");
        // arriba: base 40-3 = 37, alto 37-3 = 34 → techo en y = 3
        assert!(svg.contains("0.00,3.00"), "{svg}");
        // abajo: base 80, alto 80-(40+3) = 37 → techo en y = 43
        assert!(svg.contains("0.00,43.00"), "{svg}");
        assert_eq!(svg.matches("<line").count(), 1, "sin grilla queda sólo el divisor");
    }

    #[test]
    fn grafico_doble_con_un_punto_no_dibuja_areas() {
        let svg = dual_history_graph(&dual(&[0.5], &[], true), 100.0, 80.0, "#ffffff");
        assert_eq!(svg.matches("<polygon").count(), 0);
        assert!(!svg.contains("NaN") && !svg.contains("inf"));
    }
}
