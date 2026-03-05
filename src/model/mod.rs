//! Domain model module.
//!
//! - `data`: generic columnar numeric data storage + parsing utilities.
//! - `types`: reusable configuration/style types for plotting.
//! - `plot`: aggregate plot model (axes, legend, series, layout).

pub mod data;
pub mod plot;
pub mod types;

pub use data::*;
pub use plot::*;
pub use types::*;
