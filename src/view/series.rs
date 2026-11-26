use crate::model::{
    Color, LegendConfig, LegendPosition, LineInterpolation, LineStyle, MarkerShape, MarkerStyle,
    PlotAnnotation, PlotModel, RangePolicy, ScaleType, TickConfig, TickNumberFormat,
};
use plotters::coord::Shift;
use plotters::coord::types::RangedCoordf64;
use plotters::prelude::*;
use plotters::style::Color as PlottersColor;
use plotters::style::text_anchor::{HPos, Pos, VPos};

pub(crate) fn resolve_data_range(
    policy: &RangePolicy,
    points: impl IntoIterator<Item = (f64, f64)>,
    is_x: bool,
    fallback: std::ops::Range<f64>,
    scale: ScaleType,
) -> std::ops::Range<f64> {
    if let RangePolicy::Manual { min, max } = policy {
        let transform = |value: f64| match scale {
            ScaleType::Linear => Some(value),
            ScaleType::Log10 | ScaleType::Log10Axis if value > 0.0 => Some(value.log10()),
            ScaleType::LogE if value > 0.0 => Some(value.ln()),
            _ => None,
        };
        return match (transform(*min), transform(*max)) {
            (Some(min), Some(max)) if min.is_finite() && max.is_finite() && min < max => min..max,
            _ => fallback,
        };
    }

    let (mut min_value, mut max_value) = (f64::INFINITY, f64::NEG_INFINITY);
    for (x, y) in points {
        let value = if is_x { x } else { y };
        if value.is_finite() {
            min_value = min_value.min(value);
            max_value = max_value.max(value);
        }
    }
    if !min_value.is_finite() || !max_value.is_finite() {
        return fallback;
    }

    let span = max_value - min_value;
    let magnitude = min_value.abs().max(max_value.abs());
    let precision_padding = magnitude * f64::EPSILON * 2.0;
    let padding = if span > 0.0 {
        (span * 0.05).max(precision_padding)
    } else {
        0.1_f64.max(precision_padding)
    };
    (min_value - padding)..(max_value + padding)
}

pub(crate) fn estimate_tick_label_width(
    range: &std::ops::Range<f64>,
    count: usize,
    font_family: &str,
    font_size: u32,
    step: f64,
    number_format: TickNumberFormat,
    scale: ScaleType,
) -> u32 {
    crate::font::ensure_plotters_fonts();
    let count = count.clamp(2, 100);
    let font = (font_family, font_size.max(1)).into_font();
    let widest = (0..count)
        .map(|index| {
            let value = range.start + (range.end - range.start) * index as f64 / (count - 1) as f64;
            let label = format_axis_tick(value, step, number_format, scale);
            font.layout_box(&label)
                .ok()
                .map(|((min_x, _), (max_x, _))| (max_x - min_x).max(0) as u32)
                .unwrap_or_else(|| (label.chars().count() as f32 * font_size as f32 * 0.62) as u32)
        })
        .max()
        .unwrap_or(1);
    widest + 12
}

pub(crate) fn resolve_label_areas(
    y_range: &std::ops::Range<f64>,
    y_ticks: &TickConfig,
    y_scale: ScaleType,
    font_family: &str,
    x_axis_title: &str,
    y_axis_title: &str,
    configured_x: u32,
    configured_y: u32,
    x_label_font: u32,
    y_label_font: u32,
    x_title_font: u32,
    y_title_font: u32,
) -> (u32, u32) {
    crate::font::ensure_plotters_fonts();
    let x_tick_height = measure_text(font_family, x_label_font, "0123456789")
        .map(|(_, height)| height)
        .unwrap_or(x_label_font);
    let x_title_height = measure_text(font_family, x_title_font, x_axis_title)
        .map(|(_, height)| height)
        .unwrap_or(x_title_font);
    let y_title_height = measure_text(font_family, y_title_font, y_axis_title)
        .map(|(_, height)| height)
        .unwrap_or(y_title_font);
    let x_area = configured_x.max(x_tick_height + x_title_height + 30);
    let y_label_count = labels_from_step(y_range, y_ticks.major_step).unwrap_or(5);
    let y_step = y_ticks.major_step.unwrap_or_else(|| {
        if y_scale == ScaleType::Log10Axis {
            1.0
        } else {
            nice_step(y_range, y_label_count)
        }
    });
    let y_area = configured_y
        .max((y_label_font as f32 * 1.6) as u32 + 16)
        .max(y_title_font + 28)
        .max(y_label_font + y_title_font + 28)
        .max(
            estimate_tick_label_width(
                y_range,
                y_label_count,
                font_family,
                y_label_font,
                y_step,
                y_ticks.number_format,
                y_scale,
            ) + y_title_height
                + 16,
        );
    (x_area, y_area)
}

fn measure_text(font_family: &str, font_size: u32, text: &str) -> Option<(u32, u32)> {
    let font = (font_family, font_size.max(1)).into_font();
    let ((min_x, min_y), (max_x, max_y)) = font.layout_box(text).ok()?;
    Some(((max_x - min_x).max(0) as u32, (max_y - min_y).max(0) as u32))
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct TextBounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

#[cfg(test)]
fn centered_text_bounds(center: (f32, f32), size: (f32, f32)) -> TextBounds {
    TextBounds {
        left: center.0 - size.0 / 2.0,
        top: center.1 - size.1 / 2.0,
        right: center.0 + size.0 / 2.0,
        bottom: center.1 + size.1 / 2.0,
    }
}

fn clamp_text_center(center: (f32, f32), size: (f32, f32), canvas: (u32, u32)) -> (f32, f32) {
    let clamp_axis = |value: f32, extent: f32, limit: u32| {
        if extent >= limit as f32 {
            limit as f32 / 2.0
        } else {
            value.clamp(extent / 2.0, limit as f32 - extent / 2.0)
        }
    };
    (
        clamp_axis(center.0, size.0, canvas.0),
        clamp_axis(center.1, size.1, canvas.1),
    )
}

fn labels_from_step(range: &std::ops::Range<f64>, step: Option<f64>) -> Option<usize> {
    let step = step?;
    if !step.is_finite() || step <= 0.0 {
        return None;
    }
    Some((((range.end - range.start) / step).ceil() as usize).clamp(2, 100))
}

pub(crate) fn resolve_auto_tick_steps(
    x_range: &std::ops::Range<f64>,
    y_range: &std::ops::Range<f64>,
    x_ticks: &TickConfig,
    y_ticks: &TickConfig,
    canvas_size: (u32, u32),
    x_label_area: u32,
    y_label_area: u32,
    caption_height: u32,
    margins: ChartMargins,
    x_font_size: u32,
    y_font_size: u32,
) -> (TickConfig, TickConfig) {
    let plot_width = canvas_size
        .0
        .saturating_sub(margins.left + margins.right + y_label_area)
        .max(1);
    let plot_height = canvas_size
        .1
        .saturating_sub(margins.top + margins.bottom + x_label_area + caption_height)
        .max(1);
    let x_labels = (plot_width / (x_font_size.max(8) * 4)).clamp(2, 12) as usize;
    let y_labels = (plot_height / (y_font_size.max(8) * 2)).clamp(2, 12) as usize;

    let mut resolved_x = x_ticks.clone();
    let mut resolved_y = y_ticks.clone();
    if resolved_x.major_step.is_none() {
        resolved_x.major_step = Some(nice_step(x_range, x_labels));
    }
    if resolved_y.major_step.is_none() {
        resolved_y.major_step = Some(nice_step(y_range, y_labels));
    }
    (resolved_x, resolved_y)
}

fn nice_step(range: &std::ops::Range<f64>, max_labels: usize) -> f64 {
    let span = (range.end - range.start).abs();
    if !span.is_finite() || span <= 0.0 {
        return 1.0;
    }
    let raw_step = span / (max_labels.saturating_sub(1).max(1) as f64);
    let magnitude = 10.0_f64.powf(raw_step.log10().floor());
    let normalized = raw_step / magnitude;
    let nice_fraction = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    let step = nice_fraction * magnitude;
    if step <= span {
        step
    } else {
        let span_magnitude = 10.0_f64.powf(span.log10().floor());
        let span_fraction = span / span_magnitude;
        let fraction = if span_fraction >= 5.0 {
            5.0
        } else if span_fraction >= 2.0 {
            2.0
        } else {
            1.0
        };
        fraction * span_magnitude
    }
}

pub(crate) fn draw_tick_labels<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    chart: &ChartContext<'_, DB, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    x_range: std::ops::Range<f64>,
    y_range: std::ops::Range<f64>,
    x_ticks: &TickConfig,
    y_ticks: &TickConfig,
    x_scale: ScaleType,
    y_scale: ScaleType,
    x_font_size: u32,
    y_font_size: u32,
    font_family: &str,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let (left, top) = chart.backend_coord(&(x_range.start, y_range.end));
    let (right, bottom) = chart.backend_coord(&(x_range.end, y_range.start));
    let x_style = TextStyle::from((font_family, x_font_size).into_font())
        .pos(Pos::new(HPos::Center, VPos::Top));
    let y_style = TextStyle::from((font_family, y_font_size).into_font())
        .pos(Pos::new(HPos::Right, VPos::Center));
    let max_x_labels = ((right - left) / (x_font_size.max(8) as i32 * 4)).clamp(2, 100) as usize;
    let max_y_labels = ((bottom - top) / (y_font_size.max(8) as i32 * 2)).clamp(2, 100) as usize;
    let max_x_grid = ((right - left) / 4).clamp(2, 256) as usize;
    let max_y_grid = ((bottom - top) / 4).clamp(2, 256) as usize;

    if x_ticks.major_ticks_visible {
        let length = x_ticks.major_tick_length.max(0.0).round() as i32;
        for (value, _) in tick_values(&x_range, x_ticks, max_x_labels) {
            let (x, _) = chart.backend_coord(&(value, y_range.start));
            area.draw(&PathElement::new(
                vec![(x, bottom), (x, bottom + length)],
                BLACK.stroke_width(1),
            ))?;
        }
    }
    if y_ticks.major_ticks_visible {
        let length = y_ticks.major_tick_length.max(0.0).round() as i32;
        for (value, _) in tick_values(&y_range, y_ticks, max_y_labels) {
            let (_, y) = chart.backend_coord(&(x_range.start, value));
            area.draw(&PathElement::new(
                vec![(left, y), (left - length, y)],
                BLACK.stroke_width(1),
            ))?;
        }
    }
    if x_ticks.minor_ticks_visible && x_ticks.minor_per_major > 0 {
        let majors = major_positions(&x_range, x_ticks, max_x_grid);
        let length = x_ticks.minor_tick_length.max(0.0).round() as i32;
        for value in minor_positions(&x_range, x_ticks, &majors, max_x_grid) {
            let (x, _) = chart.backend_coord(&(value, y_range.start));
            area.draw(&PathElement::new(
                vec![(x, bottom), (x, bottom + length)],
                BLACK.stroke_width(1),
            ))?;
        }
    }
    if y_ticks.minor_ticks_visible && y_ticks.minor_per_major > 0 {
        let majors = major_positions(&y_range, y_ticks, max_y_grid);
        let length = y_ticks.minor_tick_length.max(0.0).round() as i32;
        for value in minor_positions(&y_range, y_ticks, &majors, max_y_grid) {
            let (_, y) = chart.backend_coord(&(x_range.start, value));
            area.draw(&PathElement::new(
                vec![(left, y), (left - length, y)],
                BLACK.stroke_width(1),
            ))?;
        }
    }

    for (value, step) in tick_values(&x_range, x_ticks, max_x_labels) {
        let (x, _) = chart.backend_coord(&(value, y_range.start));
        area.draw(&Text::new(
            format_axis_tick(value, step, x_ticks.number_format, x_scale),
            (x, bottom + 4),
            x_style.clone(),
        ))?;
    }
    for (value, step) in tick_values(&y_range, y_ticks, max_y_labels) {
        let (_, y) = chart.backend_coord(&(x_range.start, value));
        area.draw(&Text::new(
            format_axis_tick(value, step, y_ticks.number_format, y_scale),
            (left - 6, y),
            y_style.clone(),
        ))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AxisTitleGeometry {
    pub x_center: (f32, f32),
    pub x_size: (f32, f32),
    pub y_center: (f32, f32),
    pub y_size: (f32, f32),
}

pub(crate) fn axis_title_geometry(
    canvas: (u32, u32),
    x_label: &str,
    y_label: &str,
    x_font_size: u32,
    y_font_size: u32,
    x_label_area: u32,
    y_label_area: u32,
    caption_height: u32,
    margins: ChartMargins,
    font_family: &str,
    x_anchor: Option<[f32; 2]>,
    y_anchor: Option<[f32; 2]>,
) -> AxisTitleGeometry {
    let (width, height) = canvas;
    let left = margins.left as i32 + y_label_area as i32;
    let right = width as i32 - margins.right as i32;
    let top = margins.top as i32 + caption_height as i32;
    let bottom = height as i32 - margins.bottom as i32 - x_label_area as i32;
    let x_extent = measure_text(font_family, x_font_size.max(8), x_label)
        .unwrap_or((x_font_size.max(8), x_font_size.max(8)));
    let y_unrotated_extent = measure_text(font_family, y_font_size.max(8), y_label)
        .unwrap_or((y_font_size.max(8), y_font_size.max(8)));
    // A 270 degree rotation swaps the measured width and height.
    let y_extent = (y_unrotated_extent.1, y_unrotated_extent.0);
    let x_center = x_anchor
        .map(|anchor| (anchor[0] * width as f32, anchor[1] * height as f32))
        .unwrap_or((
            (left + right) as f32 / 2.0,
            height as f32 - margins.bottom as f32 - x_extent.1 as f32 / 2.0,
        ));
    let y_center = y_anchor
        .map(|anchor| (anchor[0] * width as f32, anchor[1] * height as f32))
        .unwrap_or((
            margins.left as f32 + y_extent.0 as f32 / 2.0 + 4.0,
            (top + bottom) as f32 / 2.0,
        ));
    let x_center = clamp_text_center(
        x_center,
        (x_extent.0 as f32, x_extent.1 as f32),
        (width, height),
    );
    let y_center = clamp_text_center(
        y_center,
        (y_extent.0 as f32, y_extent.1 as f32),
        (width, height),
    );
    AxisTitleGeometry {
        x_center,
        x_size: (x_extent.0 as f32, x_extent.1 as f32),
        y_center,
        y_size: (y_extent.0 as f32, y_extent.1 as f32),
    }
}

pub(crate) fn draw_axis_titles<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    x_label: &str,
    y_label: &str,
    x_font_size: u32,
    y_font_size: u32,
    x_label_area: u32,
    y_label_area: u32,
    caption_height: u32,
    margins: ChartMargins,
    font_family: &str,
    x_anchor: Option<[f32; 2]>,
    y_anchor: Option<[f32; 2]>,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let geometry = axis_title_geometry(
        area.dim_in_pixel(),
        x_label,
        y_label,
        x_font_size,
        y_font_size,
        x_label_area,
        y_label_area,
        caption_height,
        margins,
        font_family,
        x_anchor,
        y_anchor,
    );
    let x_style = TextStyle::from((font_family, x_font_size.max(8)).into_font())
        .pos(Pos::new(HPos::Center, VPos::Center));
    let y_style = TextStyle::from((font_family, y_font_size.max(8)).into_font())
        .transform(FontTransform::Rotate270)
        .pos(Pos::new(HPos::Center, VPos::Center));
    let x_position = (
        geometry.x_center.0.round() as i32,
        geometry.x_center.1.round() as i32,
    );
    let y_position = (
        geometry.y_center.0.round() as i32,
        geometry.y_center.1.round() as i32,
    );
    area.draw(&Text::new(x_label.to_owned(), x_position, x_style))?;
    area.draw(&Text::new(y_label.to_owned(), y_position, y_style))?;
    Ok(())
}

fn tick_values(
    range: &std::ops::Range<f64>,
    ticks: &TickConfig,
    max_positions: usize,
) -> Vec<(f64, f64)> {
    let Some(step) = effective_major_step(range, ticks, max_positions) else {
        return Vec::new();
    };
    let first = (range.start / step).ceil() * step;
    if !first.is_finite() {
        return Vec::new();
    }
    let total = ((range.end - first) / step).floor().max(0.0);
    let count = ((total as usize).saturating_add(1)).min(max_positions.max(1));
    (0..count)
        .map(|index| (first + index as f64 * step, step))
        .filter(|(value, _)| value.is_finite() && *value <= range.end)
        .collect()
}

fn effective_major_step(
    range: &std::ops::Range<f64>,
    ticks: &TickConfig,
    max_positions: usize,
) -> Option<f64> {
    let span = range.end - range.start;
    if !span.is_finite() || span <= 0.0 {
        return None;
    }
    let base_step = ticks
        .major_step
        .filter(|step| step.is_finite() && *step > 0.0)
        .unwrap_or(span / 5.0);
    if !base_step.is_finite() || base_step <= 0.0 {
        return None;
    }
    let max_positions = max_positions.max(1);
    let estimated_positions = (span / base_step).ceil();
    let stride = if estimated_positions.is_finite() && estimated_positions > max_positions as f64 {
        (estimated_positions / max_positions as f64).ceil()
    } else {
        1.0
    };
    let effective = base_step * stride;
    (effective.is_finite() && effective > 0.0).then_some(effective)
}

fn format_tick(value: f64, step: f64, number_format: TickNumberFormat) -> String {
    match number_format {
        TickNumberFormat::Auto => {
            if value.abs() >= 1_000_000.0 || (value != 0.0 && value.abs() < 0.0001) {
                let magnitude = value.abs().log10().floor();
                let normalized_step_log = step.log10() - magnitude;
                let decimals = if normalized_step_log.is_finite() {
                    (-normalized_step_log.floor() as i32).clamp(2, 16) as usize
                } else {
                    2
                };
                return format!("{value:.decimals$e}");
            }
            let decimals = if step >= 1.0 {
                0
            } else {
                (-step.log10().floor() as i32).clamp(0, 16) as usize
            };
            format!("{value:.decimals$}")
        }
        TickNumberFormat::Fixed { decimals } => {
            let decimals = decimals.min(8) as usize;
            format!("{value:.decimals$}")
        }
        TickNumberFormat::Scientific { decimals } => {
            let decimals = decimals.min(8) as usize;
            format!("{value:.decimals$e}")
        }
    }
}

fn format_axis_tick(
    coordinate: f64,
    coordinate_step: f64,
    number_format: TickNumberFormat,
    scale: ScaleType,
) -> String {
    if scale != ScaleType::Log10Axis {
        return format_tick(coordinate, coordinate_step, number_format);
    }
    let value = 10.0_f64.powf(coordinate);
    if !value.is_finite() || value <= 0.0 {
        return format_tick(coordinate, coordinate_step, number_format);
    }
    let step = value * (10.0_f64.powf(coordinate_step) - 1.0);
    let step = if step.is_finite() && step > 0.0 {
        step
    } else {
        value
    };
    format_tick(value, step, number_format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_scientific_ticks_distinguish_values_smaller_than_the_magnitude() {
        let first = format_tick(1_000_000.0, 1.0, TickNumberFormat::Auto);
        let second = format_tick(1_000_001.0, 1.0, TickNumberFormat::Auto);
        assert_ne!(first, second);
        assert_eq!(first, "1.000000e6");
        let close_first = format_tick(1_000_000.0, 0.001, TickNumberFormat::Auto);
        let close_second = format_tick(1_000_000.001, 0.001, TickNumberFormat::Auto);
        assert_ne!(close_first, close_second);
        let huge_first = format_tick(1.0e300, 1.0e285, TickNumberFormat::Auto);
        let huge_second = format_tick(1.0e300 + 1.0e285, 1.0e285, TickNumberFormat::Auto);
        assert_ne!(huge_first, huge_second);
        let tiny_first = format_tick(1.0e-300, 1.0e-310, TickNumberFormat::Auto);
        let tiny_second = format_tick(1.0e-300 + 1.0e-310, 1.0e-310, TickNumberFormat::Auto);
        assert_ne!(tiny_first, tiny_second);
    }

    #[test]
    fn logarithmic_axis_labels_original_values_while_log_transform_labels_transformed_values() {
        for (coordinate, expected) in [(0.0, "1"), (1.0, "10"), (2.0, "100")] {
            assert_eq!(
                format_axis_tick(
                    coordinate,
                    1.0,
                    TickNumberFormat::Auto,
                    ScaleType::Log10Axis
                ),
                expected
            );
        }
        assert_eq!(
            format_axis_tick(2.0, 1.0, TickNumberFormat::Auto, ScaleType::Log10),
            "2"
        );
        assert_eq!(scale_suffix(ScaleType::Log10Axis), "");
        assert_eq!(scale_suffix(ScaleType::Log10), " [log10(data)]");
    }

    #[test]
    fn conventional_log_axis_uses_one_decade_major_ticks_by_default() {
        let mut model = PlotModel::default();
        model.axes.x.scale = ScaleType::Log10Axis;
        model.axes.y.scale = ScaleType::Log10Axis;
        let config = ChartRenderConfig::from_model(
            &model,
            0.0..3.0,
            -1.0..2.0,
            ChartMargins {
                top: 0,
                bottom: 0,
                left: 0,
                right: 0,
            },
            0,
            0,
            0,
        );
        assert_eq!(config.x_ticks.major_step, Some(1.0));
        assert_eq!(config.y_ticks.major_step, Some(1.0));
        assert_eq!(config.x_scale, ScaleType::Log10Axis);
        assert_eq!(config.y_scale, ScaleType::Log10Axis);

        let narrow = ChartRenderConfig::from_model(
            &model,
            2.0_f64.log10()..8.0_f64.log10(),
            2.0_f64.log10()..8.0_f64.log10(),
            ChartMargins {
                top: 0,
                bottom: 0,
                left: 0,
                right: 0,
            },
            0,
            0,
            0,
        );
        let ticks = tick_values(&narrow.x_range, &narrow.x_ticks, 100);
        assert!(ticks.len() >= 5, "narrow log ticks: {ticks:?}");
        assert!(
            ticks
                .iter()
                .any(|(coordinate, _)| (0.65..=0.75).contains(coordinate))
        );
    }

    #[test]
    fn dense_manual_grid_generation_has_a_fixed_work_bound() {
        let ticks = TickConfig {
            major_step: Some(0.000_000_1),
            number_format: TickNumberFormat::Auto,
            minor_per_major: u16::MAX,
            major_ticks_visible: true,
            minor_ticks_visible: true,
            major_tick_length: 5.0,
            minor_tick_length: 3.0,
            major_grid_visible: true,
            minor_grid_visible: true,
            major_grid_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            major_grid_width: 1.0,
            minor_grid_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            minor_grid_width: 1.0,
        };
        let range = 0.0..100.0;
        let majors = major_positions(&range, &ticks, 256);
        let minors = minor_positions(&range, &ticks, &majors, 256);
        assert!(majors.len() <= 256);
        assert!(minors.len() <= 257 * 32);

        let pixel_bounded = major_positions(&range, &ticks, 10);
        let bounded_minors = minor_positions(&range, &ticks, &pixel_bounded, 10);
        assert!(pixel_bounded.len() <= 10);
        assert!(bounded_minors.len() <= 10);
        assert!(pixel_bounded.first().unwrap() < pixel_bounded.last().unwrap());
        assert!(tick_values(&range, &ticks, 6).len() <= 6);
    }

    #[test]
    fn selected_font_glyph_metrics_distinguish_narrow_and_wide_text() {
        crate::font::ensure_plotters_fonts();
        let narrow = measure_text(crate::font::SANS, 20, "iiii").unwrap().0;
        let wide = measure_text(crate::font::SANS, 20, "WWWW").unwrap().0;
        assert!(wide > narrow);
    }

    #[test]
    fn automatic_axis_title_rectangles_stay_inside_the_canvas_and_outside_tick_bands() {
        crate::font::ensure_plotters_fonts();
        let canvas = (800, 600);
        let margins = ChartMargins {
            top: 4,
            bottom: 4,
            left: 4,
            right: 4,
        };
        let y_range = -2_000_000.0..-1_000_000.0;
        let mut ticks = PlotModel::default().axes.y.ticks;
        ticks.major_step = Some(500_000.0);
        ticks.number_format = TickNumberFormat::Fixed { decimals: 0 };
        let x_title = "Время, мс";
        let y_title = "Амплитуда, мВ";
        let (x_area, y_area) = resolve_label_areas(
            &y_range,
            &ticks,
            ScaleType::Linear,
            crate::font::SANS,
            x_title,
            y_title,
            0,
            0,
            26,
            26,
            20,
            20,
        );
        let x_title_size = measure_text(crate::font::SANS, 20, x_title).unwrap();
        let y_title_size = measure_text(crate::font::SANS, 20, y_title).unwrap();
        let y_rotated_size = (y_title_size.1, y_title_size.0);
        let x_title_center = (
            (margins.left + y_area + canvas.0 - margins.right) as f32 / 2.0,
            canvas.1 as f32 - margins.bottom as f32 - x_title_size.1 as f32 / 2.0,
        );
        let y_title_center = (
            margins.left as f32 + y_rotated_size.0 as f32 / 2.0 + 4.0,
            (margins.top + canvas.1 - margins.bottom - x_area) as f32 / 2.0,
        );
        let x_bounds = centered_text_bounds(
            x_title_center,
            (x_title_size.0 as f32, x_title_size.1 as f32),
        );
        let y_bounds = centered_text_bounds(
            y_title_center,
            (y_rotated_size.0 as f32, y_rotated_size.1 as f32),
        );
        let y_tick_width = estimate_tick_label_width(
            &y_range,
            3,
            crate::font::SANS,
            26,
            500_000.0,
            TickNumberFormat::Fixed { decimals: 0 },
            ScaleType::Linear,
        ) as f32;
        let y_tick_left = margins.left as f32 + y_area as f32 - 6.0 - y_tick_width;
        let x_tick_height = measure_text(crate::font::SANS, 26, "0123456789").unwrap().1 as f32;
        let x_tick_bottom =
            canvas.1 as f32 - margins.bottom as f32 - x_area as f32 + 4.0 + x_tick_height;

        assert!(y_bounds.left >= 0.0);
        assert!(y_bounds.top >= 0.0 && y_bounds.bottom <= canvas.1 as f32);
        assert!(y_bounds.right + 4.0 <= y_tick_left);
        assert!(x_bounds.left >= 0.0 && x_bounds.right <= canvas.0 as f32);
        assert!(x_tick_bottom + 4.0 <= x_bounds.top);
    }

    #[test]
    fn manually_anchored_axis_titles_are_clamped_to_visible_canvas_bounds() {
        let canvas = (400, 300);
        let text_size = (140.0, 24.0);
        let center = clamp_text_center((-50.0, 400.0), text_size, canvas);
        let bounds = centered_text_bounds(center, text_size);
        assert!(bounds.left >= 0.0);
        assert!(bounds.top >= 0.0);
        assert!(bounds.right <= canvas.0 as f32);
        assert!(bounds.bottom <= canvas.1 as f32);
    }
}

pub(crate) struct LegendItem<'a> {
    pub name: &'a str,
    pub color: Color,
    pub line_visible: bool,
    pub line_style: LineStyle,
    pub line_interpolation: LineInterpolation,
    pub area_fill: bool,
    pub marker: Option<&'a MarkerStyle>,
}

pub(crate) struct LegendLayout {
    pub width: f32,
    pub height: f32,
    pub rows_per_column: usize,
    pub row_height: f32,
    pub column_offsets: Vec<f32>,
    pub title_row: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChartMargins {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl ChartMargins {
    pub fn uniform(value: u32) -> Self {
        Self {
            top: value,
            bottom: value,
            left: value,
            right: value,
        }
    }
}

pub(crate) fn chart_margins_for_legend(
    base: u32,
    config: &LegendConfig,
    labels: &[&str],
    caption_height: u32,
    bottom_label_area: u32,
) -> ChartMargins {
    let mut margins = ChartMargins::uniform(base);
    if !config.visible || labels.is_empty() {
        return margins;
    }
    let layout = legend_layout(
        config.title.as_deref(),
        labels,
        config.font_size,
        config.padding,
        config.row_spacing,
        config.sample_length,
        config.columns,
        config.column_spacing,
    );
    let gap = 12;
    match config.position {
        LegendPosition::OutsideLeft => margins.left = base + layout.width.ceil() as u32 + gap,
        LegendPosition::OutsideRight => margins.right = base + layout.width.ceil() as u32 + gap,
        LegendPosition::OutsideTop => {
            let title_gap = if caption_height > 0 { 10 } else { 0 };
            margins.top = base + caption_height + title_gap + layout.height.ceil() as u32 + gap
        }
        LegendPosition::OutsideBottom => {
            margins.bottom = base + layout.height.ceil() as u32 + gap + bottom_label_area
        }
        _ => {}
    }
    margins
}

pub(crate) fn legend_layout(
    title: Option<&str>,
    labels: &[&str],
    font_size: u32,
    padding: u32,
    row_spacing: u32,
    sample_length: u32,
    requested_columns: u32,
    column_spacing: u32,
) -> LegendLayout {
    let font_size = font_size.max(8) as f32;
    let columns = (requested_columns as usize).clamp(1, labels.len().max(1));
    let rows_per_column = labels.len().div_ceil(columns);
    let title_row = title.is_some_and(|value| !value.is_empty());
    let row_height = font_size + row_spacing as f32;
    let mut column_offsets = Vec::with_capacity(columns);
    let mut content_width = 0.0_f32;
    for column in 0..columns {
        let widest_chars = labels
            .iter()
            .enumerate()
            .filter(|(index, _)| index / rows_per_column.max(1) == column)
            .map(|(_, label)| label.chars().count())
            .max()
            .unwrap_or(1) as f32;
        column_offsets.push(padding as f32 + content_width);
        content_width += sample_length as f32 + 8.0 + widest_chars * font_size * 0.64;
        if column + 1 < columns {
            content_width += column_spacing as f32;
        }
    }
    let title_width = title
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count() as f32 * font_size * 0.64)
        .unwrap_or_default();
    let width = content_width.max(title_width) + padding as f32 * 2.0;
    let rows = rows_per_column + usize::from(title_row);
    let height = rows as f32 * row_height + padding as f32 * 2.0;
    LegendLayout {
        width,
        height,
        rows_per_column,
        row_height,
        column_offsets,
        title_row,
    }
}

pub(crate) struct SeriesRender<'a> {
    pub points: &'a [(f64, f64)],
    pub line_segments: &'a [Vec<(f64, f64)>],
    pub error_bars: &'a [ErrorBar],
    pub area_baseline: Option<f64>,
    pub color: Color,
    pub line_visible: bool,
    pub line_width: f32,
    pub line_style: LineStyle,
    pub line_interpolation: LineInterpolation,
    pub marker: Option<&'a MarkerStyle>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ErrorBar {
    pub center: (f64, f64),
    pub x_bounds: Option<(f64, f64)>,
    pub y_bounds: Option<(f64, f64)>,
}

#[derive(Default)]
pub(crate) struct ErrorBarSet {
    pub bars: Vec<ErrorBar>,
    pub omitted_log_x: usize,
    pub omitted_log_y: usize,
}

impl ErrorBar {
    pub(crate) fn range_points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        std::iter::once(self.center)
            .chain(
                self.x_bounds
                    .into_iter()
                    .flat_map(|(low, high)| [(low, self.center.1), (high, self.center.1)]),
            )
            .chain(
                self.y_bounds
                    .into_iter()
                    .flat_map(|(low, high)| [(self.center.0, low), (self.center.0, high)]),
            )
    }
}

pub(crate) struct ChartRenderConfig {
    pub x_range: std::ops::Range<f64>,
    pub y_range: std::ops::Range<f64>,
    pub x_label: String,
    pub y_label: String,
    pub x_ticks: TickConfig,
    pub y_ticks: TickConfig,
    pub x_scale: ScaleType,
    pub y_scale: ScaleType,
    pub x_label_font_size: u32,
    pub y_label_font_size: u32,
    pub x_title_font_size: u32,
    pub y_title_font_size: u32,
    pub x_title_anchor: Option<[f32; 2]>,
    pub y_title_anchor: Option<[f32; 2]>,
    pub font_family: String,
    pub margins: ChartMargins,
    pub caption_height: u32,
    pub x_label_area: u32,
    pub y_label_area: u32,
    pub annotations: Vec<PlotAnnotation>,
}

impl ChartRenderConfig {
    pub(crate) fn from_model(
        model: &PlotModel,
        x_range: std::ops::Range<f64>,
        y_range: std::ops::Range<f64>,
        margins: ChartMargins,
        caption_height: u32,
        x_label_area: u32,
        y_label_area: u32,
    ) -> Self {
        let mut x_ticks = model.axes.x.ticks.clone();
        let mut y_ticks = model.axes.y.ticks.clone();
        if model.axes.x.scale == ScaleType::Log10Axis && x_ticks.major_step.is_none() {
            x_ticks.major_step = Some(log_axis_auto_step(&x_range));
        }
        if model.axes.y.scale == ScaleType::Log10Axis && y_ticks.major_step.is_none() {
            y_ticks.major_step = Some(log_axis_auto_step(&y_range));
        }
        Self {
            x_range,
            y_range,
            x_label: format!("{}{}", model.axes.x.label, scale_suffix(model.axes.x.scale)),
            y_label: format!("{}{}", model.axes.y.label, scale_suffix(model.axes.y.scale)),
            x_ticks,
            y_ticks,
            x_scale: model.axes.x.scale,
            y_scale: model.axes.y.scale,
            x_label_font_size: model.axes.x.label_font_size,
            y_label_font_size: model.axes.y.label_font_size,
            x_title_font_size: model.axes.x.axis_title_font_size,
            y_title_font_size: model.axes.y.axis_title_font_size,
            x_title_anchor: model.axes.x.title_anchor,
            y_title_anchor: model.axes.y.title_anchor,
            font_family: model.layout.font_family.plotters_name(),
            margins,
            caption_height,
            x_label_area,
            y_label_area,
            annotations: model.annotations.clone(),
        }
    }
}

fn log_axis_auto_step(range: &std::ops::Range<f64>) -> f64 {
    let span = range.end - range.start;
    if span >= 0.9 {
        1.0
    } else if span >= 0.3 {
        0.1
    } else {
        nice_step(range, 6).max(f64::MIN_POSITIVE)
    }
}

pub(crate) fn scale_suffix(scale: ScaleType) -> &'static str {
    match scale {
        ScaleType::Linear => "",
        ScaleType::Log10 => " [log10(data)]",
        ScaleType::Log10Axis => "",
        ScaleType::LogE => " [ln(data)]",
    }
}

pub(crate) fn draw_chart_content<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    chart: &mut ChartContext<'_, DB, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    config: &ChartRenderConfig,
    series: &[SeriesRender<'_>],
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let (x_ticks, y_ticks) = resolve_auto_tick_steps(
        &config.x_range,
        &config.y_range,
        &config.x_ticks,
        &config.y_ticks,
        area.dim_in_pixel(),
        config.x_label_area,
        config.y_label_area,
        config.caption_height,
        config.margins,
        config.x_label_font_size,
        config.y_label_font_size,
    );
    let mut mesh = chart.configure_mesh();
    mesh.disable_mesh()
        .x_desc("")
        .y_desc("")
        .x_label_style((config.font_family.as_str(), config.x_label_font_size))
        .y_label_style((config.font_family.as_str(), config.y_label_font_size))
        .x_labels(0)
        .y_labels(0)
        .max_light_lines(0)
        .draw()?;
    draw_grid(
        chart,
        config.x_range.clone(),
        config.y_range.clone(),
        &x_ticks,
        &y_ticks,
    )?;
    draw_tick_labels(
        area,
        chart,
        config.x_range.clone(),
        config.y_range.clone(),
        &x_ticks,
        &y_ticks,
        config.x_scale,
        config.y_scale,
        config.x_label_font_size,
        config.y_label_font_size,
        &config.font_family,
    )?;
    draw_axis_titles(
        area,
        &config.x_label,
        &config.y_label,
        config.x_title_font_size,
        config.y_title_font_size,
        config.x_label_area,
        config.y_label_area,
        config.caption_height,
        config.margins,
        &config.font_family,
        config.x_title_anchor,
        config.y_title_anchor,
    )?;
    for series in series {
        if series.points.is_empty() {
            continue;
        }
        draw_series(chart, series, &config.x_range, &config.y_range)?;
    }
    for annotation in &config.annotations {
        let scaled = |value: f64, scale| match scale {
            ScaleType::Linear => value.is_finite().then_some(value),
            ScaleType::Log10 | ScaleType::Log10Axis => {
                (value.is_finite() && value > 0.0).then(|| value.log10())
            }
            ScaleType::LogE => (value.is_finite() && value > 0.0).then(|| value.ln()),
        };
        let (Some(x), Some(y)) = (
            scaled(annotation.x, config.x_scale),
            scaled(annotation.y, config.y_scale),
        ) else {
            continue;
        };
        if !config.x_range.contains(&x) || !config.y_range.contains(&y) {
            continue;
        }
        let color = RGBAColor(
            annotation.color.r,
            annotation.color.g,
            annotation.color.b,
            annotation.color.a as f64 / 255.0,
        );
        let style = (config.font_family.as_str(), annotation.font_size.max(4))
            .into_font()
            .color(&color)
            .pos(Pos::new(HPos::Center, VPos::Center));
        chart.draw_series(std::iter::once(Text::new(
            annotation.text.clone(),
            (x, y),
            style,
        )))?;
    }
    Ok(())
}

pub(crate) fn draw_custom_legend<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    title: Option<&str>,
    items: &[LegendItem<'_>],
    config: &LegendConfig,
    font_family: &str,
    outer_margin: u32,
    caption_height: u32,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    if items.is_empty() {
        return Ok(());
    }
    let (canvas_width, canvas_height) = area.dim_in_pixel();
    let title = title.filter(|title| !title.trim().is_empty());
    let font_size = config.font_size.max(8);
    let padding = config.padding as i32;
    let labels = items.iter().map(|item| item.name).collect::<Vec<_>>();
    let layout = legend_layout(
        title,
        &labels,
        font_size,
        config.padding,
        config.row_spacing,
        config.sample_length,
        config.columns,
        config.column_spacing,
    );
    let box_width = layout.width.ceil() as i32;
    let box_height = layout.height.ceil() as i32;
    let (x, y) = match config.position {
        LegendPosition::OutsideLeft => {
            (outer_margin as i32, (canvas_height as i32 - box_height) / 2)
        }
        LegendPosition::OutsideRight => (
            canvas_width as i32 - outer_margin as i32 - box_width,
            (canvas_height as i32 - box_height) / 2,
        ),
        LegendPosition::OutsideTop => (
            (canvas_width as i32 - box_width) / 2,
            outer_margin as i32 + caption_height as i32 + if caption_height > 0 { 10 } else { 0 },
        ),
        LegendPosition::OutsideBottom => (
            (canvas_width as i32 - box_width) / 2,
            canvas_height as i32 - outer_margin as i32 - box_height,
        ),
        _ => (
            (config.anchor[0].clamp(0.0, 1.0) * (canvas_width as i32 - box_width).max(0) as f32)
                as i32,
            (config.anchor[1].clamp(0.0, 1.0) * (canvas_height as i32 - box_height).max(0) as f32)
                as i32,
        ),
    };
    let background = RGBAColor(
        config.background_color.r,
        config.background_color.g,
        config.background_color.b,
        config.background_color.a as f64 / 255.0,
    );
    area.draw(&Rectangle::new(
        [(x, y), (x + box_width, y + box_height)],
        background.filled(),
    ))?;
    area.draw(&Rectangle::new(
        [(x, y), (x + box_width, y + box_height)],
        RGBAColor(
            config.border_color.r,
            config.border_color.g,
            config.border_color.b,
            config.border_color.a as f64 / 255.0,
        )
        .stroke_width(config.border_width.max(0.0).round() as u32),
    ))?;
    let text_color = RGBAColor(
        config.font_color.r,
        config.font_color.g,
        config.font_color.b,
        config.font_color.a as f64 / 255.0,
    );
    let text_style = (font_family, font_size.max(8))
        .into_font()
        .color(&text_color);
    if let Some(title) = title {
        area.draw(&Text::new(
            title.to_owned(),
            (x + padding, y + padding),
            text_style.clone(),
        ))?;
    }
    for (index, item) in items.iter().enumerate() {
        let column = index / layout.rows_per_column.max(1);
        let row = index % layout.rows_per_column.max(1);
        let row = row + usize::from(layout.title_row);
        let row_height = layout.row_height as i32;
        let center_y = y + padding + row as i32 * row_height + row_height / 2;
        let color = RGBAColor(
            item.color.r,
            item.color.g,
            item.color.b,
            item.color.a as f64 / 255.0,
        );
        let sample_start = x + layout.column_offsets[column].round() as i32;
        let sample_end = sample_start + config.sample_length as i32;
        let sample_center = (sample_start + sample_end) / 2;
        if item.area_fill {
            area.draw(&Rectangle::new(
                [(sample_start, center_y - 4), (sample_end, center_y + 4)],
                color.mix(0.2).filled(),
            ))?;
        }
        if item.line_visible {
            if item.line_interpolation == LineInterpolation::StepAfter {
                let step_points = [
                    (sample_start, center_y + 3),
                    (sample_center, center_y + 3),
                    (sample_center, center_y - 3),
                    (sample_end, center_y - 3),
                ];
                match item.line_style {
                    LineStyle::Solid => area.draw(&PathElement::new(
                        step_points.to_vec(),
                        color.stroke_width(2),
                    ))?,
                    LineStyle::Dashed => {
                        for (index, segment) in step_points.windows(2).enumerate() {
                            if index % 2 == 0 {
                                area.draw(&PathElement::new(
                                    segment.to_vec(),
                                    color.stroke_width(2),
                                ))?;
                            }
                        }
                    }
                    LineStyle::Dotted => {
                        for point in step_points.iter().step_by(2) {
                            area.draw(&Circle::new(*point, 1, color.filled()))?;
                        }
                    }
                }
            } else {
                let segments = match item.line_style {
                    LineStyle::Solid => vec![(sample_start, sample_end)],
                    LineStyle::Dashed => (0..2)
                        .map(|part| {
                            let start = sample_start + part * config.sample_length as i32 / 2;
                            let end = start + config.sample_length as i32 / 3;
                            (start, end.min(sample_end))
                        })
                        .collect(),
                    LineStyle::Dotted => (0..=4)
                        .map(|part| {
                            let dot_x = sample_start + part * config.sample_length as i32 / 4;
                            (dot_x, dot_x)
                        })
                        .collect(),
                };
                for (start, end) in segments {
                    area.draw(&PathElement::new(
                        vec![(start, center_y), (end, center_y)],
                        color.stroke_width(2),
                    ))?;
                }
            }
        }
        if let Some(marker) = item.marker {
            let center = (sample_center, center_y);
            let size = marker.size.max(2.0).round() as i32;
            let style = color.filled();
            match marker.shape {
                MarkerShape::Circle => area.draw(&Circle::new(center, size, style))?,
                MarkerShape::Square => area.draw(&Rectangle::new(
                    [
                        (center.0 - size, center.1 - size),
                        (center.0 + size, center.1 + size),
                    ],
                    style,
                ))?,
                MarkerShape::Triangle => area.draw(&TriangleMarker::new(center, size, style))?,
                MarkerShape::Cross => area.draw(&Cross::new(center, size, style))?,
            }
        }
        area.draw(&Text::new(
            item.name.to_owned(),
            (sample_end + padding, center_y - font_size as i32 / 2),
            text_style.clone(),
        ))?;
    }
    Ok(())
}

pub(crate) fn draw_grid<DB: DrawingBackend>(
    chart: &mut ChartContext<'_, DB, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    x_range: std::ops::Range<f64>,
    y_range: std::ops::Range<f64>,
    x_ticks: &TickConfig,
    y_ticks: &TickConfig,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let (plot_width, plot_height) = chart.plotting_area().dim_in_pixel();
    let max_x_lines = (plot_width / 4).clamp(2, 256) as usize;
    let max_y_lines = (plot_height / 4).clamp(2, 256) as usize;
    let x_major_lines = major_positions(&x_range, x_ticks, max_x_lines);
    let y_major_lines = major_positions(&y_range, y_ticks, max_y_lines);
    let x_minor_lines = minor_positions(&x_range, x_ticks, &x_major_lines, max_x_lines);
    let y_minor_lines = minor_positions(&y_range, y_ticks, &y_major_lines, max_y_lines);

    if x_ticks.major_grid_visible {
        let style = grid_style(x_ticks.major_grid_color, x_ticks.major_grid_width);
        chart.draw_series(
            x_major_lines
                .into_iter()
                .map(|x| PathElement::new(vec![(x, y_range.start), (x, y_range.end)], style)),
        )?;
    }
    if y_ticks.major_grid_visible {
        let style = grid_style(y_ticks.major_grid_color, y_ticks.major_grid_width);
        chart.draw_series(
            y_major_lines
                .into_iter()
                .map(|y| PathElement::new(vec![(x_range.start, y), (x_range.end, y)], style)),
        )?;
    }
    if x_ticks.minor_grid_visible {
        let style = grid_style(x_ticks.minor_grid_color, x_ticks.minor_grid_width);
        chart.draw_series(
            x_minor_lines
                .into_iter()
                .map(|x| PathElement::new(vec![(x, y_range.start), (x, y_range.end)], style)),
        )?;
    }
    if y_ticks.minor_grid_visible {
        let style = grid_style(y_ticks.minor_grid_color, y_ticks.minor_grid_width);
        chart.draw_series(
            y_minor_lines
                .into_iter()
                .map(|y| PathElement::new(vec![(x_range.start, y), (x_range.end, y)], style)),
        )?;
    }
    Ok(())
}

fn grid_style(color: Color, width: f32) -> ShapeStyle {
    RGBAColor(color.r, color.g, color.b, color.a as f64 / 255.0)
        .stroke_width(width.max(0.25).round() as u32)
}

fn major_positions(
    range: &std::ops::Range<f64>,
    ticks: &TickConfig,
    max_positions: usize,
) -> Vec<f64> {
    let Some(step) = effective_major_step(range, ticks, max_positions.min(256)) else {
        return Vec::new();
    };
    let first = (range.start / step).ceil() * step;
    if !first.is_finite() {
        return Vec::new();
    }
    let count =
        (((range.end - first) / step).floor().max(0.0) as usize).min(max_positions.min(256));
    (0..=count)
        .map(|index| first + index as f64 * step)
        .filter(|value| *value > range.start && *value < range.end)
        .collect()
}

fn minor_positions(
    range: &std::ops::Range<f64>,
    ticks: &TickConfig,
    majors: &[f64],
    max_positions: usize,
) -> Vec<f64> {
    let interval_count = majors.len().saturating_add(1).max(1);
    let max_divisions = (max_positions / interval_count).max(1);
    let divisions = (ticks.minor_per_major.min(32) as usize + 1).min(max_divisions + 1);
    if divisions < 2 || majors.is_empty() {
        return Vec::new();
    }
    let step = majors
        .get(1)
        .map(|next| next - majors[0])
        .unwrap_or_else(|| ticks.major_step.unwrap_or(range.end - range.start));
    let boundaries = std::iter::once(range.start)
        .chain(majors.iter().copied())
        .chain(std::iter::once(range.end))
        .collect::<Vec<_>>();
    let mut positions =
        Vec::with_capacity(max_positions.min(majors.len().saturating_add(1) * divisions));
    for interval in boundaries.windows(2) {
        let start = interval[0];
        let span = interval[1] - interval[0];
        let local_step = if span > 0.0 { span } else { step };
        for minor in 1..divisions {
            let value = start + local_step * minor as f64 / divisions as f64;
            if value.is_finite() && positions.len() < max_positions {
                positions.push(value);
            }
        }
    }
    positions
}

/// Draws one series' line, optional markers, and legend sample consistently
/// for interactive rendering and file export.
pub(crate) fn draw_series<DB: DrawingBackend>(
    chart: &mut ChartContext<'_, DB, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    series: &SeriesRender<'_>,
    x_range: &std::ops::Range<f64>,
    y_range: &std::ops::Range<f64>,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let SeriesRender {
        points,
        line_segments,
        error_bars,
        area_baseline,
        color,
        line_visible,
        line_width,
        line_style,
        line_interpolation,
        marker,
    } = series;
    let color = RGBAColor(color.r, color.g, color.b, color.a as f64 / 255.0);
    let line = ShapeStyle::from(&color).stroke_width(line_width.max(1.0) as u32);
    let (plot_width, plot_height) = chart.plotting_area().dim_in_pixel();
    let stroke_clearance = if *line_style == LineStyle::Dotted {
        1.0
    } else {
        f64::from(line_width.max(1.0)) / 2.0
    };
    let clipped_x_range = inset_range(x_range, stroke_clearance, plot_width);
    let clipped_y_range = inset_range(y_range, stroke_clearance, plot_height);
    if let Some(baseline) = *area_baseline {
        for segment in line_segments.iter() {
            let segment = match line_interpolation {
                LineInterpolation::Linear => segment.clone(),
                LineInterpolation::StepAfter => step_after_points(segment),
            };
            let fill_style = color.mix(0.2).filled();
            for triangle in area_fill_triangles(&segment, baseline) {
                let clipped = clip_polygon_to_ranges(&triangle, x_range, y_range);
                if clipped.len() >= 3 {
                    chart.draw_series(std::iter::once(Polygon::new(clipped, fill_style)))?;
                }
            }
        }
    }
    if !error_bars.is_empty() {
        let x_cap = (x_range.end - x_range.start).abs() * 4.0 / plot_width.max(1) as f64;
        let y_cap = (y_range.end - y_range.start).abs() * 4.0 / plot_height.max(1) as f64;
        let mut paths = Vec::with_capacity(error_bars.len() * 6);
        for error in error_bars.iter() {
            if let Some((low, high)) = error.x_bounds {
                push_clipped_path(
                    &mut paths,
                    (low, error.center.1),
                    (high, error.center.1),
                    &clipped_x_range,
                    &clipped_y_range,
                    line,
                );
                for x in [low, high] {
                    push_clipped_path(
                        &mut paths,
                        (x, error.center.1 - y_cap),
                        (x, error.center.1 + y_cap),
                        &clipped_x_range,
                        &clipped_y_range,
                        line,
                    );
                }
            }
            if let Some((low, high)) = error.y_bounds {
                push_clipped_path(
                    &mut paths,
                    (error.center.0, low),
                    (error.center.0, high),
                    &clipped_x_range,
                    &clipped_y_range,
                    line,
                );
                for y in [low, high] {
                    push_clipped_path(
                        &mut paths,
                        (error.center.0 - x_cap, y),
                        (error.center.0 + x_cap, y),
                        &clipped_x_range,
                        &clipped_y_range,
                        line,
                    );
                }
            }
        }
        chart.draw_series(paths)?;
    }
    if *line_visible {
        for segment in line_segments.iter() {
            let interpolated = match line_interpolation {
                LineInterpolation::Linear => segment.clone(),
                LineInterpolation::StepAfter => step_after_points(segment),
            };
            for line_points in
                clip_polyline_to_ranges(&interpolated, &clipped_x_range, &clipped_y_range)
            {
                match line_style {
                    LineStyle::Solid => {
                        chart.draw_series(LineSeries::new(line_points.iter().copied(), line))?;
                    }
                    LineStyle::Dashed => {
                        chart.draw_series(DashedLineSeries::new(
                            line_points.iter().copied(),
                            8,
                            5,
                            line,
                        ))?;
                    }
                    LineStyle::Dotted => {
                        chart.draw_series(DottedLineSeries::new(
                            line_points.iter().copied(),
                            0,
                            6,
                            move |point| Circle::new(point, 1, line),
                        ))?;
                    }
                }
            }
        }
    }

    if let Some(marker) = marker {
        let radius = marker.size.max(1.0).round() as i32;
        draw_clipped_markers(
            chart,
            points,
            marker.shape,
            radius,
            line.filled(),
            x_range,
            y_range,
        )?;
    }

    Ok(())
}

fn push_clipped_path(
    paths: &mut Vec<PathElement<(f64, f64)>>,
    start: (f64, f64),
    end: (f64, f64),
    x_range: &std::ops::Range<f64>,
    y_range: &std::ops::Range<f64>,
    style: ShapeStyle,
) {
    if let Some((start, end)) = clip_segment_to_ranges(start, end, x_range, y_range) {
        paths.push(PathElement::new(vec![start, end], style));
    }
}

fn clip_segment_to_ranges(
    start: (f64, f64),
    end: (f64, f64),
    x_range: &std::ops::Range<f64>,
    y_range: &std::ops::Range<f64>,
) -> Option<((f64, f64), (f64, f64))> {
    if ![start.0, start.1, end.0, end.1]
        .into_iter()
        .all(f64::is_finite)
    {
        return None;
    }
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let (mut entering, mut leaving) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, start.0 - x_range.start),
        (dx, x_range.end - start.0),
        (-dy, start.1 - y_range.start),
        (dy, y_range.end - start.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let intersection = q / p;
        if p < 0.0 {
            entering = entering.max(intersection);
        } else {
            leaving = leaving.min(intersection);
        }
        if entering > leaving {
            return None;
        }
    }
    let at = |t: f64| (start.0 + dx * t, start.1 + dy * t);
    Some((at(entering), at(leaving)))
}

fn clip_polyline_to_ranges(
    points: &[(f64, f64)],
    x_range: &std::ops::Range<f64>,
    y_range: &std::ops::Range<f64>,
) -> Vec<Vec<(f64, f64)>> {
    let mut clipped = Vec::<Vec<(f64, f64)>>::new();
    for pair in points.windows(2) {
        let Some((start, end)) = clip_segment_to_ranges(pair[0], pair[1], x_range, y_range) else {
            continue;
        };
        if let Some(segment) = clipped.last_mut()
            && segment.last().is_some_and(|last| {
                (last.0 - start.0).abs() < 1e-12 && (last.1 - start.1).abs() < 1e-12
            })
        {
            if segment.last().copied() != Some(end) {
                segment.push(end);
            }
        } else {
            clipped.push(vec![start, end]);
        }
    }
    clipped
}

fn clip_polygon_to_ranges(
    points: &[(f64, f64)],
    x_range: &std::ops::Range<f64>,
    y_range: &std::ops::Range<f64>,
) -> Vec<(f64, f64)> {
    let mut polygon = points
        .iter()
        .copied()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .collect::<Vec<_>>();
    let boundaries = [
        (0, x_range.start, true),
        (0, x_range.end, false),
        (1, y_range.start, true),
        (1, y_range.end, false),
    ];
    for (axis, edge, keep_greater) in boundaries {
        if polygon.is_empty() {
            break;
        }
        let input = std::mem::take(&mut polygon);
        let mut previous = *input.last().unwrap();
        let mut previous_inside = if keep_greater {
            previous_axis(previous, axis) >= edge
        } else {
            previous_axis(previous, axis) <= edge
        };
        for current in input {
            let current_inside = if keep_greater {
                previous_axis(current, axis) >= edge
            } else {
                previous_axis(current, axis) <= edge
            };
            if current_inside != previous_inside {
                let denom = previous_axis(current, axis) - previous_axis(previous, axis);
                if denom != 0.0 {
                    let t = (edge - previous_axis(previous, axis)) / denom;
                    polygon.push((
                        previous.0 + (current.0 - previous.0) * t,
                        previous.1 + (current.1 - previous.1) * t,
                    ));
                }
            }
            if current_inside {
                polygon.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
    }
    polygon
}

fn previous_axis(point: (f64, f64), axis: usize) -> f64 {
    if axis == 0 { point.0 } else { point.1 }
}

fn draw_clipped_markers<DB: DrawingBackend>(
    chart: &ChartContext<'_, DB, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    points: &[(f64, f64)],
    shape: MarkerShape,
    radius: i32,
    style: ShapeStyle,
    x_range: &std::ops::Range<f64>,
    y_range: &std::ops::Range<f64>,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let area = chart.plotting_area();
    let (x_pixels, y_pixels) = area.get_pixel_range();
    let (width, height) = (x_pixels.end - x_pixels.start, y_pixels.end - y_pixels.start);
    if width <= 0 || height <= 0 {
        return Ok(());
    }
    let x_radius = (x_range.end - x_range.start).abs() * f64::from(radius + 1) / f64::from(width);
    let y_radius = (y_range.end - y_range.start).abs() * f64::from(radius + 1) / f64::from(height);
    let local_x = 0.0..f64::from(width - 1);
    let local_y = 0.0..f64::from(height - 1);
    let pixel_area = area.strip_coord_spec();
    for point in points {
        if !point.0.is_finite()
            || !point.1.is_finite()
            || point.0 < x_range.start - x_radius
            || point.0 > x_range.end + x_radius
            || point.1 < y_range.start - y_radius
            || point.1 > y_range.end + y_radius
        {
            continue;
        }
        let mapped = area.map_coordinate(point);
        let center = (
            f64::from(mapped.0 - x_pixels.start),
            f64::from(mapped.1 - y_pixels.start),
        );
        match shape {
            MarkerShape::Cross => {
                for (start, end) in [
                    (
                        (center.0 - f64::from(radius), center.1 - f64::from(radius)),
                        (center.0 + f64::from(radius), center.1 + f64::from(radius)),
                    ),
                    (
                        (center.0 - f64::from(radius), center.1 + f64::from(radius)),
                        (center.0 + f64::from(radius), center.1 - f64::from(radius)),
                    ),
                ] {
                    if let Some((start, end)) =
                        clip_segment_to_ranges(start, end, &local_x, &local_y)
                    {
                        pixel_area.draw(&PathElement::new(
                            vec![
                                (start.0.round() as i32, start.1.round() as i32),
                                (end.0.round() as i32, end.1.round() as i32),
                            ],
                            style,
                        ))?;
                    }
                }
            }
            MarkerShape::Circle | MarkerShape::Square | MarkerShape::Triangle => {
                let polygon = match shape {
                    MarkerShape::Circle => (0..32)
                        .map(|index| {
                            let angle = std::f64::consts::TAU * index as f64 / 32.0;
                            (
                                center.0 + f64::from(radius) * angle.cos(),
                                center.1 + f64::from(radius) * angle.sin(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    MarkerShape::Square => vec![
                        (center.0 - f64::from(radius), center.1 - f64::from(radius)),
                        (center.0 + f64::from(radius), center.1 - f64::from(radius)),
                        (center.0 + f64::from(radius), center.1 + f64::from(radius)),
                        (center.0 - f64::from(radius), center.1 + f64::from(radius)),
                    ],
                    MarkerShape::Triangle => [-90.0, -210.0, -330.0]
                        .into_iter()
                        .map(|degrees| {
                            let angle = degrees * std::f64::consts::PI / 180.0;
                            (
                                (center.0 + f64::from(radius) * angle.cos()).ceil(),
                                (center.1 + f64::from(radius) * angle.sin()).ceil(),
                            )
                        })
                        .collect(),
                    MarkerShape::Cross => unreachable!(),
                };
                let clipped = clip_polygon_to_ranges(&polygon, &local_x, &local_y);
                if clipped.len() >= 3 {
                    pixel_area.draw(&Polygon::new(
                        clipped
                            .into_iter()
                            .map(|(x, y)| (x.round() as i32, y.round() as i32))
                            .collect::<Vec<_>>(),
                        style,
                    ))?;
                }
            }
        }
    }
    Ok(())
}

fn inset_range(range: &std::ops::Range<f64>, pixels: f64, extent: u32) -> std::ops::Range<f64> {
    let clearance = (range.end - range.start).abs() * pixels / extent.max(1) as f64;
    let inset = (range.start + clearance)..(range.end - clearance);
    if inset.start < inset.end {
        inset
    } else {
        let middle = (range.start + range.end) / 2.0;
        middle..middle
    }
}

pub(crate) fn step_after_points(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let Some(first) = points.first().copied() else {
        return Vec::new();
    };
    let mut stepped = Vec::with_capacity(points.len().saturating_mul(2));
    stepped.push(first);
    for pair in points.windows(2) {
        stepped.push((pair[1].0, pair[0].1));
        stepped.push(pair[1]);
    }
    stepped
}

fn area_fill_triangles(points: &[(f64, f64)], baseline: f64) -> Vec<[(f64, f64); 3]> {
    let mut triangles = Vec::with_capacity(points.len().saturating_sub(1) * 2);
    for pair in points.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        if ![x0, y0, x1, y1, baseline].into_iter().all(f64::is_finite) {
            continue;
        }

        let base0 = (x0, baseline);
        let base1 = (x1, baseline);
        let point0 = (x0, y0);
        let point1 = (x1, y1);
        let crosses_baseline = (y0 < baseline && y1 > baseline) || (y0 > baseline && y1 < baseline);
        if crosses_baseline {
            let fraction = (baseline - y0) / (y1 - y0);
            let crossing = (x0 + (x1 - x0) * fraction, baseline);
            triangles.push([base0, point0, crossing]);
            triangles.push([base1, crossing, point1]);
        } else {
            triangles.push([base0, point0, point1]);
            triangles.push([base0, point1, base1]);
        }
    }
    triangles
}

#[cfg(test)]
mod area_fill_tests {
    use super::{
        area_fill_triangles, clip_polygon_to_ranges, clip_polyline_to_ranges,
        clip_segment_to_ranges,
    };

    fn triangle_area(triangle: [(f64, f64); 3]) -> f64 {
        let [a, b, c] = triangle;
        ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() * 0.5
    }

    #[test]
    fn area_fill_splits_non_convex_contour_into_local_triangles() {
        let points = [(0.0, 0.0), (1.0, 3.0), (2.0, -2.0), (3.0, 2.0)];
        let triangles = area_fill_triangles(&points, 0.0);
        let area = triangles.iter().copied().map(triangle_area).sum::<f64>();

        assert_eq!(triangles.len(), 6);
        assert!((area - 3.8).abs() < 1e-12);
        assert!(
            triangles
                .iter()
                .all(|triangle| triangle.iter().all(|(_, y)| y == &0.0 || y.abs() <= 3.0))
        );
    }

    #[test]
    fn step_area_fill_uses_stepped_contour() {
        let points = [(0.0, 2.0), (1.0, 4.0), (2.0, 1.0)];
        let stepped = super::step_after_points(&points);
        let triangles = area_fill_triangles(&stepped, 0.0);
        let area = triangles.iter().copied().map(triangle_area).sum::<f64>();

        assert_eq!(
            stepped,
            [(0.0, 2.0), (1.0, 2.0), (1.0, 4.0), (2.0, 4.0), (2.0, 1.0)]
        );
        assert!((area - 6.0).abs() < 1e-12);
    }

    #[test]
    fn line_segments_crossing_manual_ranges_are_clipped_at_both_edges() {
        let clipped =
            clip_polyline_to_ranges(&[(-5.0, -5.0), (15.0, 15.0)], &(0.0..10.0), &(0.0..10.0));
        assert_eq!(clipped, vec![vec![(0.0, 0.0), (10.0, 10.0)]]);
        assert!(
            clip_segment_to_ranges((-5.0, 12.0), (15.0, 12.0), &(0.0..10.0), &(0.0..10.0),)
                .is_none()
        );
    }

    #[test]
    fn filled_area_polygons_stay_inside_manual_ranges() {
        let clipped = clip_polygon_to_ranges(
            &[(-5.0, 5.0), (5.0, 15.0), (15.0, 5.0)],
            &(0.0..10.0),
            &(0.0..10.0),
        );
        assert!(clipped.len() >= 3);
        assert!(clipped.iter().all(|(x, y)| {
            *x >= -1e-12 && *x <= 10.0 + 1e-12 && *y >= -1e-12 && *y <= 10.0 + 1e-12
        }));
        assert!(triangle_area([clipped[0], clipped[1], clipped[2]]) > 0.0);
    }

    #[test]
    fn error_bar_caps_are_clipped_to_axis_ranges() {
        assert_eq!(
            clip_segment_to_ranges((5.0, -2.0), (5.0, 2.0), &(0.0..10.0), &(0.0..10.0),),
            Some(((5.0, 0.0), (5.0, 2.0)))
        );
    }

    #[test]
    fn marker_shapes_at_plot_edges_are_clipped_instead_of_discarded() {
        let circle = (0..32)
            .map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / 32.0;
                (2.0 * angle.cos(), 5.0 + 2.0 * angle.sin())
            })
            .collect::<Vec<_>>();
        let clipped = clip_polygon_to_ranges(&circle, &(0.0..10.0), &(0.0..10.0));
        assert!(clipped.len() > 10);
        assert!(
            clipped.iter().all(|(x, y)| {
                *x >= -1e-12 && *x <= 10.0 + 1e-12 && *y >= -1e-12 && *y <= 10.0 + 1e-12
            }),
            "clipped marker polygon escaped the range: {clipped:?}"
        );
        assert!(triangle_area([clipped[0], clipped[1], clipped[2]]) > 0.0);
    }
}
