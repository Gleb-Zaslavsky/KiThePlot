//! Data ingestion and columnar storage layer.
//!
//! This module is intentionally backend-agnostic:
//! - `DataSource` is a trait for host applications to provide numeric columns.
//! - `DataTable` is an internal normalized representation used by the editor.
//! - CSV/TXT parsing utilities build `DataTable` from files.

use std::fs;
use std::path::Path;

/// One numeric column with a stable display name.
#[derive(Clone, Debug)]
pub struct ColumnData {
    pub name: String,
    pub values: Vec<f64>,
}

/// In-memory normalized table used by plotting logic.
#[derive(Clone, Debug)]
pub struct DataTable {
    pub columns: Vec<ColumnData>,
    pub row_count: usize,
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
        parse_delimited(&text, Delimiter::Comma)
    }

    /// Parses a whitespace-separated TXT file into a numeric table.
    pub fn from_txt_path(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read TXT: {e} / Ne udalos prochitat TXT"))?;
        parse_delimited(&text, Delimiter::Whitespace)
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
    pub fn points_for_columns(&self, x: &str, y: &str) -> Result<Vec<(f32, f32)>, String> {
        let x_values = self
            .column_values(x)
            .ok_or_else(|| format!("X column not found: {x}"))?;
        let y_values = self
            .column_values(y)
            .ok_or_else(|| format!("Y column not found: {y}"))?;

        let len = x_values.len().min(y_values.len());
        Ok((0..len)
            .map(|i| (x_values[i] as f32, y_values[i] as f32))
            .collect())
    }

    /// Normalizes any `DataSource` implementation into a `DataTable`.
    pub fn from_data_source(source: &dyn DataSource) -> Result<Self, String> {
        let names = source.column_names();
        if names.is_empty() {
            return Err("Data source has no columns / Istochnik ne soderzhit stolbcov".to_owned());
        }

        let mut columns = Vec::with_capacity(names.len());
        let mut row_count = source.len();
        for name in names {
            let values = source
                .column(&name)
                .ok_or_else(|| format!("Missing column in source: {name}"))?;
            row_count = row_count.min(values.len());
            columns.push(ColumnData { name, values });
        }

        for col in &mut columns {
            col.values.truncate(row_count);
        }

        Ok(Self { columns, row_count })
    }
}

#[derive(Clone, Copy)]
enum Delimiter {
    Comma,
    Whitespace,
}

fn parse_delimited(text: &str, delimiter: Delimiter) -> Result<DataTable, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));

    let first = lines
        .next()
        .ok_or_else(|| "Input file is empty / Fail pust".to_owned())?;

    let first_tokens = split_tokens(first, delimiter);
    if first_tokens.is_empty() {
        return Err("First row has no values / V pervoy stroke net znacheniy".to_owned());
    }

    let first_is_header = first_tokens.iter().any(|t| t.parse::<f64>().is_err());
    let headers: Vec<String>;
    let mut rows: Vec<Vec<f64>> = Vec::new();

    if first_is_header {
        headers = first_tokens;
    } else {
        headers = (1..=first_tokens.len())
            .map(|i| format!("col_{i}"))
            .collect();
        rows.push(parse_numeric_row(&first_tokens)?);
    }

    for (line_no, line) in lines.enumerate() {
        let tokens = split_tokens(line, delimiter);
        if tokens.len() != headers.len() {
            return Err(format!(
                "Row {} has {} values, expected {}",
                line_no + 2,
                tokens.len(),
                headers.len()
            ));
        }
        rows.push(parse_numeric_row(&tokens)?);
    }

    if rows.is_empty() {
        return Err("No numeric rows found / Net chislovyh strok".to_owned());
    }

    let row_count = rows.len();
    let mut columns = Vec::with_capacity(headers.len());
    for (idx, name) in headers.into_iter().enumerate() {
        let mut values = Vec::with_capacity(row_count);
        for row in &rows {
            values.push(row[idx]);
        }
        columns.push(ColumnData { name, values });
    }

    Ok(DataTable { columns, row_count })
}

fn split_tokens(line: &str, delimiter: Delimiter) -> Vec<String> {
    match delimiter {
        Delimiter::Comma => line
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
        Delimiter::Whitespace => line
            .split_whitespace()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

fn parse_numeric_row(tokens: &[String]) -> Result<Vec<f64>, String> {
    let mut row = Vec::with_capacity(tokens.len());
    for token in tokens {
        let value = token
            .parse::<f64>()
            .map_err(|_| format!("Failed to parse number: {token}"))?;
        row.push(value);
    }
    Ok(row)
}

/// Generic data-provider contract for embedding this crate into host apps.
pub trait DataSource: Send + Sync {
    fn column(&self, name: &str) -> Option<Vec<f64>>;
    fn column_names(&self) -> Vec<String>;
    fn len(&self) -> usize;
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
