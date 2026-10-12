//! Timbre-Kern: hier entstehen Renderer, ECS, Asset-System und Scripting-Host.

pub mod input;
pub mod kamera;
pub mod renderer;
pub mod zeit;

pub use glam::{vec2, Vec2};
pub use input::{Input, KeyCode, MouseButton};
pub use kamera::Kamera2d;
pub use renderer::{FrameStatistik, Renderer, Sprite, TexturId};
pub use zeit::FesterTakt;

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
