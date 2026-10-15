//! Logo, Startbild und kleine Animationen von Timbre Studio (Eis/Arktis).
//!
//! Die Bilder erzeugt `tools/marke_erzeugen.py` nach `assets/marke/` – alles mit Code gezeichnet.

use egui::{Color32, Pos2, Rect, TextureHandle, TextureOptions};

use crate::eis;

/// PNG-Dateien des Logos (für Fenster-Symbol, Leiste und Startbild)
pub const LOGO_64: &[u8] = include_bytes!("../../../assets/marke/logo_64.png");
const LOGO_256: &[u8] = include_bytes!("../../../assets/marke/logo_256.png");
const STARTBILD: &[u8] = include_bytes!("../../../assets/marke/startbild.png");
const KRISTALL: &[u8] = include_bytes!("../../../assets/marke/kristall_512.png");

/// Wie lange die Startanimation dauert (Sekunden)
const START_DAUER: f32 = 2.6;

pub struct Marke {
    pub logo: TextureHandle,
    pub kristall: TextureHandle,
    startbild: TextureHandle,
}

fn textur(ctx: &egui::Context, name: &str, png: &[u8]) -> TextureHandle {
    let (b, h, rgba) = timbre_core::assets::png_lesen(png).unwrap_or((1, 1, vec![0; 4]));
    let bild = egui::ColorImage::from_rgba_unmultiplied([b as usize, h as usize], &rgba);
    ctx.load_texture(name, bild, TextureOptions::LINEAR)
}

impl Marke {
    pub fn laden(ctx: &egui::Context) -> Self {
        Self {
            logo: textur(ctx, "timbre_logo", LOGO_256),
            kristall: textur(ctx, "timbre_kristall", KRISTALL),
            startbild: textur(ctx, "timbre_startbild", STARTBILD),
        }
    }
}

/// Weiches Ein-/Ausblenden: 0 → 1 → 0 über die Startdauer.
fn deckkraft(t: f32) -> f32 {
    let ein = (t / 0.35).clamp(0.0, 1.0);
    let aus = ((START_DAUER - t) / 0.6).clamp(0.0, 1.0);
    let x = ein.min(aus);
    x * x * (3.0 - 2.0 * x)
}

/// Pseudo-Zufall für Schneeflocken (immer gleich, damit nichts flackert)
fn zufall(i: u32, salz: u32) -> f32 {
    let mut x = i
        .wrapping_mul(747_796_405)
        .wrapping_add(salz.wrapping_mul(2_891_336_453));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    (x % 10_000) as f32 / 10_000.0
}

/// Fallender Schnee in einem Bereich. `t` = Zeit in Sekunden, `alpha` = Deckkraft 0..1.
pub fn schnee(maler: &egui::Painter, bereich: Rect, t: f32, anzahl: u32, alpha: f32) {
    for i in 0..anzahl {
        let tempo = 25.0 + zufall(i, 1) * 55.0;
        let groesse = 0.8 + zufall(i, 2) * 2.2;
        let y = (zufall(i, 3) * bereich.height() + t * tempo) % (bereich.height() + 10.0) - 5.0;
        let wiegen = (t * (0.6 + zufall(i, 4)) + zufall(i, 5) * 6.3).sin() * 12.0;
        let x = zufall(i, 6) * bereich.width() + wiegen;
        let a = (alpha * (0.35 + zufall(i, 7) * 0.65) * 255.0) as u8;
        maler.circle_filled(
            Pos2::new(bereich.min.x + x, bereich.min.y + y),
            groesse,
            Color32::from_rgba_unmultiplied(0xE4, 0xF1, 0xFA, a),
        );
    }
}

/// Startanimation über dem ganzen Fenster: Startbild blendet ein, Schnee fällt, Logo leuchtet,
/// dann blendet alles aus. Ein Klick überspringt. Liefert `false`, sobald sie vorbei ist.
pub fn startanimation(ctx: &egui::Context, marke: &Marke, t: f32) -> bool {
    if t >= START_DAUER || ctx.input(|i| i.pointer.any_click() || !i.keys_down.is_empty()) && t > 0.2 {
        return false;
    }
    let alpha = deckkraft(t);
    let flaeche = ctx.content_rect();
    let maler = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("startanimation"),
    ));
    maler.rect_filled(flaeche, 0.0, eis::NACHT.gamma_multiply(alpha));

    // Startbild mittig, so groß wie möglich (Seitenverhältnis 1200:640)
    let verhaeltnis = 1200.0 / 640.0;
    let mut b = flaeche.width() * 0.8;
    let mut h = b / verhaeltnis;
    if h > flaeche.height() * 0.8 {
        h = flaeche.height() * 0.8;
        b = h * verhaeltnis;
    }
    // leichtes Heranzoomen
    let zoom = 0.96 + 0.04 * (t / START_DAUER);
    let bild = Rect::from_center_size(flaeche.center(), egui::vec2(b, h) * zoom);
    maler.image(
        marke.startbild.id(),
        bild,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE.gamma_multiply(alpha),
    );
    // Logo-Glanz: ein heller Schimmer läuft über den Kristall (Kristall liegt bei x≈0.125..0.375 des Bildes)
    let glanz = ((t - 0.5) / 1.2).clamp(0.0, 1.0);
    if glanz > 0.0 && glanz < 1.0 {
        let x = bild.min.x + bild.width() * (0.12 + 0.26 * glanz);
        let streifen = Rect::from_center_size(
            Pos2::new(x, bild.center().y),
            egui::vec2(bild.width() * 0.03, bild.height() * 0.5),
        );
        maler.rect_filled(
            streifen,
            4.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, (60.0 * alpha) as u8),
        );
    }
    schnee(&maler, flaeche, t, 140, alpha);
    maler.text(
        Pos2::new(flaeche.center().x, bild.max.y + 18.0),
        egui::Align2::CENTER_TOP,
        "Klicken zum Überspringen",
        egui::FontId::proportional(12.0),
        eis::NEBEL.gamma_multiply(alpha),
    );
    ctx.request_repaint();
    true
}
