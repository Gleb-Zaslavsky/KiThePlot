//! Core reusable types for plot configuration and styling.
//!
//! These types are intentionally renderer-agnostic and form the stable contract
//! between UI/controller logic and rendering backends.

/// Stable identifier of a plotted series.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SeriesId(pub u64);

/// Pair of x/y axis configurations.
#[derive(Clone, Debug)]
pub struct AxesConfig {
    pub x: AxisConfig,
    pub y: AxisConfig,
}

/// Full axis setup edited by axis controls.
#[derive(Clone, Debug)]
pub struct AxisConfig {
    pub label: String,
    pub axis_title_font_size: u32,
    pub label_font_size: u32,
    pub scale: ScaleType,
    pub range: RangePolicy,
    pub ticks: TickConfig,
}

/// Axis transformation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleType {
    Linear,
    Log10,
    LogE,
}

/// Axis range policy (`Auto` from data or fixed manual bounds).
#[derive(Clone, Debug)]
pub enum RangePolicy {
    Auto,
    Manual { min: f64, max: f64 },
}

/// Tick density controls used by mesh configuration.
#[derive(Clone, Debug)]
pub struct TickConfig {
    pub major_step: Option<f64>,
    pub minor_per_major: u16,
}

/// Chart legend configuration.
#[derive(Clone, Debug)]
pub struct LegendConfig {
    pub visible: bool,
    pub title: Option<String>,
    pub position: LegendPosition,
    pub font_size: u32,
    pub font_color: Color,
}

/// Legend position inside plot area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Layout-level settings of the plot area and title style.
#[derive(Clone, Debug)]
pub struct LayoutConfig {
    pub title: String,
    pub x_label_area_size: u32,
    pub y_label_area_size: u32,
    pub margin: u32,
    pub title_font_size: u32,
    pub title_font_color: Color,
}

/// RGBA color in 8-bit channels.
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Visual style of one plotted series.
#[derive(Clone, Debug)]
pub struct SeriesStyle {
    pub color: Color,
    pub line_width: f32,
    pub line_style: LineStyle,
    pub marker: Option<MarkerStyle>,
}

/// Line pattern used for a series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
}

/// Marker configuration for point rendering.
#[derive(Clone, Debug)]
pub struct MarkerStyle {
    pub shape: MarkerShape,
    pub size: f32,
}

/// Marker shape variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
