//! 3D-Ansicht des Studios: Kamera drehen/schieben/zoomen, Körper anklicken und verschieben,
//! Boden-Gitter, Properties „Transform 3D“ und „Mesh (3D)“.
//!
//! Gezeichnet werden die Körper vom `Renderer3d` (in `main.rs`), alles andere (Gitter, Auswahlrahmen,
//! Namen leerer Objekte) zeichnet egui darüber.

use egui::{Color32, PointerButton, RichText, Sense, Stroke};
use timbre_core::glam::{Mat4, Vec3};
use timbre_core::welt::{self, Mesh3d, Objekt, Raum};
use timbre_core::{vec2, Bereich, Form3d, Kamera3d, Modell, Vec2};

use super::{property_block, Modus, Property, Studio};
use crate::eis;

/// Bildschirmpunkt (egui) eines Weltpunkts – `None`, wenn er hinter der Kamera liegt.
fn projizieren(m: &Mat4, p: Vec3, rect: egui::Rect) -> Option<egui::Pos2> {
    let c = *m * p.extend(1.0);
    if c.w <= 0.05 {
        return None;
    }
    let n = c.truncate() / c.w;
    Some(egui::pos2(
        rect.min.x + (n.x + 1.0) * 0.5 * rect.width(),
        rect.min.y + (1.0 - n.y) * 0.5 * rect.height(),
    ))
}

/// Abstand entlang des Strahls bis zum Treffer mit einem Körper (in dessen eigenem Raum, Größe 1).
fn strahl_trifft(form: Form3d, start: Vec3, richtung: Vec3) -> Option<f32> {
    match form {
        Form3d::Kugel => {
            // |start + t·r|² = 0.25
            let a = richtung.length_squared();
            let b = 2.0 * start.dot(richtung);
            let c = start.length_squared() - 0.25;
            let d = b * b - 4.0 * a * c;
            if d < 0.0 {
                return None;
            }
            let t = (-b - d.sqrt()) / (2.0 * a);
            (t >= 0.0).then_some(t)
        }
        _ => {
            // Quader (Ebene: ganz flach, Zylinder: als Quader genähert)
            let halb = if form == Form3d::Ebene {
                Vec3::new(0.5, 0.02, 0.5)
            } else {
                Vec3::splat(0.5)
            };
            let mut t_min = f32::NEG_INFINITY;
            let mut t_max = f32::INFINITY;
            for i in 0..3 {
                let (s, r, h) = (start[i], richtung[i], halb[i]);
                if r.abs() < 1e-8 {
                    if s.abs() > h {
                        return None;
                    }
                } else {
                    let a = (-h - s) / r;
                    let b = (h - s) / r;
                    t_min = t_min.max(a.min(b));
                    t_max = t_max.min(a.max(b));
                }
            }
            (t_max >= t_min.max(0.0)).then_some(t_min.max(0.0))
        }
    }
}

/// Farbe für den 3D-Renderer (linear, wie im Shader)
fn linear(c: Color32, deckkraft: f32) -> [f32; 4] {
    let r = egui::Rgba::from(c);
    [r.r(), r.g(), r.b(), deckkraft]
}

/// Boden-Gitter (y = 0) als ganz flache, unbeleuchtete Quader – so verdecken Körper die Linien richtig.
/// Achsen: X rot, Z blau, jede 5. Linie heller.
fn gitter_linien(kamera: &Kamera3d, ziel: &mut Vec<Modell>) {
    let weite = 20i32;
    let mitte = Vec3::new(kamera.ziel.x.round(), 0.0, kamera.ziel.z.round());
    let dicke = (kamera.abstand * 0.0016).clamp(0.008, 0.2);
    let laenge = weite as f32 * 2.0;
    for i in -weite..=weite {
        let f = i as f32;
        for entlang_x in [true, false] {
            let (pos, skala, welt_wert) = if entlang_x {
                (
                    mitte + Vec3::new(0.0, 0.0, f),
                    Vec3::new(laenge, 0.0005, dicke),
                    mitte.z + f,
                )
            } else {
                (
                    mitte + Vec3::new(f, 0.0, 0.0),
                    Vec3::new(dicke, 0.0005, laenge),
                    mitte.x + f,
                )
            };
            // halbdurchsichtig, damit das Gitter auf hellen Böden nicht zu hart wirkt
            let farbe = if welt_wert == 0.0 && entlang_x {
                linear(Color32::from_rgb(0xD0, 0x50, 0x50), 0.8)
            } else if welt_wert == 0.0 {
                linear(Color32::from_rgb(0x50, 0x88, 0xE0), 0.8)
            } else if (welt_wert as i32) % 5 == 0 {
                linear(eis::GLETSCHER, 0.35)
            } else {
                linear(eis::GLETSCHER, 0.16)
            };
            ziel.push(Modell {
                form: Form3d::Wuerfel,
                matrix: Mat4::from_scale_rotation_translation(skala, Default::default(), pos + Vec3::Y * 0.002),
                farbe,
                unbeleuchtet: true,
            });
        }
    }
}

impl Studio {
    pub fn ist_3d(&self) -> bool {
        self.ansicht3d
    }

    /// Körper einsammeln, die der 3D-Renderer zeichnen soll – im Bearbeiten-Modus mit Boden-Gitter.
    pub fn modelle_bauen(&mut self) -> (Kamera3d, &[Modell]) {
        welt::modelle_sammeln(&self.welt, &mut self.modelle);
        if self.modus == Modus::Bearbeiten {
            gitter_linien(&self.kamera3d, &mut self.modelle);
        }
        (self.kamera3d, &self.modelle)
    }

    /// WASD im Bearbeiten-Modus: Kamera-Ziel waagerecht verschieben (in Blickrichtung).
    pub(super) fn kamera3d_bewegen(&mut self, r: Vec2, dt: f32) {
        if r == Vec2::ZERO {
            return;
        }
        let k = &mut self.kamera3d;
        let vorne = Vec3::new(-k.gier.sin(), 0.0, -k.gier.cos());
        let rechts = Vec3::new(k.gier.cos(), 0.0, -k.gier.sin());
        let r = r.normalize();
        k.ziel += (vorne * r.y + rechts * r.x) * k.abstand * 0.9 * dt;
    }

    /// Kamera auf ein 3D-Objekt richten (Doppelklick im Explorer).
    pub(super) fn kamera3d_zu(&mut self, o: Objekt) -> bool {
        match welt::welt_raum(&self.welt, o) {
            Some(m) => {
                self.kamera3d.ziel = m.w_axis.truncate();
                true
            }
            None => false,
        }
    }

    /// Welt-Position eines 3D-Objekts setzen (Raum ist relativ zum Eltern-Raum).
    fn welt_position3d_setzen(&mut self, o: Objekt, p: Vec3) {
        welt::welt_position3d_setzen(&mut self.welt, o, p);
    }

    /// Nächstes Objekt unter dem Mauszeiger: Körper per Strahl, leere 3D-Objekte per Abstand am Bildschirm.
    fn treffer3d(&self, start: Vec3, richtung: Vec3, maus: egui::Pos2, m: &Mat4, rect: egui::Rect) -> Option<Objekt> {
        let mut bester: Option<(f32, Objekt)> = None;
        for (o, mesh) in self.welt.query::<(Objekt, &Mesh3d)>().iter() {
            let Some(welt_m) = welt::welt_raum(&self.welt, o) else {
                continue;
            };
            let inv = welt_m.inverse();
            let s = inv.transform_point3(start);
            let r = inv.transform_vector3(richtung);
            if let Some(t_lokal) = strahl_trifft(mesh.form, s, r) {
                // zurück in Weltabstand, damit verschieden große Körper vergleichbar sind
                let t = welt_m.transform_point3(s + r * t_lokal).distance(start);
                if bester.is_none_or(|(b, _)| t < b) {
                    bester = Some((t, o));
                }
            }
        }
        if let Some((_, o)) = bester {
            return Some(o);
        }
        self.welt
            .query::<(Objekt, &Raum)>()
            .without::<&Mesh3d>()
            .iter()
            .filter_map(|(o, _)| {
                let p = welt::welt_raum(&self.welt, o)?.w_axis.truncate();
                let s = projizieren(m, p, rect)?;
                (s.distance(maus) < 10.0).then_some(o)
            })
            .next()
    }

    pub(super) fn szene3d_ui(&mut self, ui: &mut egui::Ui) {
        let (rect, antwort) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        self.bereich = Some(Bereich {
            x: rect.min.x * ppp,
            y: rect.min.y * ppp,
            breite: rect.width() * ppp,
            hoehe: rect.height() * ppp,
        });
        self.maus_ueber_szene = antwort.hovered();
        let bearbeiten = self.modus == Modus::Bearbeiten;
        let umschalt = ui.input(|i| i.modifiers.shift);

        // Kamera: rechte Maus = drehen, mittlere oder Umschalt+rechte = schieben, Rad = zoomen
        let schieben =
            antwort.dragged_by(PointerButton::Middle) || (antwort.dragged_by(PointerButton::Secondary) && umschalt);
        if schieben {
            let d = antwort.drag_delta();
            let k = &mut self.kamera3d;
            let rechts = Vec3::new(k.gier.cos(), 0.0, -k.gier.sin());
            let vorne = (k.ziel - k.position()).normalize_or_zero();
            let hoch = rechts.cross(vorne);
            let faktor = k.abstand * (k.sichtfeld * 0.5).tan() * 2.0 / rect.height().max(1.0);
            k.ziel += (-rechts * d.x + hoch * d.y) * faktor;
        } else if antwort.dragged_by(PointerButton::Secondary) {
            let d = antwort.drag_delta();
            self.kamera3d.gier -= d.x * 0.008;
            self.kamera3d.neigung = (self.kamera3d.neigung + d.y * 0.008).clamp(-1.5, 1.5);
        }
        if antwort.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.kamera3d.abstand = (self.kamera3d.abstand * 0.998f32.powf(scroll)).clamp(0.5, 500.0);
            }
        }

        let kamera = self.kamera3d;
        let m = kamera.matrix(rect.width() / rect.height().max(1.0));
        let painter = ui.painter().with_clip_rect(rect);
        let strahl = |p: egui::Pos2| {
            kamera.strahl(
                vec2(p.x - rect.min.x, p.y - rect.min.y),
                vec2(rect.width(), rect.height()),
            )
        };

        // Leere 3D-Objekte (ohne Mesh) als Raute mit Namen
        if bearbeiten {
            let leere: Vec<(Objekt, Vec3)> = self
                .welt
                .query::<(Objekt, &Raum)>()
                .without::<&Mesh3d>()
                .iter()
                .filter_map(|(o, _)| welt::welt_raum(&self.welt, o).map(|w| (o, w.w_axis.truncate())))
                .collect();
            for (o, p) in leere {
                let Some(q) = projizieren(&m, p, rect) else { continue };
                let farbe = if self.auswahl == Some(o) {
                    eis::EIS
                } else {
                    eis::AKZENT.gamma_multiply(0.8)
                };
                let r = 6.0;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        q + egui::vec2(0.0, -r),
                        q + egui::vec2(r, 0.0),
                        q + egui::vec2(0.0, r),
                        q + egui::vec2(-r, 0.0),
                    ],
                    eis::GLETSCHER.gamma_multiply(0.35),
                    Stroke::new(1.5, farbe),
                ));
                painter.text(
                    q + egui::vec2(9.0, -9.0),
                    egui::Align2::LEFT_BOTTOM,
                    self.name(o),
                    egui::FontId::proportional(11.0),
                    farbe,
                );
            }
        }

        // Auswahl: Rahmen um den Körper (12 Kanten des Würfels in dessen eigenem Raum)
        if let Some(o) = self.auswahl {
            if let (Some(welt_m), Ok(mesh)) = (welt::welt_raum(&self.welt, o), self.welt.get::<&Mesh3d>(o).map(|x| *x))
            {
                let h = if mesh.form == Form3d::Ebene {
                    Vec3::new(0.5, 0.0, 0.5)
                } else {
                    Vec3::splat(0.5)
                };
                let ecke = |x: f32, y: f32, z: f32| welt_m.transform_point3(Vec3::new(x * h.x, y * h.y, z * h.z));
                let mut kanten = Vec::new();
                for a in [-1.0, 1.0] {
                    for b in [-1.0, 1.0] {
                        kanten.push((ecke(-1.0, a, b), ecke(1.0, a, b)));
                        kanten.push((ecke(a, -1.0, b), ecke(a, 1.0, b)));
                        kanten.push((ecke(a, b, -1.0), ecke(a, b, 1.0)));
                    }
                }
                for (a, b) in kanten {
                    if let (Some(p), Some(q)) = (projizieren(&m, a, rect), projizieren(&m, b, rect)) {
                        painter.line_segment([p, q], Stroke::new(1.5, eis::EIS));
                    }
                }
                if let Some(p) = projizieren(&m, welt_m.w_axis.truncate(), rect) {
                    painter.circle_filled(p, 3.0, eis::EIS);
                }
            }
        }

        // Auswählen und Verschieben (linke Maus, nur im Bearbeiten-Modus).
        // Ziehen bewegt waagerecht, mit Umschalt senkrecht. Gitter an = auf 0,5 einrasten (Alt = frei).
        if bearbeiten {
            let einrasten = self.gitter && !ui.input(|i| i.modifiers.alt);
            if antwort.drag_started_by(PointerButton::Primary) || antwort.clicked_by(PointerButton::Primary) {
                if let Some(maus) = antwort.interact_pointer_pos() {
                    let (start, richtung) = strahl(maus);
                    let o = self.treffer3d(start, richtung, maus, &m, rect);
                    self.auswahl = o;
                    self.ziehen3d = None;
                    if let (Some(o), true) = (o, antwort.drag_started_by(PointerButton::Primary)) {
                        if let Some(p) = welt::welt_raum(&self.welt, o).map(|w| w.w_axis.truncate()) {
                            // Treffpunkt auf der waagerechten Ebene durch das Objekt
                            let t = if richtung.y.abs() > 1e-4 {
                                (p.y - start.y) / richtung.y
                            } else {
                                0.0
                            };
                            let treffer = start + richtung * t.max(0.0);
                            self.ziehen3d = Some((o, p - treffer, p.y));
                        }
                    }
                }
            }
            if antwort.dragged_by(PointerButton::Primary) {
                if let (Some((o, versatz, hoehe)), Some(maus)) = (self.ziehen3d, antwort.interact_pointer_pos()) {
                    if let Some(alt) = welt::welt_raum(&self.welt, o).map(|w| w.w_axis.truncate()) {
                        let mut neu = if umschalt {
                            // senkrecht: Mausbewegung nach oben = höher
                            let faktor = kamera.abstand * (kamera.sichtfeld * 0.5).tan() * 2.0 / rect.height().max(1.0);
                            let y = alt.y - antwort.drag_delta().y * faktor;
                            self.ziehen3d = Some((o, versatz, y));
                            Vec3::new(alt.x, y, alt.z)
                        } else {
                            let (start, richtung) = strahl(maus);
                            if richtung.y.abs() < 1e-4 {
                                alt
                            } else {
                                let t = ((hoehe - start.y) / richtung.y).clamp(0.0, 1000.0);
                                let p = start + richtung * t + versatz;
                                Vec3::new(p.x, hoehe, p.z)
                            }
                        };
                        if einrasten {
                            let raster = |v: f32| (v * 2.0).round() / 2.0;
                            neu = if umschalt {
                                Vec3::new(neu.x, raster(neu.y), neu.z)
                            } else {
                                Vec3::new(raster(neu.x), neu.y, raster(neu.z))
                            };
                        }
                        if neu != alt {
                            self.welt_position3d_setzen(o, neu);
                            self.geaendert = true;
                        }
                    }
                }
            }
            if antwort.drag_stopped() && self.ziehen3d.take().is_some() {
                self.markieren();
            }
        }

        // Hilfe unten links
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            "Rechte Maus: drehen · Mitte/Umschalt+Rechts: schieben · Rad: zoomen · WASD: fahren · Ziehen: verschieben (Umschalt = hoch/runter)",
            egui::FontId::proportional(11.0),
            eis::NEBEL.gamma_multiply(0.7),
        );
    }

    /// Property-Blöcke „Transform 3D“ und „Mesh (3D)“. Liefert das Property, dessen ✕ geklickt wurde.
    pub(super) fn properties3d_ui(&mut self, ui: &mut egui::Ui, o: Objekt) -> Option<Property> {
        let mut entfernen = None;
        if let Ok(mut r) = self.welt.get::<&Raum>(o).map(|r| *r) {
            let vorher = r;
            let drei = |ui: &mut egui::Ui, v: &mut Vec3, tempo: f64, suffix: &str| {
                ui.horizontal(|ui| {
                    for (wert, achse) in [(&mut v.x, "x "), (&mut v.y, "y "), (&mut v.z, "z ")] {
                        ui.add(egui::DragValue::new(wert).speed(tempo).prefix(achse).suffix(suffix));
                    }
                });
            };
            if property_block(ui, Property::Raum, |ui| {
                egui::Grid::new("raum").num_columns(2).show(ui, |ui| {
                    ui.label("Position");
                    drei(ui, &mut r.position, 0.05, "");
                    ui.end_row();
                    ui.label("Drehung");
                    drei(ui, &mut r.drehung, 1.0, "°");
                    ui.end_row();
                    ui.label("Skala");
                    drei(ui, &mut r.skala, 0.02, "");
                    ui.end_row();
                });
                if !self.ansicht3d {
                    ui.label(RichText::new("Sichtbar in der 3D-Ansicht (oben: 3D)").weak().small());
                }
            }) {
                entfernen = Some(Property::Raum);
            }
            if r != vorher {
                let _ = self.welt.insert_one(o, r);
                self.markieren();
            }
        }
        if let Ok(mut mesh) = self.welt.get::<&Mesh3d>(o).map(|m| *m) {
            let vorher = mesh;
            if property_block(ui, Property::Mesh, |ui| {
                egui::Grid::new("mesh").num_columns(2).show(ui, |ui| {
                    ui.label("Form");
                    egui::ComboBox::from_id_salt("form3d")
                        .selected_text(mesh.form.name())
                        .show_ui(ui, |ui| {
                            for f in Form3d::ALLE {
                                ui.selectable_value(&mut mesh.form, f, f.name());
                            }
                        });
                    ui.end_row();
                    ui.label("Farbe");
                    ui.color_edit_button_rgba_unmultiplied(&mut mesh.farbe);
                    ui.end_row();
                });
            }) {
                entfernen = Some(Property::Mesh);
            }
            if mesh != vorher {
                let _ = self.welt.insert_one(o, mesh);
                self.markieren();
            }
        }
        entfernen
    }

    /// Neuer 3D-Körper (Explorer „+ Körper“): Objekt mit Transform 3D + Mesh am Kamera-Ziel.
    pub(super) fn koerper_anlegen(&mut self, form: Form3d) {
        self.markieren();
        let mut position = self.kamera3d.ziel;
        if form != Form3d::Ebene {
            position.y = position.y.max(0.5); // auf dem Boden stehen statt halb darin
        }
        let skala = if form == Form3d::Ebene {
            Vec3::new(10.0, 1.0, 10.0)
        } else {
            Vec3::ONE
        };
        // Nicht genau dorthin, wo schon ein Körper steht: nach rechts ausweichen
        if form != Form3d::Ebene {
            let belegt: Vec<Vec3> = self
                .welt
                .query::<(Objekt, &Mesh3d)>()
                .iter()
                .filter(|(_, m)| m.form != Form3d::Ebene)
                .filter_map(|(o, _)| welt::welt_raum(&self.welt, o).map(|w| w.w_axis.truncate()))
                .collect();
            for _ in 0..50 {
                if belegt.iter().all(|p| p.distance(position) > 0.9) {
                    break;
                }
                position.x += 1.5;
            }
        }
        let farbe = match form {
            Form3d::Ebene => [0.80, 0.86, 0.92, 1.0],
            Form3d::Kugel => [0.50, 0.85, 0.80, 1.0],
            Form3d::Zylinder => [0.62, 0.66, 0.88, 1.0],
            Form3d::Wuerfel => [0.55, 0.78, 0.95, 1.0],
        };
        let o = self.welt.spawn((
            welt::Name(form.name().into()),
            Raum {
                position,
                skala,
                ..Default::default()
            },
            Mesh3d { form, farbe },
        ));
        self.auswahl = Some(o);
        self.ansicht3d = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strahl_trifft_koerper() {
        // von vorne auf einen Würfel
        let t = strahl_trifft(Form3d::Wuerfel, Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z).unwrap();
        assert!((t - 4.5).abs() < 1e-4);
        let t = strahl_trifft(Form3d::Kugel, Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z).unwrap();
        assert!((t - 4.5).abs() < 1e-4);
        // knapp vorbei
        assert!(strahl_trifft(Form3d::Kugel, Vec3::new(0.6, 0.0, 5.0), Vec3::NEG_Z).is_none());
        assert!(strahl_trifft(Form3d::Wuerfel, Vec3::new(0.0, 0.6, 5.0), Vec3::NEG_Z).is_none());
    }
}
