//! Die Spielwelt als **ECS** (Entity Component System).
//!
//! - **Objekt** (Entity): nur eine Nummer, z. B. „Objekt 17“.
//! - **Komponenten**: Bausteine, die man an ein Objekt hängt – Position, Bild, Bewegung, Name …
//!   Ein Objekt ist genau das, was seine Komponenten sagen.
//! - **Systeme**: Funktionen, die alle Objekte mit bestimmten Komponenten durchgehen,
//!   z. B. „bewege alles, was `Transform` und `Bewegung` hat“.
//!
//! Darunter liegt die Bibliothek `hecs`. Sie speichert gleichartige Objekte dicht
//! hintereinander im Speicher – darum bleiben auch 100.000 Objekte schnell.

use glam::Vec2;
pub use hecs::{Entity as Objekt, World as Welt};

use crate::renderer::{Sprite, TexturId};

/// Wo ein Objekt ist, wie es gedreht und wie groß es skaliert ist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    /// Bogenmaß, gegen den Uhrzeigersinn.
    pub drehung: f32,
    pub skala: Vec2,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            drehung: 0.0,
            skala: Vec2::ONE,
        }
    }
}

impl Transform {
    pub fn bei(position: Vec2) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    fn lerp(&self, ziel: &Transform, t: f32) -> Transform {
        Transform {
            position: self.position.lerp(ziel.position, t),
            drehung: self.drehung + (ziel.drehung - self.drehung) * t,
            skala: self.skala.lerp(ziel.skala, t),
        }
    }
}

/// Transform vom letzten Logik-Schritt – wird automatisch gepflegt und nur zum
/// flüssigen Zeichnen (Interpolation) gebraucht.
#[derive(Clone, Copy, Debug)]
pub struct Vorher(pub Transform);

/// Das Objekt wird als Bild gezeichnet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bild {
    pub textur: TexturId,
    /// Größe in Welteinheiten (wird noch mit `Transform::skala` multipliziert).
    pub groesse: Vec2,
    /// RGBA 0..1, wird mit der Textur multipliziert.
    pub farbe: [f32; 4],
    /// Ausschnitt der Textur: x, y, Breite, Höhe (0..1).
    pub uv: [f32; 4],
    /// Zeichenreihenfolge: höhere Ebenen liegen oben. Innerhalb einer Ebene ist die
    /// Reihenfolge nicht festgelegt.
    pub ebene: i32,
}

impl Bild {
    pub fn new(textur: TexturId, groesse: Vec2) -> Self {
        Self {
            textur,
            groesse,
            farbe: [1.0; 4],
            uv: [0.0, 0.0, 1.0, 1.0],
            ebene: 0,
        }
    }
}

/// Gleichmäßige Bewegung pro Sekunde.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bewegung {
    pub linear: Vec2,
    /// Bogenmaß pro Sekunde.
    pub drehung: f32,
}

/// Anzeigename (z. B. für Editor und Szenendateien).
#[derive(Clone, Debug, PartialEq)]
pub struct Name(pub String);

// ---------------------------------------------------------------------------
// Objekt-Nummern für Scripts
// ---------------------------------------------------------------------------

/// Objekt → Zahl. Die Script-Schnittstelle (`wit/timbre.wit`) kennt Objekte nur als `u64`.
pub fn objekt_nummer(o: Objekt) -> u64 {
    o.to_bits().get()
}

/// Zahl → Objekt (None, wenn die Zahl kein gültiges Objekt sein kann).
pub fn objekt_aus_nummer(n: u64) -> Option<Objekt> {
    Objekt::from_bits(n)
}

// ---------------------------------------------------------------------------
// Systeme
// ---------------------------------------------------------------------------

/// Zu Beginn jedes Logik-Schritts: aktuellen Transform als „vorher“ merken.
pub fn schritt_beginnen(welt: &mut Welt) {
    // Neue Objekte bekommen ihre Vorher-Komponente beim ersten Mal.
    let neu: Vec<(Objekt, Transform)> = welt
        .query_mut::<hecs::Without<(Objekt, &Transform), &Vorher>>()
        .into_iter()
        .map(|(o, t)| (o, *t))
        .collect();
    for (o, t) in neu {
        let _ = welt.insert_one(o, Vorher(t));
    }
    for (t, v) in welt.query_mut::<(&Transform, &mut Vorher)>() {
        v.0 = *t;
    }
}

/// Alles mit `Bewegung` weiterbewegen.
pub fn bewegen(welt: &mut Welt, dt: f32) {
    for (t, b) in welt.query_mut::<(&mut Transform, &Bewegung)>() {
        t.position += b.linear * dt;
        t.drehung += b.drehung * dt;
    }
}

/// Alle sichtbaren Objekte als Sprites einsammeln. `alpha` = Anteil zwischen den Logik-Schritten.
pub fn sprites_sammeln(welt: &Welt, alpha: f32, ziel: &mut Vec<Sprite>) {
    ziel.clear();
    for (t, vorher, bild) in welt.query::<(&Transform, Option<&Vorher>, &Bild)>().iter() {
        let t = match vorher {
            Some(v) => v.0.lerp(t, alpha),
            None => *t,
        };
        ziel.push(Sprite {
            position: t.position,
            groesse: bild.groesse * t.skala,
            drehung: t.drehung,
            farbe: bild.farbe,
            textur: bild.textur,
            uv: bild.uv,
            ebene: bild.ebene,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bewegung_und_interpolation() {
        let mut welt = Welt::new();
        let o = welt.spawn((
            Transform::default(),
            Bewegung {
                linear: Vec2::new(10.0, 0.0),
                drehung: 0.0,
            },
        ));
        schritt_beginnen(&mut welt);
        bewegen(&mut welt, 1.0);
        assert_eq!(
            welt.get::<&Transform>(o).unwrap().position,
            Vec2::new(10.0, 0.0)
        );
        assert_eq!(welt.get::<&Vorher>(o).unwrap().0.position, Vec2::ZERO);
    }

    #[test]
    fn sprites_nur_fuer_objekte_mit_bild() {
        let mut welt = Welt::new();
        let textur = TexturId::test(0);
        welt.spawn((
            Transform::bei(Vec2::new(1.0, 2.0)),
            Bild::new(textur, Vec2::splat(4.0)),
        ));
        welt.spawn((Transform::default(),)); // unsichtbar
        let mut sprites = Vec::new();
        sprites_sammeln(&welt, 0.5, &mut sprites);
        assert_eq!(sprites.len(), 1);
        assert_eq!(sprites[0].position, Vec2::new(1.0, 2.0));
    }

    #[test]
    fn nummern_hin_und_zurueck() {
        let mut welt = Welt::new();
        let o = welt.spawn((Name("Kiste".into()),));
        assert_eq!(objekt_aus_nummer(objekt_nummer(o)), Some(o));
    }
}
