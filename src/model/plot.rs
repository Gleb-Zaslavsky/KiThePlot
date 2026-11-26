//! Plot aggregate state.
//!
//! `PlotModel` is the central in-memory document edited by UI controls and
//! consumed by rendering/export logic.

use crate::model::types::*;
use serde::{Deserialize, Serialize};

/// Main plot document.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotModel {
    pub axes: AxesConfig,
    pub series: Vec<SeriesModel>,
    pub legend: LegendConfig,
    pub layout: LayoutConfig,
    #[serde(default)]
    pub annotations: Vec<PlotAnnotation>,
    #[serde(default)]
    pub panels: Vec<PlotPanel>,
}

/// One logical visual series on the chart.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesModel {
    pub id: SeriesId,
    pub name: String,
    pub x_column: String,
    pub y_column: String,
    #[serde(default)]
    pub x_error_column: Option<String>,
    #[serde(default)]
    pub y_error_column: Option<String>,
    #[serde(default)]
    pub x_error_lower_column: Option<String>,
    #[serde(default)]
    pub x_error_upper_column: Option<String>,
    #[serde(default)]
    pub y_error_lower_column: Option<String>,
    #[serde(default)]
    pub y_error_upper_column: Option<String>,
    pub style: SeriesStyle,
    pub visible: bool,
}
