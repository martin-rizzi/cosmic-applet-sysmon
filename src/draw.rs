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
    let mut svg = String::with_capacity(240);
    header(&mut svg, w, h, border);

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let inner_h = (h - 2.0).max(0.0);
        let bar_h = inner_h * fraction;
        let _ = write!(
            svg,
            r#"<rect x="1" y="{:.2}" width="{:.2}" height="{bar_h:.2}" rx="1" ry="1" fill="{}"/>"#,
            1.0 + inner_h - bar_h,
            (w - 2.0).max(0.0),
            threshold_color(fraction)
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
}
