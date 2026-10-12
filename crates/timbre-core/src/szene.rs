//! Szenen speichern und laden.
//!
//! Eine Szene ist eine `.tbr`-Datei vom Typ Szene; ihr Inhalt ist **JSON** – lesbar,
//! und auch Grove (JavaScript) und Bridge (Python) können es ohne Extra-Bibliothek lesen.
//! Texturen werden über ihre Asset-ID referenziert, nie über Dateinamen.
//!
//! Beispiel (ein Objekt pro Zeile):
//! ```json
//! {
//!   "format": 0,
//!   "objekte": [
//!     {"id":1,"name":"Spieler","position":[0,0],"bild":{"textur":"995f…","groesse":[64,64],"ebene":2}},
//!     {"name":"Steuerung","eltern":1,"script":{"datei":"spieler.luau"}},
//!     {"name":"Baumscheibe","position":[120,-40],"drehung":0.5,"bild":{"textur":"a3ad…","groesse":[48,48]}}
//!   ]
//! }
//! ```
//! - `id` steht nur bei Objekten, die Kinder haben; Kinder verweisen per `eltern` darauf.
//! - Objekte ohne `position` haben keinen Platz in der Welt (z. B. Scripts).
//! - Fehlende Felder bekommen Standardwerte (Skala 1, Farbe weiß, ganze Textur, Script aktiv …).

use std::collections::HashMap;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use timbre_tbr::{AssetId, Szene};

use crate::assets::Assets;
use crate::welt::{Bewegung, Bild, Eltern, Form, Kollision, Name, Objekt, Script, Transform, Welt};

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
    id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    eltern: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    position: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "ist_null")]
    drehung: f32,
    #[serde(default = "eins", skip_serializing_if = "ist_eins")]
    skala: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bild: Option<BildDatei>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bewegung: Option<BewegungDatei>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    script: Option<ScriptDatei>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kollision: Option<KollisionDatei>,
}

#[derive(Serialize, Deserialize)]
struct KollisionDatei {
    /// "kreis" oder "rechteck"
    form: String,
    #[serde(default = "wahr", skip_serializing_if = "ist_wahr")]
    fest: bool,
    #[serde(default = "wahr", skip_serializing_if = "ist_wahr")]
    verankert: bool,
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

#[derive(Serialize, Deserialize)]
struct ScriptDatei {
    datei: String,
    #[serde(default = "wahr", skip_serializing_if = "ist_wahr")]
    aktiv: bool,
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
fn wahr() -> bool {
    true
}
fn ist_wahr(v: &bool) -> bool {
    *v
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

/// Alle Objekte der Welt in eine Szene schreiben. Eltern stehen immer vor ihren Kindern.
pub fn speichern(welt: &Welt, assets: &Assets, id: AssetId, name: &str) -> Result<Szene, String> {
    // Reihenfolge: Wurzeln zuerst, dann jeweils deren Kinder (Tiefensuche).
    let mut alle: Vec<Objekt> = welt.iter().map(|e| e.entity()).collect();
    alle.sort_by_key(|o| o.id());
    let eltern_von = |o: Objekt| welt.get::<&Eltern>(o).ok().map(|e| e.0).filter(|e| welt.contains(*e));
    let mut reihenfolge = Vec::with_capacity(alle.len());
    let mut stapel: Vec<Objekt> = alle
        .iter()
        .copied()
        .filter(|o| eltern_von(*o).is_none())
        .rev()
        .collect();
    while let Some(o) = stapel.pop() {
        reihenfolge.push(o);
        let mut k: Vec<Objekt> = alle.iter().copied().filter(|x| eltern_von(*x) == Some(o)).collect();
        k.reverse();
        stapel.extend(k);
    }

    // Nummern nur für Objekte, die Kinder haben
    let mut nummern: HashMap<Objekt, u32> = HashMap::new();
    for &o in &reihenfolge {
        if let Some(e) = eltern_von(o) {
            let n = nummern.len() as u32 + 1;
            nummern.entry(e).or_insert(n);
        }
    }

    let mut objekte = Vec::with_capacity(reihenfolge.len());
    for o in reihenfolge {
        let t = welt.get::<&Transform>(o).ok().map(|t| *t);
        let bild = match welt.get::<&Bild>(o).ok().map(|b| *b) {
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
            id: nummern.get(&o).copied(),
            name: welt.get::<&Name>(o).ok().map(|n| n.0.clone()),
            eltern: eltern_von(o).and_then(|e| nummern.get(&e).copied()),
            position: t.map(|t| r2(t.position)),
            drehung: t.map(|t| r(t.drehung.rem_euclid(std::f32::consts::TAU))).unwrap_or(0.0),
            skala: t.map(|t| r2(t.skala)).unwrap_or(eins()),
            bild,
            bewegung: welt.get::<&Bewegung>(o).ok().map(|b| BewegungDatei {
                linear: r2(b.linear),
                drehung: r(b.drehung),
            }),
            script: welt.get::<&Script>(o).ok().map(|s| ScriptDatei {
                datei: s.datei.clone(),
                aktiv: s.aktiv,
            }),
            kollision: welt.get::<&Kollision>(o).ok().map(|k| KollisionDatei {
                form: match k.form {
                    Form::Kreis => "kreis".into(),
                    Form::Rechteck => "rechteck".into(),
                },
                fest: k.fest,
                verankert: k.verankert,
            }),
        });
    }

    // Ein Objekt pro Zeile: gut lesbar, und bei Git sieht man genau, welche Objekte sich geändert haben.
    let mut text = format!("{{\n  \"format\": {SZENEN_FORMAT},\n  \"objekte\": [\n");
    for (i, o) in objekte.iter().enumerate() {
        let zeile = serde_json::to_string(o).map_err(|e| format!("Szene kann nicht geschrieben werden: {e}"))?;
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
    let datei: SzeneDatei =
        serde_json::from_str(&szene.text).map_err(|e| format!("Szene '{}' ist kein gültiges JSON: {e}", szene.name))?;
    if datei.format > SZENEN_FORMAT {
        return Err(format!(
            "Szene '{}' hat Format {}, unterstützt wird bis {}",
            szene.name, datei.format, SZENEN_FORMAT
        ));
    }

    // Erst alles prüfen, dann einsetzen – eine kaputte Szene soll die Welt nicht halb füllen.
    let ids: Vec<u32> = datei.objekte.iter().filter_map(|o| o.id).collect();
    let mut baukaesten = Vec::with_capacity(datei.objekte.len());
    for (nr, o) in datei.objekte.iter().enumerate() {
        if let Some(e) = o.eltern {
            if !ids.contains(&e) {
                return Err(format!("Objekt {nr}: Eltern-Objekt {e} gibt es nicht"));
            }
        }
        let mut b = hecs::EntityBuilder::new();
        if let Some(p) = o.position {
            b.add(Transform {
                position: p.into(),
                drehung: o.drehung,
                skala: o.skala.into(),
            });
        }
        if let Some(n) = &o.name {
            b.add(Name(n.clone()));
        }
        if let Some(bild) = &o.bild {
            let asset = AssetId::aus_hex(&bild.textur)
                .ok_or_else(|| format!("Objekt {nr}: '{}' ist keine gültige Asset-ID", bild.textur))?;
            let textur = assets
                .textur(asset)
                .ok_or_else(|| format!("Objekt {nr}: Textur {} ist nicht geladen", bild.textur))?;
            b.add(Bild {
                textur,
                groesse: bild.groesse.into(),
                farbe: bild.farbe,
                uv: bild.uv,
                ebene: bild.ebene,
            });
        }
        if let Some(bew) = &o.bewegung {
            b.add(Bewegung {
                linear: bew.linear.into(),
                drehung: bew.drehung,
            });
        }
        if let Some(s) = &o.script {
            b.add(Script {
                datei: s.datei.clone(),
                aktiv: s.aktiv,
            });
        }
        if let Some(k) = &o.kollision {
            let form = match k.form.as_str() {
                "kreis" => Form::Kreis,
                "rechteck" => Form::Rechteck,
                f => {
                    return Err(format!(
                        "Objekt {nr}: unbekannte Kollisionsform '{f}' (kreis oder rechteck)"
                    ))
                }
            };
            b.add(Kollision {
                form,
                fest: k.fest,
                verankert: k.verankert,
            });
        }
        baukaesten.push(b);
    }

    let mut neu: Vec<Objekt> = Vec::with_capacity(baukaesten.len());
    let mut nach_id: HashMap<u32, Objekt> = HashMap::new();
    for (mut b, o) in baukaesten.into_iter().zip(&datei.objekte) {
        let objekt = welt.spawn(b.build());
        if let Some(id) = o.id {
            nach_id.insert(id, objekt);
        }
        neu.push(objekt);
    }
    // Eltern erst jetzt setzen – dann ist die Reihenfolge in der Datei egal.
    for (objekt, o) in neu.iter().zip(&datei.objekte) {
        if let Some(e) = o.eltern.and_then(|e| nach_id.get(&e)) {
            let _ = welt.insert_one(*objekt, Eltern(*e));
        }
    }
    Ok(neu.len())
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
        let (t, n) = neue_welt.query_mut::<(&Transform, &Name)>().into_iter().next().unwrap();
        assert_eq!(t.position, Vec2::new(1.5, -2.0));
        assert_eq!(n.0, "Punkt");
    }

    #[test]
    fn baum_und_scripts_hin_und_zurueck() {
        let mut welt = Welt::new();
        let spieler = welt.spawn((Name("Spieler".into()), Transform::default()));
        welt.spawn((
            Name("Steuerung".into()),
            Eltern(spieler),
            Script {
                datei: "spieler.luau".into(),
                aktiv: false,
            },
        ));
        welt.spawn((
            Name("Laub".into()),
            Script {
                datei: "laub.luau".into(),
                aktiv: true,
            },
        ));
        welt.spawn((
            Name("Wand".into()),
            Transform::default(),
            Kollision {
                form: Form::Kreis,
                fest: false,
                verankert: true,
            },
        ));
        let assets = Assets::default();
        let szene = speichern(&welt, &assets, AssetId::default(), "Test").unwrap();

        let mut neu = Welt::new();
        laden(&mut neu, &assets, &szene).unwrap();
        let mut q = neu.query::<(&Name, &Script, Option<&Eltern>)>();
        let mut scripts: Vec<_> = q
            .iter()
            .map(|(n, s, e)| (n.0.clone(), s.clone(), e.map(|e| e.0)))
            .collect();
        scripts.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(scripts[0].0, "Laub");
        assert!(scripts[0].2.is_none());
        assert_eq!(
            scripts[1].1,
            Script {
                datei: "spieler.luau".into(),
                aktiv: false
            }
        );
        let eltern = scripts[1].2.unwrap();
        assert_eq!(neu.get::<&Name>(eltern).unwrap().0, "Spieler");
        let k = *neu.query::<&Kollision>().iter().next().unwrap();
        assert_eq!(
            k,
            Kollision {
                form: Form::Kreis,
                fest: false,
                verankert: true
            }
        );
        // Script-Objekte haben keine Position
        assert!(!szene
            .text
            .lines()
            .any(|z| z.contains("Steuerung") && z.contains("position")));
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
