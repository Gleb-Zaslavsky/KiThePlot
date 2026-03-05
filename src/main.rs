//! Standalone binary entrypoint.
//!
//! The binary just delegates to the library runner so the same crate can be
//! used both as a standalone app and as an embeddable library.

fn main() -> Result<(), eframe::Error> {
    plot_redactor::run_native()
}
