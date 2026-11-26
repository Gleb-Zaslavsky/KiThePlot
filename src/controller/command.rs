//! Undoable state transitions.
//!
//! Commands keep old/new values so the controller can apply and reverse edits safely.

use crate::model::{
    AxisConfig, AxisKind, ErrorBarSide, LayoutConfig, LegendConfig, MarkerStyle, PlotAnnotation,
    PlotModel, PlotPanel, RangePolicy, ScaleType, SeriesId, SeriesModel,
};

/// EN: Mutations with old/new values for undo/redo-safe state transitions.
/// RU: Izmeneniya sostoyaniya s old/new dlya bezopasnogo undo/redo.
pub enum Command {
    ReplacePlotModel {
        old: Box<PlotModel>,
        new: Box<PlotModel>,
    },
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
    SetAxisTitleFontSize {
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
    SetAxesRanges {
        old_x: RangePolicy,
        new_x: RangePolicy,
        old_y: RangePolicy,
        new_y: RangePolicy,
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
        old_panels: Vec<PlotPanel>,
        new_panels: Vec<PlotPanel>,
    },
    RemoveSeries {
        series: SeriesModel,
        index: usize,
        old_panels: Vec<PlotPanel>,
        new_panels: Vec<PlotPanel>,
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
    SetSeriesErrorColumn {
        series_id: SeriesId,
        axis: AxisKind,
        old: Option<String>,
        new: Option<String>,
    },
    SetSeriesAsymmetricErrorColumn {
        series_id: SeriesId,
        axis: AxisKind,
        side: ErrorBarSide,
        old: Option<String>,
        new: Option<String>,
    },
    SetSeriesLineWidth {
        series_id: SeriesId,
        old: f32,
        new: f32,
    },
    SetSeriesLineVisible {
        series_id: SeriesId,
        old: bool,
        new: bool,
    },
    SetSeriesLineStyle {
        series_id: SeriesId,
        old: crate::model::LineStyle,
        new: crate::model::LineStyle,
    },
    SetSeriesLineInterpolation {
        series_id: SeriesId,
        old: crate::model::LineInterpolation,
        new: crate::model::LineInterpolation,
    },
    SetSeriesAreaBaseline {
        series_id: SeriesId,
        old: Option<f64>,
        new: Option<f64>,
        old_visible: bool,
        new_visible: bool,
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
        old_preset: MarkerStyle,
        new_preset: MarkerStyle,
    },
    ReplaceLegend {
        old: LegendConfig,
        new: LegendConfig,
    },
    ReplaceLayout {
        old: LayoutConfig,
        new: LayoutConfig,
    },
    ReplaceAnnotations {
        old: Vec<PlotAnnotation>,
        new: Vec<PlotAnnotation>,
    },
    ReplacePanels {
        old: Vec<PlotPanel>,
        new: Vec<PlotPanel>,
    },
    Batch {
        commands: Vec<Command>,
    },
}
