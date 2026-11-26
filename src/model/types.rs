//! Core reusable types for plot configuration and styling.
//!
//! These types are intentionally renderer-agnostic and form the stable contract
//! between UI/controller logic and rendering backends.

use serde::{Deserialize, Serialize};

/// Stable identifier of a plotted series.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeriesId(pub u64);

/// Pair of x/y axis configurations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxesConfig {
    pub x: AxisConfig,
    pub y: AxisConfig,
}

/// Full axis setup edited by axis controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxisConfig {
    pub label: String,
    /// Optional normalized anchor for the axis title in the chart canvas.
    pub title_anchor: Option<[f32; 2]>,
    pub axis_title_font_size: u32,
    pub label_font_size: u32,
    pub scale: ScaleType,
    pub range: RangePolicy,
    pub ticks: TickConfig,
}

/// Axis transformation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleType {
    Linear,
    /// Plot the transformed value `log10(data)` and label the axis accordingly.
    Log10,
    /// Use logarithmic spacing while displaying original data values (`1`, `10`, `100`, ...).
    Log10Axis,
    LogE,
}

/// Axis range policy (`Auto` from data or fixed manual bounds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RangePolicy {
    Auto,
    Manual { min: f64, max: f64 },
}

/// Tick density controls used by mesh configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TickConfig {
    pub major_step: Option<f64>,
    pub number_format: TickNumberFormat,
    pub minor_per_major: u16,
    pub major_ticks_visible: bool,
    pub minor_ticks_visible: bool,
    pub major_tick_length: f32,
    pub minor_tick_length: f32,
    pub major_grid_visible: bool,
    pub minor_grid_visible: bool,
    pub major_grid_color: Color,
    pub major_grid_width: f32,
    pub minor_grid_color: Color,
    pub minor_grid_width: f32,
}

/// Numeric notation used for axis tick labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TickNumberFormat {
    Auto,
    Fixed { decimals: u8 },
    Scientific { decimals: u8 },
}

/// Chart legend configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegendConfig {
    pub visible: bool,
    pub title: Option<String>,
    pub position: LegendPosition,
    pub font_size: u32,
    pub font_color: Color,
    pub padding: u32,
    pub row_spacing: u32,
    pub sample_length: u32,
    pub columns: u32,
    pub column_spacing: u32,
    pub background_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    /// Normalized top-left anchor within the chart canvas.
    pub anchor: [f32; 2],
}

/// Legend position inside plot area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegendPosition {
    /// User-defined normalized position. It can be moved by dragging the legend.
    Custom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    OutsideLeft,
    OutsideRight,
    OutsideTop,
    OutsideBottom,
}

/// Layout-level settings of the plot area and title style.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutConfig {
    pub title: String,
    pub title_visible: bool,
    pub font_family: PlotFontFamily,
    pub x_label_area_size: u32,
    pub y_label_area_size: u32,
    pub margin: u32,
    pub title_font_size: u32,
    pub title_font_color: Color,
}

/// A text annotation anchored to data coordinates and rendered in preview/export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotAnnotation {
    pub id: u64,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub font_size: u32,
    pub color: Color,
}

/// One linked-axis plot panel inside a publication figure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotPanel {
    pub id: u64,
    pub title: String,
    /// Series shown in this panel. Empty means no series are assigned.
    pub series_ids: Vec<SeriesId>,
}

/// Portable appearance-only preset, independent of tables and column bindings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotStyleDocument {
    pub format_version: u32,
    pub layout: LayoutConfig,
    pub x_axis: AxisStyle,
    pub y_axis: AxisStyle,
    pub legend: LegendConfig,
    /// Applied by series order; extra target series keep their current appearance.
    pub series: Vec<SeriesStyle>,
    #[serde(default)]
    pub custom_font_asset_base64: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxisStyle {
    pub axis_title_font_size: u32,
    pub label_font_size: u32,
    pub ticks: TickConfig,
}

/// Font families shared by chart text in preview and export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlotFontFamily {
    SansSerif,
    Serif,
    Monospace,
    /// Legacy name-only custom font reference. Prefer `CustomAsset` for stable identity.
    Custom(String),
    /// Custom font bound to both its user-facing family name and content fingerprint.
    CustomAsset {
        family: String,
        asset_id: String,
    },
}

impl PlotFontFamily {
    pub fn plotters_name(&self) -> String {
        match self {
            Self::SansSerif => crate::font::SANS.to_owned(),
            Self::Serif => crate::font::SERIF.to_owned(),
            Self::Monospace => crate::font::MONO.to_owned(),
            Self::Custom(name) => crate::font::legacy_renderer_family(name),
            Self::CustomAsset { asset_id, .. } => crate::font::renderer_family(asset_id),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::SansSerif => "Sans Serif".to_owned(),
            Self::Serif => "Serif".to_owned(),
            Self::Monospace => "Monospace".to_owned(),
            Self::Custom(name) => name.clone(),
            Self::CustomAsset { family, .. } => family.clone(),
        }
    }
}

/// RGBA color in 8-bit channels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Visual style of one plotted series.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeriesStyle {
    pub color: Color,
    pub line_visible: bool,
    pub line_width: f32,
    pub line_style: LineStyle,
    #[serde(default)]
    pub line_interpolation: LineInterpolation,
    #[serde(default)]
    pub area_baseline: Option<f64>,
    #[serde(default)]
    pub area_fill_visible: bool,
    pub marker: Option<MarkerStyle>,
    #[serde(default = "default_marker_preset")]
    pub marker_preset: MarkerStyle,
}

fn default_marker_preset() -> MarkerStyle {
    MarkerStyle {
        shape: MarkerShape::Circle,
        size: 3.0,
    }
}

/// Shape used to interpolate between consecutive series points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineInterpolation {
    Linear,
    StepAfter,
}

impl Default for LineInterpolation {
    fn default() -> Self {
        Self::Linear
    }
}

/// Line pattern used for a series.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
}

/// Marker configuration for point rendering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarkerStyle {
    pub shape: MarkerShape,
    pub size: f32,
}

/// Marker shape variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerShape {
    Circle,
    Square,
    Triangle,
    Cross,
}

/// Axis selector for generic axis actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisKind {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorBarSide {
    Lower,
    Upper,
}

/// Supported export image formats.
#[derive(Clone, Copy, Debug)]
pub enum ImageFormat {
    Png,
    Svg,
}

/// Export output pixel dimensions.
#[derive(Clone, Copy, Debug)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalUnit {
    Millimeters,
    Inches,
}

/// Publication target dimensions and raster resolution.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PublicationDimensions {
    pub width: f64,
    pub height: f64,
    pub unit: PhysicalUnit,
    pub dpi: u32,
}

impl Default for PublicationDimensions {
    fn default() -> Self {
        Self {
            width: 180.0,
            height: 120.0,
            unit: PhysicalUnit::Millimeters,
            dpi: 300,
        }
    }
}

impl PublicationDimensions {
    pub fn pixel_size(self) -> Result<ImageSize, String> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.0
            || self.height <= 0.0
            || !(36..=4800).contains(&self.dpi)
        {
            return Err("Export width, height, or DPI is outside the supported range".to_owned());
        }
        let mm_per_unit = match self.unit {
            PhysicalUnit::Millimeters => 1.0,
            PhysicalUnit::Inches => 25.4,
        };
        let pixels_per_mm = self.dpi as f64 / 25.4;
        let width = (self.width * mm_per_unit * pixels_per_mm).round();
        let height = (self.height * mm_per_unit * pixels_per_mm).round();
        if !width.is_finite()
            || !height.is_finite()
            || width < 1.0
            || height < 1.0
            || width > 16_000.0
            || height > 16_000.0
            || width * height > 64_000_000.0
        {
            return Err("Export dimensions exceed the 16,000 px / 64 MP safety limit".to_owned());
        }
        Ok(ImageSize {
            width: width as u32,
            height: height as u32,
        })
    }

    pub fn size_in_millimeters(self) -> (f64, f64) {
        let scale = match self.unit {
            PhysicalUnit::Millimeters => 1.0,
            PhysicalUnit::Inches => 25.4,
        };
        (self.width * scale, self.height * scale)
    }

    pub fn size_in_points(self) -> (f64, f64) {
        let (width_mm, height_mm) = self.size_in_millimeters();
        (width_mm / 25.4 * 72.0, height_mm / 25.4 * 72.0)
    }
}
