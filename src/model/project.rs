//! Portable native plot documents.

use serde::{Deserialize, Serialize};

use crate::model::{DataTable, PlotModel, PublicationDimensions, RangePolicy};

/// Current on-disk project schema version.
pub const PROJECT_FORMAT_VERSION: u32 = 3;

/// A complete, portable plot document with its data embedded.
#[derive(Clone, Serialize, Deserialize)]
pub struct PlotProject {
    pub format_version: u32,
    pub model: PlotModel,
    pub data: DataTable,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub publication_dimensions: PublicationDimensions,
    #[serde(default)]
    pub custom_font_asset_base64: Option<String>,
}

impl PlotProject {
    pub fn new(
        model: PlotModel,
        data: DataTable,
        custom_font_asset_base64: Option<String>,
        source_name: String,
        publication_dimensions: PublicationDimensions,
    ) -> Self {
        Self {
            format_version: PROJECT_FORMAT_VERSION,
            model,
            data,
            source_name,
            publication_dimensions,
            custom_font_asset_base64,
        }
    }

    /// Checks document invariants before a project replaces live editor state.
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=PROJECT_FORMAT_VERSION).contains(&self.format_version) {
            return Err(format!(
                "Unsupported project version {}; supported versions are 1 through {}",
                self.format_version, PROJECT_FORMAT_VERSION
            ));
        }
        if self.data.columns.len() < 2 || self.data.row_count == 0 {
            return Err("Project must contain at least two non-empty data columns".to_owned());
        }
        self.publication_dimensions
            .pixel_size()
            .map_err(|error| format!("Project has invalid publication dimensions: {error}"))?;
        if matches!(
            &self.model.layout.font_family,
            crate::model::PlotFontFamily::Custom(_)
                | crate::model::PlotFontFamily::CustomAsset { .. }
        ) {
            let Some(encoded_asset) = &self.custom_font_asset_base64 else {
                return Err(
                    "Project references a custom font but does not embed its font asset".to_owned(),
                );
            };
            let bytes =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded_asset)
                    .map_err(|_| "Project contains an invalid embedded font asset".to_owned())?;
            if let crate::model::PlotFontFamily::CustomAsset { asset_id, .. } =
                &self.model.layout.font_family
            {
                if self.format_version < 2 || crate::font::font_asset_id(&bytes) != *asset_id {
                    return Err(
                        "Project font asset fingerprint does not match its embedded file"
                            .to_owned(),
                    );
                }
            } else if self.format_version >= 2 {
                return Err(
                    "Project version 2 requires a fingerprinted custom font reference".to_owned(),
                );
            }
        } else if self.format_version >= 2 && self.custom_font_asset_base64.is_some() {
            return Err("Project embeds a custom font that is not selected by the plot".to_owned());
        }
        let mut names = std::collections::HashSet::new();
        for column in &self.data.columns {
            if column.name.trim().is_empty() || !names.insert(column.name.as_str()) {
                return Err("Project data contains an empty or duplicate column name".to_owned());
            }
            if column.values.len() != self.data.row_count {
                return Err(format!(
                    "Column '{}' has {} rows, expected {}",
                    column.name,
                    column.values.len(),
                    self.data.row_count
                ));
            }
            if let Some((row, _)) = column
                .values
                .iter()
                .enumerate()
                .find(|(_, value)| !value.is_finite())
            {
                return Err(format!(
                    "Column '{}' has a non-finite value at row {}",
                    column.name,
                    row + 1
                ));
            }
        }

        for (axis_name, axis) in [("X", &self.model.axes.x), ("Y", &self.model.axes.y)] {
            if !(1..=256).contains(&axis.axis_title_font_size)
                || !(1..=256).contains(&axis.label_font_size)
            {
                return Err(format!("Project has invalid {axis_name} font sizes"));
            }
            if let RangePolicy::Manual { min, max } = &axis.range {
                if !min.is_finite() || !max.is_finite() || min >= max {
                    return Err(format!("Project has an invalid manual {axis_name} range"));
                }
                if axis.scale != crate::model::ScaleType::Linear && (*min <= 0.0 || *max <= 0.0) {
                    return Err(format!(
                        "Project has a non-positive manual {axis_name} log range"
                    ));
                }
            }
            if axis
                .ticks
                .major_step
                .is_some_and(|step| !step.is_finite() || step <= 0.0)
                || !axis.ticks.major_tick_length.is_finite()
                || !(0.0..=20.0).contains(&axis.ticks.major_tick_length)
                || !axis.ticks.minor_tick_length.is_finite()
                || !(0.0..=20.0).contains(&axis.ticks.minor_tick_length)
                || !axis.ticks.major_grid_width.is_finite()
                || !(0.0..=20.0).contains(&axis.ticks.major_grid_width)
                || !axis.ticks.minor_grid_width.is_finite()
                || !(0.0..=20.0).contains(&axis.ticks.minor_grid_width)
                || axis.ticks.minor_per_major > 1000
            {
                return Err(format!("Project has invalid {axis_name} tick settings"));
            }
            if axis.title_anchor.is_some_and(|anchor| {
                anchor
                    .iter()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            }) {
                return Err(format!("Project has an invalid {axis_name} title position"));
            }
        }

        if self.model.series.is_empty() {
            return Err("Project must contain at least one series".to_owned());
        }
        if !(1..=512).contains(&self.model.layout.title_font_size)
            || !(1..=256).contains(&self.model.legend.font_size)
            || self.model.layout.margin > 1000
            || self.model.layout.x_label_area_size > 2000
            || self.model.layout.y_label_area_size > 2000
        {
            return Err("Project has invalid text or plot layout dimensions".to_owned());
        }
        let mut series_ids = std::collections::HashSet::new();
        for series in &self.model.series {
            if !series_ids.insert(series.id) {
                return Err("Project contains duplicate series identifiers".to_owned());
            }
            for column in [
                Some(series.x_column.as_str()),
                Some(series.y_column.as_str()),
                series.x_error_column.as_deref(),
                series.y_error_column.as_deref(),
                series.x_error_lower_column.as_deref(),
                series.x_error_upper_column.as_deref(),
                series.y_error_lower_column.as_deref(),
                series.y_error_upper_column.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                if !names.contains(column) {
                    return Err(format!(
                        "Series '{}' references missing column '{column}'",
                        series.name
                    ));
                }
            }
            let x_asymmetric =
                series.x_error_lower_column.is_some() || series.x_error_upper_column.is_some();
            let y_asymmetric =
                series.y_error_lower_column.is_some() || series.y_error_upper_column.is_some();
            let active_error_columns = [
                series.x_error_lower_column.as_deref(),
                series.x_error_upper_column.as_deref(),
                (!x_asymmetric)
                    .then_some(series.x_error_column.as_deref())
                    .flatten(),
                series.y_error_lower_column.as_deref(),
                series.y_error_upper_column.as_deref(),
                (!y_asymmetric)
                    .then_some(series.y_error_column.as_deref())
                    .flatten(),
            ];
            for column in active_error_columns.into_iter().flatten() {
                if let Some(values) = self.data.column_values(column)
                    && let Some((row, value)) =
                        values.iter().enumerate().find(|(_, value)| **value < 0.0)
                {
                    return Err(format!(
                        "Series '{}' uncertainty column '{column}' has a negative magnitude ({value}) at row {}",
                        series.name,
                        row + 1
                    ));
                }
            }
            if !series.style.line_width.is_finite()
                || !(0.0..=100.0).contains(&series.style.line_width)
                || (series.style.line_visible && series.style.line_width == 0.0)
            {
                return Err(format!(
                    "Series '{}' has an invalid line width",
                    series.name
                ));
            }
            if series
                .style
                .area_baseline
                .is_some_and(|value| !value.is_finite())
            {
                return Err(format!(
                    "Series '{}' has an invalid area baseline",
                    series.name
                ));
            }
            if series.style.area_fill_visible
                && (series.style.area_baseline.is_none()
                    || (self.model.axes.y.scale != crate::model::ScaleType::Linear
                        && series.style.area_baseline.is_some_and(|value| value <= 0.0)))
            {
                return Err(format!(
                    "Series '{}' has an invalid area fill baseline for its Y axis",
                    series.name
                ));
            }
            if series.style.marker.as_ref().is_some_and(|marker| {
                !marker.size.is_finite() || !(0.0..=512.0).contains(&marker.size)
            }) {
                return Err(format!(
                    "Series '{}' has an invalid marker size",
                    series.name
                ));
            }
            if !series.style.marker_preset.size.is_finite()
                || !(0.0..=512.0).contains(&series.style.marker_preset.size)
            {
                return Err(format!(
                    "Series '{}' has an invalid saved marker size",
                    series.name
                ));
            }
        }

        let mut annotation_ids = std::collections::HashSet::new();
        for annotation in &self.model.annotations {
            if annotation.id == 0
                || !annotation_ids.insert(annotation.id)
                || annotation.text.trim().is_empty()
                || !annotation.x.is_finite()
                || !annotation.y.is_finite()
                || !(4..=256).contains(&annotation.font_size)
                || (self.model.axes.x.scale != crate::model::ScaleType::Linear
                    && annotation.x <= 0.0)
                || (self.model.axes.y.scale != crate::model::ScaleType::Linear
                    && annotation.y <= 0.0)
            {
                return Err("Project contains an invalid plot annotation".to_owned());
            }
        }

        let mut panel_ids = std::collections::HashSet::new();
        for panel in &self.model.panels {
            let mut assigned = std::collections::HashSet::new();
            if panel.id == 0
                || !panel_ids.insert(panel.id)
                || panel.title.trim().is_empty()
                || panel.series_ids.iter().any(|id| {
                    !assigned.insert(*id)
                        || !self.model.series.iter().any(|series| series.id == *id)
                })
            {
                return Err("Project contains an invalid plot panel".to_owned());
            }
        }
        if self.model.panels.len() > 12 {
            return Err("Project has more than 12 plot panels".to_owned());
        }
        if self.model.legend.columns == 0
            || !self.model.legend.border_width.is_finite()
            || self.model.legend.border_width < 0.0
            || self
                .model
                .legend
                .anchor
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err("Project has invalid legend settings".to_owned());
        }
        Ok(())
    }
}
