//! 2D-Kamera.
//!
//! Welt-Einheiten: bei Zoom 1 entspricht eine Einheit einem Pixel.
//! Y zeigt nach oben (wie in der Mathematik), die Kamera schaut auf `position`.

use glam::{Mat4, Vec2, Vec4};

#[derive(Clone, Copy, Debug)]
pub struct Kamera2d {
    pub position: Vec2,
    pub zoom: f32,
}

impl Default for Kamera2d {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Kamera2d {
    /// Matrix von Welt- in Bildschirmkoordinaten (für den Shader).
    pub fn matrix(&self, bildschirm: Vec2) -> Mat4 {
        // Orthografische Projektion: sichtbarer Weltausschnitt → Bereich -1..1 der Grafikkarte.
        let halb = bildschirm * 0.5 / self.zoom;
        let sx = 1.0 / halb.x;
        let sy = 1.0 / halb.y;
        Mat4::from_cols(
            Vec4::new(sx, 0.0, 0.0, 0.0),
            Vec4::new(0.0, sy, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 0.5, 0.0),
            Vec4::new(-self.position.x * sx, -self.position.y * sy, 0.5, 1.0),
        )
    }

    /// Pixelposition (z. B. Maus, (0,0) = links oben) → Weltposition.
    pub fn bildschirm_zu_welt(&self, pixel: Vec2, bildschirm: Vec2) -> Vec2 {
        let mitte = bildschirm * 0.5;
        let d = Vec2::new(pixel.x - mitte.x, mitte.y - pixel.y);
        self.position + d / self.zoom
    }

    /// Zwischen zwei Kamerazuständen überblenden (für flüssiges Zeichnen).
    pub fn lerp(&self, ziel: &Kamera2d, t: f32) -> Kamera2d {
        Kamera2d {
            position: self.position.lerp(ziel.position, t),
            zoom: self.zoom + (ziel.zoom - self.zoom) * t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mitte_ist_kameraposition() {
        let k = Kamera2d {
            position: Vec2::new(100.0, 50.0),
            zoom: 2.0,
        };
        let b = Vec2::new(800.0, 600.0);
        assert_eq!(
            k.bildschirm_zu_welt(Vec2::new(400.0, 300.0), b),
            Vec2::new(100.0, 50.0)
        );
        // 200 px rechts von der Mitte bei Zoom 2 = 100 Welteinheiten
        assert_eq!(
            k.bildschirm_zu_welt(Vec2::new(600.0, 300.0), b),
            Vec2::new(200.0, 50.0)
        );
        // oben am Bildschirm = größeres y
        assert_eq!(
            k.bildschirm_zu_welt(Vec2::new(400.0, 0.0), b),
            Vec2::new(100.0, 200.0)
        );
    }

    #[test]
    fn matrix_passt_zu_bildschirm_zu_welt() {
        let k = Kamera2d {
            position: Vec2::new(30.0, -20.0),
            zoom: 1.5,
        };
        let b = Vec2::new(800.0, 600.0);
        // rechte obere Ecke des Bildschirms muss in der Grafikkarte bei (1, 1) landen
        let w = k.bildschirm_zu_welt(Vec2::new(800.0, 0.0), b);
        let clip = k.matrix(b) * Vec4::new(w.x, w.y, 0.0, 1.0);
        assert!((clip.x - 1.0).abs() < 1e-5 && (clip.y - 1.0).abs() < 1e-5);
        assert!(clip.z >= 0.0 && clip.z <= 1.0);
    }
}
