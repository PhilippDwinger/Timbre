//! Das Aussehen von Timbre Studio: Eis / Arktis.
//!
//! Tiefes Polarnacht-Blau als Grund, Gletscher- und Eisblau für Akzente, Schneeweiß für Text.
//! Alle Farben der Oberfläche kommen von hier, damit das Studio wie aus einem Guss wirkt.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Hintergrund der Fenster (Polarnacht)
pub const NACHT: Color32 = Color32::from_rgb(0x0B, 0x16, 0x24);
/// Panels
pub const PANEL: Color32 = Color32::from_rgb(0x10, 0x1F, 0x31);
/// Blöcke in den Eigenschaften, Eingabefelder
pub const BLOCK: Color32 = Color32::from_rgb(0x15, 0x29, 0x3F);
/// Rahmen
pub const RAND: Color32 = Color32::from_rgb(0x24, 0x40, 0x5C);
/// Helles Eisblau (Überschriften, Auswahl)
pub const EIS: Color32 = Color32::from_rgb(0xBF, 0xE8, 0xFF);
/// Gletscherblau (Knöpfe, Links, Akzente)
pub const AKZENT: Color32 = Color32::from_rgb(0x5C, 0xC8, 0xF0);
/// Tiefes Gletscherblau (gedrückt / aktiv)
pub const GLETSCHER: Color32 = Color32::from_rgb(0x1E, 0x7F, 0xB8);
/// Schnee (normaler Text)
pub const SCHNEE: Color32 = Color32::from_rgb(0xE4, 0xF1, 0xFA);
/// Gedämpfter Text
pub const NEBEL: Color32 = Color32::from_rgb(0x7E, 0x9A, 0xB2);
/// Nordlicht-Grün (▶ Ausführen)
pub const NORDLICHT: Color32 = Color32::from_rgb(0x2F, 0xB8, 0x8E);
/// Warmes Rot (■ Stopp, Fehler) – bleibt bewusst warm, damit man es sofort sieht
pub const GLUT: Color32 = Color32::from_rgb(0xE0, 0x5A, 0x4F);

/// Eis-Stil auf die egui-Oberfläche anwenden.
pub fn anwenden(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = NACHT;
    v.faint_bg_color = BLOCK;
    v.code_bg_color = NACHT;
    v.override_text_color = Some(SCHNEE);
    v.hyperlink_color = AKZENT;
    v.selection.bg_fill = GLETSCHER;
    v.selection.stroke = Stroke::new(1.0, EIS);
    v.window_stroke = Stroke::new(1.0, RAND);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(6);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, RAND);
    w.noninteractive.fg_stroke = Stroke::new(1.0, SCHNEE);
    w.inactive.bg_fill = BLOCK;
    w.inactive.weak_bg_fill = BLOCK;
    w.inactive.bg_stroke = Stroke::new(1.0, RAND);
    w.inactive.fg_stroke = Stroke::new(1.0, SCHNEE);
    w.hovered.bg_fill = Color32::from_rgb(0x1D, 0x3A, 0x57);
    w.hovered.weak_bg_fill = Color32::from_rgb(0x1D, 0x3A, 0x57);
    w.hovered.bg_stroke = Stroke::new(1.0, AKZENT);
    w.hovered.fg_stroke = Stroke::new(1.5, EIS);
    w.active.bg_fill = GLETSCHER;
    w.active.weak_bg_fill = GLETSCHER;
    w.active.bg_stroke = Stroke::new(1.0, EIS);
    w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.open.bg_fill = BLOCK;
    w.open.weak_bg_fill = BLOCK;
    w.open.bg_stroke = Stroke::new(1.0, AKZENT);
    for zustand in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        zustand.corner_radius = CornerRadius::same(5);
    }
    // Immer dunkel – unabhängig von der Windows-Einstellung
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);

    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(7.0, 5.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        // Weiche Übergänge (Aufklappen, Hover)
        s.animation_time = 0.12;
    });
}
