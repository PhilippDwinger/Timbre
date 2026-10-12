//! Einfache 2D-Kollisionen: Kreise und (ungedrehte) Rechtecke.
//!
//! Jeden Logik-Schritt:
//! 1. alle Paare von Objekten mit [`Kollision`] prüfen, die sich überlappen,
//! 2. feste Objekte auseinanderschieben (verankerte bleiben stehen),
//! 3. Berührungen melden – [`Beruehrungen`] merkt sich, welche neu sind (für `on_touch`).
//!
//! Absichtlich schlicht (kein Rollen, keine Reibung, keine Schwerkraft): genug für
//! Top-Down-Spiele, Sammeln, Wände. Bei Bedarf später ersetzen.

use std::collections::{HashMap, HashSet};

use glam::Vec2;

use crate::welt::{welt_transform, welt_transform_setzen, Bewegung, Bild, Form, Kollision, Objekt, Welt};

/// Form eines Objekts in der Welt.
#[derive(Clone, Copy, Debug)]
enum Koerper {
    Kreis { mitte: Vec2, radius: f32 },
    Rechteck { mitte: Vec2, halb: Vec2 },
}

struct Teilnehmer {
    o: Objekt,
    koerper: Koerper,
    k: Kollision,
}

/// Überlappen sich a und b? Liefert die Richtung von a nach b und wie tief sie ineinander stecken.
fn ueberlappung(a: &Koerper, b: &Koerper) -> Option<(Vec2, f32)> {
    use Koerper::*;
    match (*a, *b) {
        (Kreis { mitte: ma, radius: ra }, Kreis { mitte: mb, radius: rb }) => {
            let d = mb - ma;
            let abstand = d.length();
            let tiefe = ra + rb - abstand;
            (tiefe > 0.0).then(|| (if abstand > 1e-6 { d / abstand } else { Vec2::X }, tiefe))
        }
        (Rechteck { mitte: ma, halb: ha }, Rechteck { mitte: mb, halb: hb }) => {
            let d = mb - ma;
            let ox = ha.x + hb.x - d.x.abs();
            let oy = ha.y + hb.y - d.y.abs();
            if ox <= 0.0 || oy <= 0.0 {
                return None;
            }
            let vorzeichen = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
            Some(if ox < oy {
                (Vec2::new(vorzeichen(d.x), 0.0), ox)
            } else {
                (Vec2::new(0.0, vorzeichen(d.y)), oy)
            })
        }
        (Kreis { mitte: mk, radius }, Rechteck { mitte: mr, halb }) => {
            // nächster Punkt des Rechtecks zum Kreismittelpunkt
            let naechster = mk.clamp(mr - halb, mr + halb);
            let d = naechster - mk;
            let abstand = d.length();
            if abstand > 1e-6 {
                let tiefe = radius - abstand;
                (tiefe > 0.0).then(|| (d / abstand, tiefe))
            } else {
                // Kreismitte liegt im Rechteck: kürzester Weg hinaus
                let rel = mk - mr;
                let ox = halb.x - rel.x.abs();
                let oy = halb.y - rel.y.abs();
                let vorzeichen = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
                Some(if ox < oy {
                    (Vec2::new(-vorzeichen(rel.x), 0.0), ox + radius)
                } else {
                    (Vec2::new(0.0, -vorzeichen(rel.y)), oy + radius)
                })
            }
        }
        (Rechteck { .. }, Kreis { .. }) => ueberlappung(b, a).map(|(n, t)| (-n, t)),
    }
}

/// Alle Objekte mit Kollision und Bild einsammeln.
fn teilnehmer(welt: &Welt) -> Vec<Teilnehmer> {
    let mut v = Vec::new();
    for (o, k, bild) in welt.query::<(Objekt, &Kollision, &Bild)>().iter() {
        let Some(t) = welt_transform(welt, o) else { continue };
        let groesse = (bild.groesse * t.skala).abs();
        let koerper = match k.form {
            Form::Kreis => Koerper::Kreis {
                mitte: t.position,
                radius: groesse.min_element() * 0.5,
            },
            Form::Rechteck => Koerper::Rechteck {
                mitte: t.position,
                halb: groesse * 0.5,
            },
        };
        v.push(Teilnehmer { o, koerper, k: *k });
    }
    v
}

/// Ein Paar sich berührender Objekte (kleinere Nummer zuerst).
pub type Paar = (Objekt, Objekt);

fn paar(a: Objekt, b: Objekt) -> Paar {
    if a.to_bits() < b.to_bits() {
        (a, b)
    } else {
        (b, a)
    }
}

/// Kollisionen auflösen und alle Paare liefern, die sich gerade berühren.
pub fn schritt(welt: &mut Welt) -> Vec<Paar> {
    let mut alle = teilnehmer(welt);
    let mut beruehrt = Vec::new();
    // Zweimal auflösen: Wenn ein Objekt aus A heraus in B geschoben wird, wird das gleich mit erledigt.
    for durchgang in 0..2 {
        for i in 0..alle.len() {
            for j in (i + 1)..alle.len() {
                let Some((richtung, tiefe)) = ueberlappung(&alle[i].koerper, &alle[j].koerper) else {
                    continue;
                };
                if durchgang == 0 {
                    beruehrt.push(paar(alle[i].o, alle[j].o));
                }
                let (a, b) = (&alle[i], &alle[j]);
                if !(a.k.fest && b.k.fest) || (a.k.verankert && b.k.verankert) {
                    continue;
                }
                // Anteil, um den jedes Objekt ausweicht
                let (anteil_a, anteil_b) = match (a.k.verankert, b.k.verankert) {
                    (true, false) => (0.0, 1.0),
                    (false, true) => (1.0, 0.0),
                    _ => (0.5, 0.5),
                };
                let verschiebung_a = -richtung * tiefe * anteil_a;
                let verschiebung_b = richtung * tiefe * anteil_b;
                for (index, v) in [(i, verschiebung_a), (j, verschiebung_b)] {
                    if v == Vec2::ZERO {
                        continue;
                    }
                    let o = alle[index].o;
                    if let Some(mut t) = welt_transform(welt, o) {
                        t.position += v;
                        welt_transform_setzen(welt, o, t);
                    }
                    // Geschwindigkeit in die Wand hinein wegnehmen (nicht durch die Wand schieben)
                    if let Ok(mut b) = welt.get::<&mut Bewegung>(o) {
                        let n = v.normalize_or_zero();
                        let hinein = b.linear.dot(n);
                        if hinein < 0.0 {
                            b.linear -= n * hinein;
                        }
                    }
                    alle[index].koerper = match alle[index].koerper {
                        Koerper::Kreis { mitte, radius } => Koerper::Kreis {
                            mitte: mitte + v,
                            radius,
                        },
                        Koerper::Rechteck { mitte, halb } => Koerper::Rechteck { mitte: mitte + v, halb },
                    };
                }
            }
        }
    }
    beruehrt.sort_by_key(|(a, b)| (a.to_bits(), b.to_bits()));
    beruehrt.dedup();
    beruehrt
}

/// Merkt sich die Berührungen des letzten Schritts – um neue zu erkennen (für `on_touch`).
#[derive(Default)]
pub struct Beruehrungen {
    vorher: HashSet<Paar>,
    /// Pro Objekt: was es gerade berührt (für `get_touching`)
    pub je_objekt: HashMap<Objekt, Vec<Objekt>>,
}

impl Beruehrungen {
    /// Aktuelle Paare übernehmen; liefert die Paare, die in diesem Schritt neu dazugekommen sind.
    pub fn aktualisieren(&mut self, aktuell: &[Paar]) -> Vec<Paar> {
        let jetzt: HashSet<Paar> = aktuell.iter().copied().collect();
        let mut neu: Vec<Paar> = aktuell.iter().copied().filter(|p| !self.vorher.contains(p)).collect();
        neu.sort_by_key(|(a, b)| (a.to_bits(), b.to_bits()));
        self.je_objekt.clear();
        for &(a, b) in aktuell {
            self.je_objekt.entry(a).or_default().push(b);
            self.je_objekt.entry(b).or_default().push(a);
        }
        self.vorher = jetzt;
        neu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::TexturId;
    use crate::welt::Transform;

    fn objekt(welt: &mut Welt, x: f32, groesse: f32, k: Kollision) -> Objekt {
        welt.spawn((
            Transform::bei(Vec2::new(x, 0.0)),
            Bild::new(TexturId::test(0), Vec2::splat(groesse)),
            k,
        ))
    }

    #[test]
    fn beweglicher_kreis_wird_aus_verankerter_wand_geschoben() {
        let mut welt = Welt::new();
        let wand = objekt(
            &mut welt,
            0.0,
            100.0,
            Kollision {
                form: Form::Rechteck,
                fest: true,
                verankert: true,
            },
        );
        let ball = objekt(
            &mut welt,
            60.0,
            40.0,
            Kollision {
                form: Form::Kreis,
                fest: true,
                verankert: false,
            },
        );
        let paare = schritt(&mut welt);
        assert_eq!(paare.len(), 1);
        assert_eq!(welt.get::<&Transform>(wand).unwrap().position, Vec2::ZERO);
        let x = welt.get::<&Transform>(ball).unwrap().position.x;
        assert!((x - 70.0).abs() < 1e-3, "x = {x}"); // 50 (Wandrand) + 20 (Radius)
    }

    #[test]
    fn ausloeser_meldet_nur() {
        let mut welt = Welt::new();
        objekt(
            &mut welt,
            0.0,
            100.0,
            Kollision {
                form: Form::Kreis,
                fest: true,
                verankert: false,
            },
        );
        let muenze = objekt(
            &mut welt,
            30.0,
            20.0,
            Kollision {
                form: Form::Kreis,
                fest: false,
                verankert: true,
            },
        );
        assert_eq!(schritt(&mut welt).len(), 1);
        assert_eq!(welt.get::<&Transform>(muenze).unwrap().position.x, 30.0);
    }

    #[test]
    fn neue_beruehrungen_nur_einmal() {
        let mut welt = Welt::new();
        let a = objekt(
            &mut welt,
            0.0,
            50.0,
            Kollision {
                form: Form::Kreis,
                fest: false,
                verankert: true,
            },
        );
        let b = objekt(
            &mut welt,
            10.0,
            50.0,
            Kollision {
                form: Form::Kreis,
                fest: false,
                verankert: true,
            },
        );
        let mut br = Beruehrungen::default();
        assert_eq!(br.aktualisieren(&schritt(&mut welt)), vec![paar(a, b)]);
        assert!(br.aktualisieren(&schritt(&mut welt)).is_empty());
        assert_eq!(br.je_objekt[&a], vec![b]);
    }
}
