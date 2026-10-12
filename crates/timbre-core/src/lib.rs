//! Timbre-Kern: Renderer, ECS (Welt), Assets, Szenen – später der Scripting-Host.

pub mod assets;
pub mod input;
pub mod kamera;
pub mod renderer;
#[cfg(not(target_arch = "wasm32"))]
pub mod scripting;
pub mod szene;
pub mod welt;
pub mod zeit;

pub use assets::Assets;
pub use glam::{vec2, Vec2};
pub use input::{Input, KeyCode, MouseButton};
pub use kamera::Kamera2d;
pub use renderer::{Bereich, Frame, FrameStatistik, Renderer, Sprite, TexturId};
/// Das Asset-Format, damit Spiele nicht extra `timbre-tbr` einbinden müssen.
pub use timbre_tbr as tbr;
pub use welt::{Bewegung, Bild, Eltern, Name, Objekt, Script, Transform, Welt};
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
