//! Data ingestion and columnar storage layer.
//!
//! This module is intentionally backend-agnostic:
//! - `DataSource` is a trait for host applications to provide numeric columns.
//! - `DataTable` is an internal normalized representation used by the editor.
//! - CSV/TXT parsing utilities build `DataTable` from files.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// One numeric column with a stable display name.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColumnData {
    pub name: String,
    pub values: Vec<f64>,
}

/// In-memory normalized table used by plotting logic.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataTable {
    pub columns: Vec<ColumnData>,
    pub row_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportDelimiter {
    Comma,
    Semicolon,
    Tab,
    Whitespace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalConvention {
    Dot,
    Comma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderMode {
    Auto,
    Present,
    Absent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportOptions {
    pub delimiter: ImportDelimiter,
    pub decimal: DecimalConvention,
    pub header: HeaderMode,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            delimiter: ImportDelimiter::Comma,
            decimal: DecimalConvention::Dot,
            header: HeaderMode::Auto,
        }
    }
}

/// Full-file parse summary. A table can be committed only when every data row is valid.
#[derive(Clone, Debug)]
pub struct ImportPreview {
    pub columns: Vec<String>,
    pub sample_rows: Vec<Vec<f64>>,
    pub accepted_rows: usize,
    pub rejected_rows: usize,
    pub errors: Vec<String>,
    pub fatal_error: Option<String>,
    pub table: Option<DataTable>,
}

impl ImportPreview {
    pub fn can_import(&self) -> bool {
        self.fatal_error.is_none() && self.rejected_rows == 0 && self.table.is_some()
    }
}

impl DataTable {
    /// Creates an empty table.
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            row_count: 0,
        }
    }

    /// Parses a CSV file into a numeric table.
    pub fn from_csv_path(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read CSV: {e} / Ne udalos prochitat CSV"))?;
        let options = ImportOptions::default();
        parse_import_preview(&text, options).into_table()
    }

    /// Parses a whitespace-separated TXT file into a numeric table.
    pub fn from_txt_path(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read TXT: {e} / Ne udalos prochitat TXT"))?;
        let options = ImportOptions {
            delimiter: ImportDelimiter::Whitespace,
            ..ImportOptions::default()
        };
        parse_import_preview(&text, options).into_table()
    }

    pub fn from_path_with_options(path: &Path, options: ImportOptions) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("Failed to read file: {error}"))?;
        parse_import_preview(&text, options).into_table()
    }

    pub fn preview_path_with_options(
        path: &Path,
        options: ImportOptions,
    ) -> Result<ImportPreview, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("Failed to read file: {error}"))?;
        Ok(parse_import_preview(&text, options))
    }

    /// Returns column names in display order.
    pub fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }

    /// Checks whether a column with this name exists.
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }

    /// Returns immutable numeric values of a named column.
    pub fn column_values(&self, name: &str) -> Option<&[f64]> {
        self.columns
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.values.as_slice())
    }

    /// Builds `(x, y)` points from two selected columns.
    pub fn points_for_columns(&self, x: &str, y: &str) -> Result<Vec<(f64, f64)>, String> {
        let x_values = self
            .column_values(x)
            .ok_or_else(|| format!("X column not found: {x}"))?;
        let y_values = self
            .column_values(y)
            .ok_or_else(|| format!("Y column not found: {y}"))?;

        let len = x_values.len().min(y_values.len());
        Ok((0..len).map(|i| (x_values[i], y_values[i])).collect())
    }

    /// Normalizes any `DataSource` implementation into a `DataTable`.
    pub fn from_data_source(source: &dyn DataSource) -> Result<Self, String> {
        let names = source.column_names();
        if names.len() < 2 {
            return Err("Data source must contain at least two columns".to_owned());
        }

        let expected_len = source.len();
        let mut columns = Vec::with_capacity(names.len());
        let mut unique_names = std::collections::HashSet::with_capacity(names.len());
        for name in names {
            if name.trim().is_empty() || !unique_names.insert(name.clone()) {
                return Err(format!(
                    "Data source has an empty or duplicate column name: '{name}'"
                ));
            }
            let values = source
                .column(&name)
                .ok_or_else(|| format!("Missing column in source: {name}"))?;
            if values.len() != expected_len {
                return Err(format!(
                    "Column '{name}' has {} values, but the data source declares {expected_len} rows",
                    values.len()
                ));
            }
            if let Some((row, _)) = values
                .iter()
                .enumerate()
                .find(|(_, value)| !value.is_finite())
            {
                return Err(format!(
                    "Column '{name}' has a non-finite value at row {}",
                    row + 1
                ));
            }
            columns.push(ColumnData { name, values });
        }

        Ok(Self {
            columns,
            row_count: expected_len,
        })
    }
}

impl ImportPreview {
    fn into_table(self) -> Result<DataTable, String> {
        if let Some(error) = self.fatal_error {
            return Err(error);
        }
        if self.rejected_rows > 0 {
            return Err(self.errors.first().cloned().unwrap_or_else(|| {
                format!("{} data rows contain invalid values", self.rejected_rows)
            }));
        }
        self.table.ok_or_else(|| "No numeric rows found".to_owned())
    }
}

pub fn parse_import_preview(text: &str, options: ImportOptions) -> ImportPreview {
    let records = match read_records(text, options.delimiter) {
        Ok(records) => records,
        Err(error) => return fatal_preview(error),
    };
    if records.is_empty() {
        return fatal_preview("Input file is empty".to_owned());
    }
    let header_present = match options.header {
        HeaderMode::Present => true,
        HeaderMode::Absent => false,
        HeaderMode::Auto => !record_looks_like_data(&records[0].1, options.decimal),
    };
    let first_data_index = usize::from(header_present);
    let headers: Vec<String> = if header_present {
        records[0]
            .1
            .iter()
            .map(|token| token.trim().to_owned())
            .collect()
    } else {
        (1..=records[0].1.len())
            .map(|index| format!("col_{index}"))
            .collect()
    };
    if headers.len() < 2 {
        return fatal_preview("Import requires at least two columns".to_owned());
    }
    if let Err(error) = validate_headers(&headers) {
        return fatal_preview(error);
    }

    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut rejected_rows = 0;
    for (line_no, tokens) in records.into_iter().skip(first_data_index) {
        if tokens.len() != headers.len() {
            rejected_rows += 1;
            push_import_error(
                &mut errors,
                format!(
                    "Row {line_no} has {} values, expected {}",
                    tokens.len(),
                    headers.len()
                ),
            );
            continue;
        }
        match parse_numeric_row(&tokens, line_no, &headers, options.decimal) {
            Ok(row) => rows.push(row),
            Err(error) => {
                rejected_rows += 1;
                push_import_error(&mut errors, error);
            }
        }
    }
    if rows.is_empty() && rejected_rows == 0 {
        return fatal_preview("No numeric rows found".to_owned());
    }
    let accepted_rows = rows.len();
    let sample_rows = rows.iter().take(12).cloned().collect();
    let row_count = rows.len();
    let columns = headers
        .into_iter()
        .enumerate()
        .map(|(index, name)| ColumnData {
            name,
            values: rows.iter().map(|row| row[index]).collect(),
        })
        .collect::<Vec<_>>();
    ImportPreview {
        columns: columns.iter().map(|column| column.name.clone()).collect(),
        sample_rows,
        accepted_rows,
        rejected_rows,
        errors,
        fatal_error: None,
        table: (rejected_rows == 0).then_some(DataTable { columns, row_count }),
    }
}

fn read_records(
    text: &str,
    delimiter: ImportDelimiter,
) -> Result<Vec<(usize, Vec<String>)>, String> {
    if delimiter == ImportDelimiter::Whitespace {
        return Ok(text
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let line = line.trim();
                (!line.is_empty() && !line.starts_with('#')).then(|| {
                    (
                        index + 1,
                        line.split_whitespace().map(str::to_owned).collect(),
                    )
                })
            })
            .collect());
    }
    let byte = match delimiter {
        ImportDelimiter::Comma => b',',
        ImportDelimiter::Semicolon => b';',
        ImportDelimiter::Tab => b'\t',
        ImportDelimiter::Whitespace => unreachable!(),
    };
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .comment(Some(b'#'))
        .delimiter(byte)
        .from_reader(text.as_bytes());
    let mut records = Vec::new();
    for result in reader.records() {
        match result {
            Ok(record) => {
                let line_no = record
                    .position()
                    .map(|position| position.line() as usize)
                    .unwrap_or(records.len() + 1);
                records.push((
                    line_no,
                    record.iter().map(str::trim).map(str::to_owned).collect(),
                ));
            }
            Err(error) => {
                let line = error
                    .position()
                    .map(|position| position.line())
                    .unwrap_or(0);
                return Err(format!("CSV parse error at line {line}: {error}"));
            }
        }
    }
    Ok(records)
}

fn record_looks_like_data(tokens: &[String], decimal: DecimalConvention) -> bool {
    tokens
        .iter()
        .any(|token| parse_number(token, decimal).is_ok() || is_missing_or_non_finite_marker(token))
}

fn is_missing_or_non_finite_marker(token: &str) -> bool {
    matches!(
        token.trim().to_ascii_lowercase().as_str(),
        "" | "na"
            | "n/a"
            | "nan"
            | "inf"
            | "+inf"
            | "-inf"
            | "infinity"
            | "+infinity"
            | "-infinity"
    )
}

fn parse_number(token: &str, decimal: DecimalConvention) -> Result<f64, String> {
    let normalized;
    let token = match decimal {
        DecimalConvention::Dot => token,
        DecimalConvention::Comma if !token.contains('.') => {
            normalized = token.replace(',', ".");
            &normalized
        }
        DecimalConvention::Comma => return Err("mixed decimal separators".to_owned()),
    };
    let value = token
        .parse::<f64>()
        .map_err(|_| format!("Invalid number '{token}'"))?;
    if !value.is_finite() {
        return Err("Non-finite value".to_owned());
    }
    Ok(value)
}

fn push_import_error(errors: &mut Vec<String>, error: String) {
    if errors.len() < 20 {
        errors.push(error);
    }
}

fn fatal_preview(error: String) -> ImportPreview {
    ImportPreview {
        columns: Vec::new(),
        sample_rows: Vec::new(),
        accepted_rows: 0,
        rejected_rows: 0,
        errors: Vec::new(),
        fatal_error: Some(error),
        table: None,
    }
}

fn validate_headers(headers: &[String]) -> Result<(), String> {
    let mut names = std::collections::HashSet::with_capacity(headers.len());
    for name in headers {
        if name.is_empty() || !names.insert(name.as_str()) {
            return Err(format!("CSV header is empty or duplicated: '{name}'"));
        }
    }
    Ok(())
}

fn parse_numeric_row(
    tokens: &[String],
    line_no: usize,
    headers: &[String],
    decimal: DecimalConvention,
) -> Result<Vec<f64>, String> {
    let mut row = Vec::with_capacity(tokens.len());
    for (index, token) in tokens.iter().enumerate() {
        let column = headers.get(index).map(String::as_str).unwrap_or("unknown");
        if token.is_empty() {
            return Err(format!("Missing value at row {line_no}, column '{column}'"));
        }
        let value = parse_number(token, decimal)
            .map_err(|error| format!("{error} at row {line_no}, column '{column}'"))?;
        row.push(value);
    }
    Ok(row)
}

/// Generic data-provider contract for embedding this crate into host apps.
pub trait DataSource: Send + Sync {
    fn column(&self, name: &str) -> Option<Vec<f64>>;
    fn column_names(&self) -> Vec<String>;
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl DataSource for DataTable {
    fn column(&self, name: &str) -> Option<Vec<f64>> {
        self.column_values(name).map(ToOwned::to_owned)
    }

    fn column_names(&self) -> Vec<String> {
        self.column_names()
    }

    fn len(&self) -> usize {
        self.row_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UnevenDataSource;

    impl DataSource for UnevenDataSource {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "x" => Some(vec![1.0, 2.0]),
                "y" => Some(vec![10.0]),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            vec!["x".to_owned(), "y".to_owned()]
        }

        fn len(&self) -> usize {
            2
        }
    }

    #[test]
    fn csv_rejects_missing_cells_instead_of_shifting_columns() {
        let preview = parse_import_preview("x,y,z\n1,,3\n", ImportOptions::default());
        assert_eq!(preview.rejected_rows, 1);
        assert!(preview.errors[0].contains("column 'y'"));
    }

    #[test]
    fn csv_rejects_duplicate_headers_and_non_finite_measurements() {
        let duplicate =
            parse_import_preview("time,signal,signal\n0,1,2\n", ImportOptions::default());
        assert!(duplicate.fatal_error.unwrap().contains("duplicated"));

        let non_finite = parse_import_preview("time,signal\n0,NaN\n", ImportOptions::default());
        assert!(non_finite.errors[0].contains("Non-finite"));

        let headerless_non_finite = parse_import_preview("0,NaN\n1,2\n", ImportOptions::default());
        assert_eq!(headerless_non_finite.columns, vec!["col_1", "col_2"]);
        assert_eq!(headerless_non_finite.rejected_rows, 1);
        assert!(headerless_non_finite.errors[0].contains("Non-finite"));

        let headerless_missing = parse_import_preview("1,,3\n2,4,6\n", ImportOptions::default());
        assert_eq!(headerless_missing.columns, vec!["col_1", "col_2", "col_3"]);
        assert_eq!(headerless_missing.rejected_rows, 1);
        assert!(headerless_missing.errors[0].contains("Missing value"));
    }

    #[test]
    fn import_preview_parses_quoted_semicolon_data_with_decimal_comma() {
        let options = ImportOptions {
            delimiter: ImportDelimiter::Semicolon,
            decimal: DecimalConvention::Comma,
            header: HeaderMode::Present,
        };
        let preview = parse_import_preview("\"time s\";signal\n0,5;1,25\n1,5;2,75\n", options);
        assert!(preview.can_import());
        assert_eq!(preview.columns, vec!["time s", "signal"]);
        assert_eq!(
            preview.table.unwrap().column_values("signal").unwrap(),
            &[1.25, 2.75]
        );
    }

    #[test]
    fn import_preview_reports_valid_and_rejected_rows_without_authorizing_partial_import() {
        let preview = parse_import_preview("x,y\n1,2\n3,bad\n4,5\n", ImportOptions::default());
        assert_eq!(preview.accepted_rows, 2);
        assert_eq!(preview.rejected_rows, 1);
        assert!(!preview.can_import());
        assert!(preview.errors[0].contains("row 3"));
    }

    #[test]
    fn data_source_rejects_uneven_columns_instead_of_truncating() {
        let result = DataTable::from_data_source(&UnevenDataSource);
        assert!(result.unwrap_err().contains("declares 2 rows"));
    }
}
