//! Timbre-Kern: hier entstehen Renderer, ECS, Asset-System und Scripting-Host.

pub mod renderer;

pub use renderer::Renderer;

pub const NAME: &str = "Timbre";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Liefert die Startzeile der Engine.
pub fn banner() -> String {
    format!("{NAME} {VERSION} – bereit.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_enthaelt_namen() {
        assert!(banner().starts_with("Timbre"));
    }
}
