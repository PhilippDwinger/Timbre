//! Szenen speichern und laden.
//!
//! Eine Szene ist eine `.tbr`-Datei vom Typ Szene; ihr Inhalt ist **JSON** – lesbar,
//! und auch Grove (JavaScript) und Bridge (Python) können es ohne Extra-Bibliothek lesen.
//! Texturen werden über ihre Asset-ID referenziert, nie über Dateinamen.
//!
//! Beispiel:
//! ```json
//! {
//!   "format": 0,
//!   "objekte": [
//!     {
//!       "name": "Baumscheibe",
//!       "position": [120.0, -40.0],
//!       "drehung": 0.5,
//!       "bild": { "textur": "a3ad2044fc94a04824acb65790552b33", "groesse": [48.0, 48.0], "ebene": 0 },
//!       "bewegung": { "linear": [30.0, 0.0], "drehung": 1.0 }
//!     }
//!   ]
//! }
//! ```
//! Felder, die fehlen, bekommen Standardwerte (Skala 1, Farbe weiß, ganze Textur …).

use glam::Vec2;
use serde::{Deserialize, Serialize};
use timbre_tbr::{AssetId, Szene};

use crate::assets::Assets;
use crate::welt::{Bewegung, Bild, Name, Transform, Welt};

/// Version des JSON-Inhalts (unabhängig von der `.tbr`-Formatversion).
const SZENEN_FORMAT: u32 = 0;

#[derive(Serialize, Deserialize)]
struct SzeneDatei {
    format: u32,
    objekte: Vec<ObjektDatei>,
}

#[derive(Serialize, Deserialize)]
struct ObjektDatei {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    position: [f32; 2],
    #[serde(default, skip_serializing_if = "ist_null")]
    drehung: f32,
    #[serde(default = "eins", skip_serializing_if = "ist_eins")]
    skala: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bild: Option<BildDatei>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bewegung: Option<BewegungDatei>,
}

#[derive(Serialize, Deserialize)]
struct BildDatei {
    textur: String,
    groesse: [f32; 2],
    #[serde(default = "weiss", skip_serializing_if = "ist_weiss")]
    farbe: [f32; 4],
    #[serde(default = "ganz", skip_serializing_if = "ist_ganz")]
    uv: [f32; 4],
    #[serde(default, skip_serializing_if = "ist_null_i")]
    ebene: i32,
}

#[derive(Serialize, Deserialize)]
struct BewegungDatei {
    #[serde(default)]
    linear: [f32; 2],
    #[serde(default)]
    drehung: f32,
}

fn eins() -> [f32; 2] {
    [1.0, 1.0]
}
fn weiss() -> [f32; 4] {
    [1.0; 4]
}
fn ganz() -> [f32; 4] {
    [0.0, 0.0, 1.0, 1.0]
}
fn ist_null(v: &f32) -> bool {
    *v == 0.0
}
fn ist_null_i(v: &i32) -> bool {
    *v == 0
}
fn ist_eins(v: &[f32; 2]) -> bool {
    *v == eins()
}
fn ist_weiss(v: &[f32; 4]) -> bool {
    *v == weiss()
}
fn ist_ganz(v: &[f32; 4]) -> bool {
    *v == ganz()
}

/// Gerundet auf 3 Nachkommastellen – hält die Datei lesbar (kein 12.000000476837158).
fn r(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}
fn r2(v: Vec2) -> [f32; 2] {
    [r(v.x), r(v.y)]
}

/// Alle Objekte der Welt in eine Szene schreiben.
pub fn speichern(welt: &Welt, assets: &Assets, id: AssetId, name: &str) -> Result<Szene, String> {
    let mut objekte = Vec::new();
    for (t, n, b, bew) in welt
        .query::<(&Transform, Option<&Name>, Option<&Bild>, Option<&Bewegung>)>()
        .iter()
    {
        let bild = match b {
            Some(b) => Some(BildDatei {
                textur: assets
                    .asset_von(b.textur)
                    .ok_or("Objekt benutzt eine Textur, die nicht aus einer .tbr-Datei stammt")?
                    .hex(),
                groesse: r2(b.groesse),
                farbe: b.farbe.map(r),
                uv: b.uv.map(r),
                ebene: b.ebene,
            }),
            None => None,
        };
        objekte.push(ObjektDatei {
            name: n.map(|n| n.0.clone()),
            position: r2(t.position),
            drehung: r(t.drehung.rem_euclid(std::f32::consts::TAU)),
            skala: r2(t.skala),
            bild,
            bewegung: bew.map(|b| BewegungDatei {
                linear: r2(b.linear),
                drehung: r(b.drehung),
            }),
        });
    }
    // Ein Objekt pro Zeile: gut lesbar, und bei Git sieht man genau, welche Objekte sich geändert haben.
    let mut text = format!("{{\n  \"format\": {SZENEN_FORMAT},\n  \"objekte\": [\n");
    for (i, o) in objekte.iter().enumerate() {
        let zeile = serde_json::to_string(o)
            .map_err(|e| format!("Szene kann nicht geschrieben werden: {e}"))?;
        text.push_str("    ");
        text.push_str(&zeile);
        text.push_str(if i + 1 < objekte.len() { ",\n" } else { "\n" });
    }
    text.push_str("  ]\n}\n");
    Ok(Szene {
        id,
        name: name.to_string(),
        text,
    })
}

/// Objekte aus einer Szene in die Welt setzen (vorhandene Objekte bleiben).
/// Liefert die Anzahl neuer Objekte.
pub fn laden(welt: &mut Welt, assets: &Assets, szene: &Szene) -> Result<usize, String> {
    let datei: SzeneDatei = serde_json::from_str(&szene.text)
        .map_err(|e| format!("Szene '{}' ist kein gültiges JSON: {e}", szene.name))?;
    if datei.format > SZENEN_FORMAT {
        return Err(format!(
            "Szene '{}' hat Format {}, unterstützt wird bis {}",
            szene.name, datei.format, SZENEN_FORMAT
        ));
    }

    // Erst alles prüfen, dann einsetzen – eine kaputte Szene soll die Welt nicht halb füllen.
    let mut neu = Vec::with_capacity(datei.objekte.len());
    for (nr, o) in datei.objekte.into_iter().enumerate() {
        let transform = Transform {
            position: o.position.into(),
            drehung: o.drehung,
            skala: o.skala.into(),
        };
        let bild = match o.bild {
            Some(b) => {
                let asset = AssetId::aus_hex(&b.textur).ok_or_else(|| {
                    format!("Objekt {nr}: '{}' ist keine gültige Asset-ID", b.textur)
                })?;
                let textur = assets
                    .textur(asset)
                    .ok_or_else(|| format!("Objekt {nr}: Textur {} ist nicht geladen", b.textur))?;
                Some(Bild {
                    textur,
                    groesse: b.groesse.into(),
                    farbe: b.farbe,
                    uv: b.uv,
                    ebene: b.ebene,
                })
            }
            None => None,
        };
        let bewegung = o.bewegung.map(|b| Bewegung {
            linear: b.linear.into(),
            drehung: b.drehung,
        });
        neu.push((transform, o.name.map(Name), bild, bewegung));
    }

    let anzahl = neu.len();
    for (t, name, bild, bewegung) in neu {
        let mut baukasten = hecs::EntityBuilder::new();
        baukasten.add(t);
        if let Some(n) = name {
            baukasten.add(n);
        }
        if let Some(b) = bild {
            baukasten.add(b);
        }
        if let Some(b) = bewegung {
            baukasten.add(b);
        }
        welt.spawn(baukasten.build());
    }
    Ok(anzahl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_ohne_bild_hin_und_zurueck() {
        let mut welt = Welt::new();
        welt.spawn((
            Transform::bei(Vec2::new(1.5, -2.0)),
            Name("Punkt".into()),
            Bewegung::default(),
        ));
        let assets = Assets::default();
        let szene = speichern(&welt, &assets, AssetId::default(), "Test").unwrap();
        assert!(szene.text.contains("\"Punkt\""));

        let mut neue_welt = Welt::new();
        assert_eq!(laden(&mut neue_welt, &assets, &szene).unwrap(), 1);
        let (t, n) = neue_welt
            .query_mut::<(&Transform, &Name)>()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(t.position, Vec2::new(1.5, -2.0));
        assert_eq!(n.0, "Punkt");
    }

    #[test]
    fn fehlende_textur_fuellt_welt_nicht_halb() {
        let text = r#"{ "format": 0, "objekte": [
            { "position": [0, 0] },
            { "position": [1, 1], "bild": { "textur": "00000000000000000000000000000001", "groesse": [1, 1] } }
        ] }"#;
        let szene = Szene {
            id: AssetId::default(),
            name: "kaputt".into(),
            text: text.into(),
        };
        let mut welt = Welt::new();
        assert!(laden(&mut welt, &Assets::default(), &szene).is_err());
        assert_eq!(welt.len(), 0);
    }
}
