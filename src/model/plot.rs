//! Plot aggregate state.
//!
//! `PlotModel` is the central in-memory document edited by UI controls and
//! consumed by rendering/export logic.

use crate::model::types::*;

/// Main plot document.
#[derive(Clone)]
pub struct PlotModel {
    pub axes: AxesConfig,
    pub series: Vec<SeriesModel>,
    pub legend: LegendConfig,
    pub layout: LayoutConfig,
}

/// One logical visual series on the chart.
#[derive(Clone)]
pub struct SeriesModel {
    pub id: SeriesId,
    pub name: String,
    pub x_column: String,
    pub y_column: String,
    pub style: SeriesStyle,
    pub visible: bool,
}
