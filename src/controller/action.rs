//! View-to-controller intents.
//!
//! Every user interaction is represented as an `Action` and dispatched to the controller.

use crate::model::{
    AxisKind, Color, ImageFormat, ImageSize, LegendPosition, LineStyle, MarkerShape, RangePolicy,
    ScaleType, SeriesId,
};

/// EN: UI intents emitted by View and consumed by Controller.
/// RU: Namereniya UI, kotorye otpravlyaet View i obrabatyvaet Controller.
pub enum Action {
    ImportFromCsv { path: String },
    ImportFromTxt { path: String },
    SetChartTitle(String),
    SetAxisLabel {
        axis: AxisKind,
        label: String,
    },
    SetAxisLabelFontSize {
        axis: AxisKind,
        font_size: u32,
    },
    SetAxisScale {
        axis: AxisKind,
        scale: ScaleType,
    },
    SetAxisRange {
        axis: AxisKind,
        range: RangePolicy,
    },
    SetAxisMajorTickStep {
        axis: AxisKind,
        step: Option<f64>,
    },
    SetAxisMinorTicks {
        axis: AxisKind,
        per_major: u16,
    },
    AddSeries {
        name: String,
        x_column: String,
        y_column: String,
    },
    RemoveSeries {
        series_id: SeriesId,
    },
    RenameSeries {
        series_id: SeriesId,
        name: String,
    },
    SetSeriesVisibility {
        series_id: SeriesId,
        visible: bool,
    },
    SetSeriesXColumn {
        series_id: SeriesId,
        x_column: String,
    },
    SetSeriesYColumn {
        series_id: SeriesId,
        y_column: String,
    },
    SetSeriesColor {
        series_id: SeriesId,
        color: Color,
    },
    SetSeriesLineWidth {
        series_id: SeriesId,
        width: f32,
    },
    SetSeriesLineStyle {
        series_id: SeriesId,
        line_style: LineStyle,
    },
    SetSeriesMarker {
        series_id: SeriesId,
        marker: Option<MarkerShape>,
        size: f32,
    },
    SetLegendVisible(bool),
    SetLegendTitle(Option<String>),
    SetLegendPosition(LegendPosition),
    SetLegendFontSize(u32),
    SetLegendFontColor(Color),
    SetLayoutMargin(u32),
    SetXLabelAreaSize(u32),
    SetYLabelAreaSize(u32),
    SetLabelFontSize(u32),
    SetLabelFontColor(Color),
    RequestSaveAs {
        path: String,
        format: ImageFormat,
        size: ImageSize,
    },
    Undo,
    Redo,
    ResetPlot,
}


