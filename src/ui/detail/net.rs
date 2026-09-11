// SPDX-License-Identifier: GPL-3.0-only

//! `NetworkDetail.qml`: subida y bajada, gráfico partido y leyenda. Sin barra.

use cosmic::widget::Column;
use cosmic::Element;

use super::{dual_view, legend, stat_row};
use crate::app::{Message, SysMon};
use crate::draw::{DualGraph, CRITICAL, NORMAL};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    Column::new()
        .spacing(6)
        .push(stat_row("Subida:", format::rate(m.net.tx_rate)))
        .push(stat_row("Bajada:", format::rate(m.net.rx_rate)))
        .push(dual_view(
            app,
            &DualGraph {
                top: &m.net_up_history.points(),
                bottom: &m.net_down_history.points(),
                top_color: CRITICAL,
                bottom_color: NORMAL,
                gap: 3.0,
                top_padding: 3.0,
                grid: true,
            },
            80.0,
        ))
        .push(legend(vec![(NORMAL, "Bajada".to_string()), (CRITICAL, "Subida".to_string())]))
        .into()
}
