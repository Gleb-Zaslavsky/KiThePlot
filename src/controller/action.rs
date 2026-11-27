//! View-to-controller intents.
//!
//! Every user interaction is represented as an `Action` and dispatched to the controller.

use crate::model::{
    AxisKind, Color, ErrorBarSide, ImageFormat, ImageSize, ImportOptions, LegendPosition,
    LineInterpolation, LineStyle, MarkerShape, PlotFontFamily, PublicationDimensions, RangePolicy,
    ScaleType, SeriesId, TickNumberFormat,
};

/// EN: UI intents emitted by View and consumed by Controller.
/// RU: Namereniya UI, kotorye otpravlyaet View i obrabatyvaet Controller.
#[allow(missing_docs)]
pub enum Action {
    /// EN: Import a CSV file from disk into the data table.
    /// RU: Zagruska CSV-fayla s diska v tablitsu dannyh.
    ImportFromCsv {
        path: String,
    },

    /// EN: Import a TXT/whitespace-delimited file from disk into the data table.
    /// RU: Zagruska TXT-fayla s razdelitelyami probela v tablitsu dannyh.
    ImportFromTxt {
        path: String,
    },

    /// Open a configurable parse preview before replacing the current data table.
    PreviewImport {
        path: String,
        options: ImportOptions,
    },

    /// Preview a file that will be appended as a separate project data table.
    PreviewAddTable {
        path: String,
        options: ImportOptions,
    },

    /// Commit the currently validated import preview.
    ConfirmImport,

    /// Close the import preview without changing the current data table.
    CancelImport,

    /// Open a native `.kplot` project containing plot settings and embedded data.
    OpenProject {
        path: String,
    },

    /// Save the current plot and its data to a portable `.kplot` project.
    SaveProject {
        path: String,
        dimensions: PublicationDimensions,
    },
    SaveProjectThen {
        path: String,
        dimensions: PublicationDimensions,
        continuation: Box<Action>,
    },
    SavePlotStyle {
        path: String,
    },
    ApplyPlotStyle {
        path: String,
    },

    /// EN: Set the chart title. An empty value hides the title.
    /// RU: Ustanovit zagolovok grafa. Pustoe znachenie skryvaet zagolovok.
    SetChartTitle(String),

    /// EN: Toggle chart title visibility without clearing its text.
    /// RU: Pokazat ili skryt zagolovok, ne ochishchaya tekst.
    SetChartTitleVisible(bool),

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

    /// Set or clear the normalized axis title anchor. `None` restores automatic placement.
    SetAxisTitleAnchor {
        axis: AxisKind,
        anchor: Option<[f32; 2]>,
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

    /// Set both visible plot ranges as one undoable view change.
    SetAxesRanges {
        x: RangePolicy,
        y: RangePolicy,
    },
    /// Set a panel's independent axis transforms and range policies; `None` relinks it.
    SetPanelAxisView {
        panel_id: u64,
        view: Option<crate::model::PanelAxisView>,
    },
    /// Update a panel's manual ranges during wheel zoom or panning.
    SetPanelAxisRanges {
        panel_id: u64,
        x: RangePolicy,
        y: RangePolicy,
    },
    EndAxesGesture,
    BeginContinuousEdit,
    EndContinuousEdit,

    /// EN: Override major tick step (None = auto).
    /// RU: Zadat shag osnovnyh dekeniy (None = avto).
    SetAxisMajorTickStep {
        axis: AxisKind,
        step: Option<f64>,
    },

    /// Set numeric formatting for tick labels on an axis.
    SetAxisTickNumberFormat {
        axis: AxisKind,
        format: TickNumberFormat,
    },

    /// EN: Set number of minor ticks per major step.
    /// RU: Kolichestvo vtorostepennyh dekeniy na odin osnovnoy shag.
    SetAxisMinorTicks {
        axis: AxisKind,
        per_major: u16,
    },

    /// Configure visible major/minor axis tick marks and their outward lengths.
    SetAxisTickMarkStyle {
        axis: AxisKind,
        minor: bool,
        visible: bool,
        length: f32,
    },

    /// EN: Toggle major or minor grid lines for one axis.
    /// RU: Vklyuchit ili skryt osnovnuyu ili vtorostepennuyu setku osi.
    SetAxisGridVisibility {
        axis: AxisKind,
        minor: bool,
        visible: bool,
    },
    SetAxisGridStyle {
        axis: AxisKind,
        minor: bool,
        color: Color,
        width: f32,
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

    /// Atomically replace the X/Y pair after validating that both columns have equal lengths.
    SetSeriesColumns {
        series_id: SeriesId,
        x_column: String,
        y_column: String,
    },

    /// Rebind a series to another named table, choosing a valid default X/Y pair.
    SetSeriesTable {
        series_id: SeriesId,
        table_id: crate::model::DataTableId,
    },

    /// Set or clear confidence-band bounds using two source columns.
    SetSeriesConfidenceBand {
        series_id: SeriesId,
        band: Option<crate::model::ConfidenceBand>,
    },

    /// Select an optional uncertainty column for the X or Y axis.
    SetSeriesErrorColumn {
        series_id: SeriesId,
        axis: AxisKind,
        column: Option<String>,
    },

    /// Select an optional lower or upper uncertainty column for one axis.
    SetSeriesAsymmetricErrorColumn {
        series_id: SeriesId,
        axis: AxisKind,
        side: ErrorBarSide,
        column: Option<String>,
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

    /// EN: Toggle the connecting line while keeping point markers independent.
    /// RU: Vklyuchit ili skryt linyu mezhdu tochkami otdelno ot markerov.
    SetSeriesLineVisible {
        series_id: SeriesId,
        visible: bool,
    },

    /// EN: Set series line style (Solid/Dashed/Dotted).
    /// RU: Ustanovit stil linii serii (Solid/Dashed/Dotted).
    SetSeriesLineStyle {
        series_id: SeriesId,
        line_style: LineStyle,
    },

    /// Select linear or step-after interpolation for a connected series.
    SetSeriesLineInterpolation {
        series_id: SeriesId,
        interpolation: LineInterpolation,
    },

    /// Set or clear a filled area extending from the series to a data-space Y baseline.
    SetSeriesAreaBaseline {
        series_id: SeriesId,
        baseline: Option<f64>,
    },
    SetSeriesAreaVisibility {
        series_id: SeriesId,
        visible: bool,
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

    /// Update the normalized top-left position of the legend.
    SetLegendAnchor([f32; 2]),

    /// EN: Set legend font size (>= 8).
    /// RU: Ustanovit razmer shrifta legendy (>= 8).
    SetLegendFontSize(u32),

    /// EN: Set legend font color.
    /// RU: Ustanovit tsvet shrifta legendy.
    SetLegendFontColor(Color),

    /// Configure legend box spacing, line sample, background, and border.
    SetLegendLayout {
        padding: u32,
        row_spacing: u32,
        sample_length: u32,
        columns: u32,
        column_spacing: u32,
        background_color: Color,
        border_color: Color,
        border_width: f32,
    },

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

    /// Set the font family used for all chart text.
    SetPlotFontFamily(PlotFontFamily),
    SetAnnotations(Vec<crate::model::PlotAnnotation>),
    SetPanels(Vec<crate::model::PlotPanel>),

    /// EN: Export plot to a file path in a given format and size.
    /// RU: Eksportirovat grafik v ukazannyy fail, format i razmer.
    RequestSaveAs {
        path: String,
        format: ImageFormat,
        size: ImageSize,
    },

    /// Export with physical figure dimensions and a raster resolution.
    RequestPublicationExport {
        path: String,
        format: ImageFormat,
        dimensions: PublicationDimensions,
    },

    /// Report a file-selection error through the editor status area.
    ReportUiError(String),

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
