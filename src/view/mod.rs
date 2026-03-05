//! View layer (MVC "V").
//!
//! Contains egui widgets and layout. The view never mutates model directly; it emits `Action`s.

pub mod app;
pub mod menu;
pub mod panels;

pub use app::*;
pub use menu::*;
pub use panels::*;


