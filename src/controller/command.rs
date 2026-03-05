//! Undoable state transitions.
//!
//! Commands keep old/new values so the controller can apply and reverse edits safely.

use crate::model::{
    AxisConfig, AxisKind, LayoutConfig, LegendConfig, MarkerStyle, RangePolicy, ScaleType,
    SeriesId, SeriesModel,
};

/// EN: Mutations with old/new values for undo/redo-safe state transitions.
/// RU: Izmeneniya sostoyaniya s old/new dlya bezopasnogo undo/redo.
pub enum Command {
    SetChartTitle {
        old: String,
        new: String,
    },
    SetAxisLabel {
        axis: AxisKind,
        old: String,
        new: String,
    },
    SetAxisLabelFontSize {
        axis: AxisKind,
        old: u32,
        new: u32,
    },
    SetAxisScale {
        axis: AxisKind,
        old: ScaleType,
        new: ScaleType,
    },
    SetAxisRange {
        axis: AxisKind,
        old: RangePolicy,
        new: RangePolicy,
    },
    SetAxisMajorTickStep {
        axis: AxisKind,
        old: Option<f64>,
        new: Option<f64>,
    },
    SetAxisMinorTicks {
        axis: AxisKind,
        old: u16,
        new: u16,
    },
    ReplaceAxisConfig {
        axis: AxisKind,
        old: AxisConfig,
        new: AxisConfig,
    },
    AddSeries {
        series: SeriesModel,
        index: usize,
    },
    RemoveSeries {
        series: SeriesModel,
        index: usize,
    },
    RenameSeries {
        series_id: SeriesId,
        old: String,
        new: String,
    },
    SetSeriesVisibility {
        series_id: SeriesId,
        old: bool,
        new: bool,
    },
    SetSeriesXColumn {
        series_id: SeriesId,
        old: String,
        new: String,
    },
    SetSeriesYColumn {
        series_id: SeriesId,
        old: String,
        new: String,
    },
    SetSeriesLineWidth {
        series_id: SeriesId,
        old: f32,
        new: f32,
    },
    SetSeriesLineStyle {
        series_id: SeriesId,
        old: crate::model::LineStyle,
        new: crate::model::LineStyle,
    },
    SetSeriesColor {
        series_id: SeriesId,
        old: crate::model::Color,
        new: crate::model::Color,
    },
    SetSeriesMarker {
        series_id: SeriesId,
        old: Option<MarkerStyle>,
        new: Option<MarkerStyle>,
    },
    ReplaceLegend {
        old: LegendConfig,
        new: LegendConfig,
    },
    ReplaceLayout {
        old: LayoutConfig,
        new: LayoutConfig,
    },
    Batch {
        commands: Vec<Command>,
    },
}


