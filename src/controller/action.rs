//! View-to-controller intents.
//!
//! Every user interaction is represented as an `Action` and dispatched to the controller.

use crate::model::{
    AxisKind, Color, ImageFormat, ImageSize, LegendPosition, LineStyle, MarkerShape, RangePolicy,
    ScaleType, SeriesId,
};

/// EN: UI intents emitted by View and consumed by Controller.
/// RU: Namereniya UI, kotorye otpravlyaet View i obrabatyvaet Controller.
#[allow(missing_docs)]
pub enum Action {
    /// EN: Import a CSV file from disk into the data table.
    /// RU: Zagruska CSV-fayla s diska v tablitsu dannyh.
    ImportFromCsv { path: String },

    /// EN: Import a TXT/whitespace-delimited file from disk into the data table.
    /// RU: Zagruska TXT-fayla s razdelitelyami probela v tablitsu dannyh.
    ImportFromTxt { path: String },

    /// EN: Set the chart title (non-empty enforced by controller).
    /// RU: Ustanovit zagolovok grafa (ne mozhet byt pustym).
    SetChartTitle(String),

    /// EN: Set axis displayed label text (non-empty enforced by controller).
    /// RU: Ustanovit podpis osi (ne mozhet byt pustoy).
    SetAxisLabel {
        axis: AxisKind,
        label: String,
    },

    /// EN: Set the tick label font size for the axis (clamped to >= 8).
    /// RU: Ustanovit razmer shrifta deleniya osi (ne menee 8).
    SetAxisLabelFontSize {
        axis: AxisKind,
        font_size: u32,
    },

    /// EN: Set the axis title font size (clamped to >= 8).
    /// RU: Ustanovit razmer shrifta nazvaniya osi (ne menee 8).
    SetAxisTitleFontSize {
        axis: AxisKind,
        font_size: u32,
    },

    /// EN: Switch axis scale (Linear/Log10/LogE). Log scales drop non-positive values.
    /// RU: Perekluchit shkalu osi (Linear/Log10/LogE). Pri log-shkale nepoloshitelnye znacheniya ignoriruyutsya.
    SetAxisScale {
        axis: AxisKind,
        scale: ScaleType,
    },

    /// EN: Set axis range policy (Auto or Manual[min, max]). Manual validated so min < max.
    /// RU: Ustanovit diapazon osi (Auto ili Manual[min, max]). Dlya Manual min < max obyazatelno.
    SetAxisRange {
        axis: AxisKind,
        range: RangePolicy,
    },

    /// EN: Override major tick step (None = auto).
    /// RU: Zadat shag osnovnyh dekeniy (None = avto).
    SetAxisMajorTickStep {
        axis: AxisKind,
        step: Option<f64>,
    },

    /// EN: Set number of minor ticks per major step.
    /// RU: Kolichestvo vtorostepennyh dekeniy na odin osnovnoy shag.
    SetAxisMinorTicks {
        axis: AxisKind,
        per_major: u16,
    },

    /// EN: Add a new data series; x/y columns may be empty to use defaults.
    /// RU: Dobavit novuyu seriyu; stolbtsy x/y mogut byt pustymi (po umolchaniyu).
    AddSeries {
        name: String,
        x_column: String,
        y_column: String,
    },

    /// EN: Remove existing series by id (controller prevents removing the last one).
    /// RU: Udalit sushchestvuyushchuyu seriyu po id (nelzya udalit poslednyuyu).
    RemoveSeries {
        series_id: SeriesId,
    },

    /// EN: Rename series.
    /// RU: Pereimenovat seriyu.
    RenameSeries {
        series_id: SeriesId,
        name: String,
    },

    /// EN: Toggle series visibility.
    /// RU: Perekluchit vidimost serii.
    SetSeriesVisibility {
        series_id: SeriesId,
        visible: bool,
    },

    /// EN: Change the X column for the series (must exist in the table).
    /// RU: Izmenit X-stolbets serii (dolzhen sushchestvovat v tablice).
    SetSeriesXColumn {
        series_id: SeriesId,
        x_column: String,
    },

    /// EN: Change the Y column for the series (must exist in the table).
    /// RU: Izmenit Y-stolbets serii (dolzhen sushchestvovat v tablice).
    SetSeriesYColumn {
        series_id: SeriesId,
        y_column: String,
    },

    /// EN: Set series color (RGBA bytes).
    /// RU: Ustanovit tsvet serii (RGBA v baytah).
    SetSeriesColor {
        series_id: SeriesId,
        color: Color,
    },

    /// EN: Set series line width (> 0 enforced).
    /// RU: Ustanovit tolshchinu linii serii (> 0 obyazatelno).
    SetSeriesLineWidth {
        series_id: SeriesId,
        width: f32,
    },

    /// EN: Set series line style (Solid/Dashed/Dotted).
    /// RU: Ustanovit stil linii serii (Solid/Dashed/Dotted).
    SetSeriesLineStyle {
        series_id: SeriesId,
        line_style: LineStyle,
    },

    /// EN: Enable/disable markers and/or change marker size.
    /// RU: Vklyuchit/otklyuchit markery i/ili izmenit ih razmer.
    SetSeriesMarker {
        series_id: SeriesId,
        marker: Option<MarkerShape>,
        size: f32,
    },

    /// EN: Toggle legend visibility.
    /// RU: Perekluchit vidimost legendy.
    SetLegendVisible(bool),

    /// EN: Set legend title (None = no title; controller trims/filters empty strings).
    /// RU: Ustanovit zagolovok legendy (None = bez zagolovka; pustye stroki otsekaetsya).
    SetLegendTitle(Option<String>),

    /// EN: Change legend position (top/bottom left/right).
    /// RU: Izmenit pozitsiyu legendy (verh/nis levo/pravo).
    SetLegendPosition(LegendPosition),

    /// EN: Set legend font size (>= 8).
    /// RU: Ustanovit razmer shrifta legendy (>= 8).
    SetLegendFontSize(u32),

    /// EN: Set legend font color.
    /// RU: Ustanovit tsvet shrifta legendy.
    SetLegendFontColor(Color),

    /// EN: Set outer margin around the chart area (in pixels).
    /// RU: Ustanovit vneshniy otstup vokrug grafa (pikseli).
    SetLayoutMargin(u32),

    /// EN: Set bottom label area size (affects X axis tick labels and title).
    /// RU: Ustanovit vysotu nizhney oblasti podpisey (vliyaet na X-metki i zagolovok osi).
    SetXLabelAreaSize(u32),

    /// EN: Set left label area size (affects Y axis tick labels and title).
    /// RU: Ustanovit shirinu levoy oblasti podpisey (vliyaet na Y-metki i zagolovok osi).
    SetYLabelAreaSize(u32),

    /// EN: Set chart title font size (>= 8).
    /// RU: Ustanovit razmer shrifta zagolovka grafa (>= 8).
    SetLabelFontSize(u32),

    /// EN: Set chart title font color.
    /// RU: Ustanovit tsvet shrifta zagolovka grafa.
    SetLabelFontColor(Color),

    /// EN: Export plot to a file path in a given format and size.
    /// RU: Eksportirovat grafik v ukazannyy fail, format i razmer.
    RequestSaveAs {
        path: String,
        format: ImageFormat,
        size: ImageSize,
    },

    /// EN: Undo last command if any.
    /// RU: Otmennit poslednyuyu komandu esli ona est.
    Undo,

    /// EN: Redo last undone command if any.
    /// RU: Povtorit poslednyuyu otmennennuyu komandu esli ona est.
    Redo,

    /// EN: Reserved for future: reset all plot settings to defaults.
    /// RU: Zarezervirovano: sbrosit vse nastroiki grafa k znacheniyam po umolchaniyu.
    ResetPlot,
}
