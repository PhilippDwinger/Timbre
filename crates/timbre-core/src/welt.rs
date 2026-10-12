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

/// Wo ein Objekt ist, wie es gedreht und wie groß es skaliert ist –
/// **relativ zu seinem Eltern-Objekt** (oder zur Welt, wenn es keins hat).
/// Bewegt sich das Eltern-Objekt, bewegen sich die Kinder mit.
/// Die Lage in der Welt liefert [`welt_transform`].
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

    /// `kind` (relativ zu `self`) → Lage in dem Raum, in dem `self` liegt.
    pub fn verknuepfen(&self, kind: &Transform) -> Transform {
        Transform {
            position: self.position + Vec2::from_angle(self.drehung).rotate(kind.position * self.skala),
            drehung: self.drehung + kind.drehung,
            skala: self.skala * kind.skala,
        }
    }

    /// Gegenstück zu [`Transform::verknuepfen`]: Lage `welt` → relativ zu `self`.
    pub fn relativ(&self, welt: &Transform) -> Transform {
        let teile = |a: Vec2, b: Vec2| {
            Vec2::new(
                if b.x != 0.0 { a.x / b.x } else { 0.0 },
                if b.y != 0.0 { a.y / b.y } else { 0.0 },
            )
        };
        Transform {
            position: teile(
                Vec2::from_angle(-self.drehung).rotate(welt.position - self.position),
                self.skala,
            ),
            drehung: welt.drehung - self.drehung,
            skala: teile(welt.skala, self.skala),
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

/// Das Objekt hängt unter einem anderen (Baum im Explorer).
/// Wird das Eltern-Objekt gelöscht, verschwinden die Kinder mit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eltern(pub Objekt);

/// Das Objekt ist ein Script. Der Quelltext liegt als Datei im Projekt (`scripts/<datei>`).
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    /// Dateiname relativ zum Ordner `scripts/`, z. B. `spieler.luau`
    pub datei: String,
    /// Deaktivierte Scripts laufen bei ▶ nicht mit.
    pub aktiv: bool,
}

// ---------------------------------------------------------------------------
// Baum (Eltern / Kinder)
// ---------------------------------------------------------------------------

/// Direkte Kinder eines Objekts.
pub fn kinder(welt: &Welt, eltern: Objekt) -> Vec<Objekt> {
    welt.query::<(Objekt, &Eltern)>()
        .iter()
        .filter(|(_, e)| e.0 == eltern)
        .map(|(o, _)| o)
        .collect()
}

/// Das Objekt und alle Kinder, Enkel … (Objekt selbst zuerst).
pub fn mit_nachkommen(welt: &Welt, o: Objekt) -> Vec<Objekt> {
    let mut alle = vec![o];
    let mut i = 0;
    while i < alle.len() {
        for k in kinder(welt, alle[i]) {
            if !alle.contains(&k) {
                alle.push(k);
            }
        }
        i += 1;
    }
    alle
}

/// Objekt samt allen Nachkommen löschen.
pub fn entfernen(welt: &mut Welt, o: Objekt) {
    for x in mit_nachkommen(welt, o) {
        let _ = welt.despawn(x);
    }
}

// ---------------------------------------------------------------------------
// Lage in der Welt
// ---------------------------------------------------------------------------

/// Nächstes Vorfahren-Objekt mit eigenem Transform (Ordner und Scripts ohne Transform zählen nicht).
fn transform_eltern(welt: &Welt, o: Objekt) -> Option<Objekt> {
    let mut e = welt.get::<&Eltern>(o).ok().map(|e| e.0);
    let mut schritte = 0;
    while let Some(x) = e {
        if !welt.contains(x) || schritte > 10_000 {
            return None;
        }
        if welt.get::<&Transform>(x).is_ok() {
            return Some(x);
        }
        e = welt.get::<&Eltern>(x).ok().map(|e| e.0);
        schritte += 1;
    }
    None
}

/// Lage eines Objekts in der Welt (alle Eltern eingerechnet). `None` ohne Transform.
pub fn welt_transform(welt: &Welt, o: Objekt) -> Option<Transform> {
    let eigen = *welt.get::<&Transform>(o).ok()?;
    Some(match transform_eltern(welt, o).and_then(|e| welt_transform(welt, e)) {
        Some(e) => e.verknuepfen(&eigen),
        None => eigen,
    })
}

/// Lage in der Welt setzen – rechnet in die Lage relativ zum Eltern-Objekt um.
pub fn welt_transform_setzen(welt: &mut Welt, o: Objekt, ziel: Transform) {
    let lokal = match transform_eltern(welt, o).and_then(|e| welt_transform(welt, e)) {
        Some(e) => e.relativ(&ziel),
        None => ziel,
    };
    if let Ok(mut t) = welt.get::<&mut Transform>(o) {
        *t = lokal;
    }
}

/// Objekt umhängen (`None` = oberste Ebene). Die Lage in der Welt bleibt gleich,
/// das Objekt springt also nicht. Schleifen (unter sich selbst / eigene Kinder) werden ignoriert.
/// Liefert `true`, wenn umgehängt wurde.
pub fn eltern_setzen(welt: &mut Welt, o: Objekt, neu: Option<Objekt>) -> bool {
    if let Some(e) = neu {
        if !welt.contains(e) || ist_nachkomme(welt, e, o) {
            return false;
        }
    }
    let vorher = welt_transform(welt, o);
    match neu {
        Some(e) => {
            let _ = welt.insert_one(o, Eltern(e));
        }
        None => {
            let _ = welt.remove_one::<Eltern>(o);
        }
    }
    if let Some(t) = vorher {
        welt_transform_setzen(welt, o, t);
        // Interpolation nicht durch den Sprung stören
        if let Ok(mut v) = welt.get::<&mut Vorher>(o) {
            v.0 = *welt.get::<&Transform>(o).unwrap();
        }
    }
    true
}

/// Liegt `vielleicht_nachkomme` unter `o` (oder ist es selbst)? Schützt vor Schleifen im Baum.
pub fn ist_nachkomme(welt: &Welt, vielleicht_nachkomme: Objekt, o: Objekt) -> bool {
    let mut aktuell = Some(vielleicht_nachkomme);
    let mut schritte = 0;
    while let Some(x) = aktuell {
        if x == o {
            return true;
        }
        aktuell = welt.get::<&Eltern>(x).ok().map(|e| e.0);
        schritte += 1;
        if schritte > 10_000 {
            return true; // kaputter Baum – sicherheitshalber „ja“
        }
    }
    false
}

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
/// Kinder werden mit der (überblendeten) Lage ihrer Eltern verrechnet.
pub fn sprites_sammeln(welt: &Welt, alpha: f32, ziel: &mut Vec<Sprite>) {
    ziel.clear();
    let mut merk: std::collections::HashMap<Objekt, Option<Transform>> = std::collections::HashMap::new();
    for (o, bild) in welt.query::<(Objekt, &Bild)>().iter() {
        let Some(t) = welt_transform_ueberblendet(welt, o, alpha, &mut merk) else {
            continue;
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

/// Wie [`welt_transform`], aber zwischen letztem und aktuellem Logik-Schritt überblendet.
/// `merk` vermeidet, dass gemeinsame Eltern (z. B. ein Ordner mit 400 Blättern) mehrfach gerechnet werden.
fn welt_transform_ueberblendet(
    welt: &Welt,
    o: Objekt,
    alpha: f32,
    merk: &mut std::collections::HashMap<Objekt, Option<Transform>>,
) -> Option<Transform> {
    if let Some(t) = merk.get(&o) {
        return *t;
    }
    let ergebnis = welt.get::<&Transform>(o).ok().map(|t| {
        let eigen = match welt.get::<&Vorher>(o) {
            Ok(v) => v.0.lerp(&t, alpha),
            Err(_) => *t,
        };
        match transform_eltern(welt, o).and_then(|e| welt_transform_ueberblendet(welt, e, alpha, merk)) {
            Some(e) => e.verknuepfen(&eigen),
            None => eigen,
        }
    });
    merk.insert(o, ergebnis);
    ergebnis
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
        assert_eq!(welt.get::<&Transform>(o).unwrap().position, Vec2::new(10.0, 0.0));
        assert_eq!(welt.get::<&Vorher>(o).unwrap().0.position, Vec2::ZERO);
    }

    #[test]
    fn sprites_nur_fuer_objekte_mit_bild() {
        let mut welt = Welt::new();
        let textur = TexturId::test(0);
        welt.spawn((Transform::bei(Vec2::new(1.0, 2.0)), Bild::new(textur, Vec2::splat(4.0))));
        welt.spawn((Transform::default(),)); // unsichtbar
        let mut sprites = Vec::new();
        sprites_sammeln(&welt, 0.5, &mut sprites);
        assert_eq!(sprites.len(), 1);
        assert_eq!(sprites[0].position, Vec2::new(1.0, 2.0));
    }

    #[test]
    fn kinder_werden_mitgeloescht() {
        let mut welt = Welt::new();
        let a = welt.spawn((Name("a".into()),));
        let b = welt.spawn((Name("b".into()), Eltern(a)));
        let c = welt.spawn((Name("c".into()), Eltern(b)));
        let d = welt.spawn((Name("d".into()),));
        assert!(ist_nachkomme(&welt, c, a));
        assert!(!ist_nachkomme(&welt, a, c));
        entfernen(&mut welt, a);
        assert!(!welt.contains(b) && !welt.contains(c) && welt.contains(d));
    }

    #[test]
    fn kinder_bewegen_sich_mit() {
        let mut welt = Welt::new();
        let e = welt.spawn((Transform {
            position: Vec2::new(100.0, 0.0),
            drehung: std::f32::consts::FRAC_PI_2,
            skala: Vec2::splat(2.0),
        },));
        let ordner = welt.spawn((Name("Ordner".into()), Eltern(e))); // ohne Transform: zählt nicht
        let k = welt.spawn((Transform::bei(Vec2::new(10.0, 0.0)), Eltern(ordner)));
        // Kind liegt 10 rechts vom Eltern → gedreht um 90° und doppelt groß: 20 nach oben
        let w = welt_transform(&welt, k).unwrap();
        assert!(
            (w.position - Vec2::new(100.0, 20.0)).length() < 1e-3,
            "{:?}",
            w.position
        );
        assert_eq!(w.skala, Vec2::splat(2.0));

        // Umhängen behält die Lage in der Welt
        assert!(eltern_setzen(&mut welt, k, None));
        assert!((welt_transform(&welt, k).unwrap().position - Vec2::new(100.0, 20.0)).length() < 1e-3);
        assert!(eltern_setzen(&mut welt, k, Some(e)));
        assert!((welt_transform(&welt, k).unwrap().position - Vec2::new(100.0, 20.0)).length() < 1e-3);
        // Schleife wird verweigert
        assert!(!eltern_setzen(&mut welt, e, Some(k)));

        // Weltlage setzen
        welt_transform_setzen(&mut welt, k, Transform::bei(Vec2::new(0.0, 0.0)));
        assert!(welt_transform(&welt, k).unwrap().position.length() < 1e-3);
    }

    #[test]
    fn nummern_hin_und_zurueck() {
        let mut welt = Welt::new();
        let o = welt.spawn((Name("Kiste".into()),));
        assert_eq!(objekt_aus_nummer(objekt_nummer(o)), Some(o));
    }
}
