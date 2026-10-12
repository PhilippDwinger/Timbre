//! Timbre Studio: der Editor.
//!
//! Zwei Modi wie in Roblox Studio:
//! - **Bearbeiten**: Objekte auswählen, verschieben, Eigenschaften ändern, Scripts schreiben.
//! - **Läuft** (▶ / F5): Scripts laufen. Bei ■ Stopp wird die Szene auf den Stand von vor
//!   dem Start zurückgesetzt – Änderungen während des Laufens gehen also verloren.
//!
//! Scripts sind Objekte in der Szene (Komponente [`Script`]) und hängen im Explorer-Baum,
//! gern auch unter dem Objekt, das sie steuern. Ihr Quelltext liegt als Datei in `scripts/`.
//!
//! Rückgängig/Wiederholen (Strg+Z / Strg+Y) arbeitet mit Schnappschüssen der Szene: Nach jeder
//! abgeschlossenen Änderung (Maus losgelassen, Textfeld verlassen) wird der Stand gemerkt.

use std::collections::HashMap;

use egui::{Color32, Key, Modifiers, PointerButton, RichText, Sense, Stroke};
use timbre_core::tbr::{self, AssetId};
use timbre_core::welt::{self, Bewegung, Bild, Eltern, Name, Objekt, Script, Transform, Welt};
use timbre_core::{
    szene, vec2, Assets, Bereich, FrameStatistik, Input, Kamera2d, KeyCode, Renderer, Sprite, TexturId, Vec2,
};

use crate::luau_farben;
use crate::projekt::{self, ProjektInhalt};

#[cfg(not(target_arch = "wasm32"))]
use timbre_core::scripting::{Meldung, ScriptLaufzeit, ScriptMotor};

/// Texturen sind vorerst eingebaut (später: Asset-Ordner des Projekts).
const TEXTUREN: [&[u8]; 2] = [
    include_bytes!("../../../assets/texturen/jahresringe.tbr"),
    include_bytes!("../../../assets/texturen/blatt.tbr"),
];

/// Der Luau-Interpreter als WebAssembly-Modul (gebaut mit tools/luau-gast/bauen.sh).
#[cfg(not(target_arch = "wasm32"))]
const LUAU_GAST: &[u8] = include_bytes!("../../../assets/scripting/luau.wasm");

const NEUES_SCRIPT: &str = "-- Neues Script\n-- script.parent = das Objekt, unter dem dieses Script hängt (oder nil)\n\nfunction on_start()\n\tprint(\"Hallo aus\", script.name)\nend\n\nfunction on_update(dt)\n\nend\n";
const MAX_KONSOLE: usize = 2000;
/// So viele Schritte kann man rückgängig machen.
const MAX_VERLAUF: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Modus {
    Bearbeiten,
    Laeuft,
}

#[derive(Clone, PartialEq, Eq)]
enum Tab {
    Szene,
    /// Script-Datei
    Script(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Art {
    Info,
    System,
    Fehler,
}

struct Zeile {
    art: Art,
    text: String,
}

/// Quelltext eines Scripts im Speicher.
struct Puffer {
    text: String,
    /// Stand auf der Festplatte (leer = noch nie gespeichert)
    gespeichert: Option<String>,
}

impl Puffer {
    fn geaendert(&self) -> bool {
        self.gespeichert.as_deref() != Some(self.text.as_str())
    }
}

/// Was im Explorer angeklickt wurde – wird nach dem Zeichnen ausgeführt.
enum Aktion {
    Auswaehlen(Objekt),
    Oeffnen(Objekt),
    ScriptHinzufuegen(Option<Objekt>),
    Duplizieren(Objekt),
    Loeschen(Objekt),
    ZiehenBeginnen(Objekt),
    /// Objekt im Explorer unter ein anderes (oder nach oben: `None`) gezogen
    Ablegen(Objekt, Option<Objekt>),
}

/// Das Übersetzen des Luau-Moduls dauert einen Moment – passiert im Hintergrund.
#[cfg(not(target_arch = "wasm32"))]
enum Motor {
    Laedt(std::sync::mpsc::Receiver<Result<ScriptMotor, String>>),
    Bereit(ScriptMotor),
    Kaputt(String),
}

pub struct Studio {
    projekt_name: String,
    #[cfg(not(target_arch = "wasm32"))]
    ordner: std::path::PathBuf,
    welt: Welt,
    assets: Assets,
    szene_id: Option<AssetId>,
    /// Script-Datei → Quelltext
    puffer: HashMap<String, Puffer>,
    /// Offene Script-Reiter (Dateien)
    offen: Vec<String>,
    tab: Tab,
    modus: Modus,
    schnappschuss: Option<tbr::Szene>,
    kamera_vor_start: Kamera2d,
    #[cfg(not(target_arch = "wasm32"))]
    motor: Motor,
    #[cfg(not(target_arch = "wasm32"))]
    laufzeit: Option<ScriptLaufzeit>,
    konsole: Vec<Zeile>,
    kamera: Kamera2d,
    kamera_vorher: Kamera2d,
    auswahl: Option<Objekt>,
    /// Beim Ziehen: Abstand Maus → Objektmitte
    ziehen: Option<Vec2>,
    /// Wo die Szene gezeichnet wird (Pixel); `None`, wenn ein Script-Tab offen ist
    bereich: Option<Bereich>,
    maus_welt: Vec2,
    sprites: Vec<Sprite>,
    statistik: FrameStatistik,
    fps: Fps,
    /// Rückgängig: frühere Stände der Szene (als Szenen-Text)
    zurueck: Vec<String>,
    /// Wiederholen
    vor: Vec<String>,
    /// Zuletzt gemerkter Stand
    stand: Option<String>,
    /// Seit dem letzten gemerkten Stand wurde etwas geändert
    geaendert: bool,
    /// Gerade im Explorer gezogenes Objekt
    explorer_ziehen: Option<Objekt>,
    /// Fehlerzeile pro Script-Datei (wird im Editor rot hinterlegt)
    fehler_zeilen: HashMap<String, usize>,
    /// Im Editor zu dieser Zeile springen (einmalig)
    sprung: Option<(String, usize)>,
}

impl Studio {
    pub fn new(renderer: &mut Renderer) -> Self {
        let mut assets = Assets::default();
        for t in TEXTUREN {
            assets.textur_laden(renderer, t).expect("eingebaute Textur kaputt");
        }

        #[cfg(not(target_arch = "wasm32"))]
        let ordner = projekt::ordner_aus_argumenten();
        #[cfg(not(target_arch = "wasm32"))]
        let geladen = projekt::laden(&ordner);
        #[cfg(target_arch = "wasm32")]
        let geladen: Result<ProjektInhalt, String> = Ok(projekt::eingebaut());

        let mut studio = Self {
            projekt_name: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            ordner,
            welt: Welt::new(),
            assets,
            szene_id: None,
            puffer: HashMap::new(),
            offen: Vec::new(),
            tab: Tab::Szene,
            modus: Modus::Bearbeiten,
            schnappschuss: None,
            kamera_vor_start: Kamera2d::default(),
            #[cfg(not(target_arch = "wasm32"))]
            motor: motor_im_hintergrund(),
            #[cfg(not(target_arch = "wasm32"))]
            laufzeit: None,
            konsole: Vec::new(),
            kamera: Kamera2d::default(),
            kamera_vorher: Kamera2d::default(),
            auswahl: None,
            ziehen: None,
            bereich: None,
            maus_welt: Vec2::ZERO,
            sprites: Vec::new(),
            statistik: FrameStatistik::default(),
            fps: Fps::default(),
            zurueck: Vec::new(),
            vor: Vec::new(),
            stand: None,
            geaendert: false,
            explorer_ziehen: None,
            fehler_zeilen: HashMap::new(),
            sprung: None,
        };

        match geladen {
            Ok(inhalt) => studio.projekt_uebernehmen(inhalt),
            Err(e) => studio.melden(Art::Fehler, e),
        }
        studio.stand = studio.welt_text();
        studio
    }

    fn projekt_uebernehmen(&mut self, inhalt: ProjektInhalt) {
        self.projekt_name = inhalt.name;
        for s in inhalt.scripts {
            self.puffer.insert(
                s.name,
                Puffer {
                    gespeichert: Some(s.text.clone()),
                    text: s.text,
                },
            );
        }
        if let Some(s) = inhalt.szene {
            self.szene_id = Some(s.id);
            if let Err(e) = szene::laden(&mut self.welt, &self.assets, &s) {
                self.melden(Art::Fehler, e);
            }
        }

        // Script-Dateien ohne Script-Objekt (ältere Projekte) als Objekte übernehmen.
        let mut ohne_objekt: Vec<String> = self
            .puffer
            .keys()
            .filter(|d| self.script_objekt(d).is_none())
            .cloned()
            .collect();
        ohne_objekt.sort();
        for datei in &ohne_objekt {
            let name = datei.trim_end_matches(".luau").to_string();
            self.welt.spawn((
                Name(name),
                Script {
                    datei: datei.clone(),
                    aktiv: true,
                },
            ));
        }
        if !ohne_objekt.is_empty() {
            self.melden(
                Art::System,
                format!(
                    "{} Script-Datei(en) als Script-Objekte übernommen – mit Strg+S speichern.",
                    ohne_objekt.len()
                ),
            );
        }
        let anzahl_scripts = self.welt.query::<&Script>().iter().count();
        self.melden(
            Art::System,
            format!(
                "Projekt '{}' geladen: {} Objekte, davon {} Scripts",
                self.projekt_name,
                self.welt.len(),
                anzahl_scripts
            ),
        );
    }

    pub fn titel(&self) -> String {
        format!("Timbre Studio – {}", self.projekt_name)
    }

    fn melden(&mut self, art: Art, text: impl Into<String>) {
        self.konsole.push(Zeile { art, text: text.into() });
        if self.konsole.len() > MAX_KONSOLE {
            let zu_viel = self.konsole.len() - MAX_KONSOLE;
            self.konsole.drain(..zu_viel);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn meldungen_uebernehmen(&mut self) {
        let Some(lz) = self.laufzeit.as_mut() else { return };
        let meldungen = std::mem::take(&mut lz.kontext().meldungen);
        for m in meldungen {
            match m {
                Meldung::Info(t) => self.melden(Art::Info, t),
                Meldung::Fehler(t) => self.melden(Art::Fehler, t),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Rückgängig / Wiederholen
    // -----------------------------------------------------------------------

    fn welt_text(&self) -> Option<String> {
        szene::speichern(&self.welt, &self.assets, AssetId::default(), "")
            .ok()
            .map(|s| s.text)
    }

    /// Etwas an der Szene wurde geändert – wird gemerkt, sobald die Änderung abgeschlossen ist.
    fn markieren(&mut self) {
        if self.modus == Modus::Bearbeiten {
            self.geaendert = true;
        }
    }

    /// Am Ende jedes Frames: abgeschlossene Änderung als Schritt merken.
    fn verlauf_festhalten(&mut self, ctx: &egui::Context) {
        if !self.geaendert || self.modus != Modus::Bearbeiten {
            return;
        }
        // Noch mitten in der Änderung (Maus gedrückt, Tippen in einem Feld)?
        if ctx.input(|i| i.pointer.any_down()) || ctx.egui_wants_keyboard_input() {
            return;
        }
        self.geaendert = false;
        let Some(neu) = self.welt_text() else { return };
        if self.stand.as_ref() == Some(&neu) {
            return;
        }
        if let Some(alt) = self.stand.replace(neu) {
            self.zurueck.push(alt);
            if self.zurueck.len() > MAX_VERLAUF {
                self.zurueck.remove(0);
            }
        }
        self.vor.clear();
    }

    fn rueckgaengig(&mut self) {
        if self.modus != Modus::Bearbeiten {
            return;
        }
        if let Some(alt) = self.zurueck.pop() {
            if let Some(jetzt) = self.stand.replace(alt.clone()) {
                self.vor.push(jetzt);
            }
            self.welt_aus_text(&alt);
        }
    }

    fn wiederholen(&mut self) {
        if self.modus != Modus::Bearbeiten {
            return;
        }
        if let Some(neu) = self.vor.pop() {
            if let Some(jetzt) = self.stand.replace(neu.clone()) {
                self.zurueck.push(jetzt);
            }
            self.welt_aus_text(&neu);
        }
    }

    fn welt_aus_text(&mut self, text: &str) {
        let auswahl_name = self.auswahl.map(|o| self.name(o));
        let szene = tbr::Szene {
            id: AssetId::default(),
            name: String::new(),
            text: text.to_string(),
        };
        let mut welt = Welt::new();
        match szene::laden(&mut welt, &self.assets, &szene) {
            Ok(_) => self.welt = welt,
            Err(e) => self.melden(Art::Fehler, format!("Rückgängig fehlgeschlagen: {e}")),
        }
        self.auswahl = auswahl_name.and_then(|n| {
            self.welt
                .query::<(Objekt, &Name)>()
                .iter()
                .find(|(_, x)| x.0 == n)
                .map(|(o, _)| o)
        });
        self.ziehen = None;
        self.geaendert = false;
    }

    // -----------------------------------------------------------------------
    // Hilfen rund um Objekte und Scripts
    // -----------------------------------------------------------------------

    fn name(&self, o: Objekt) -> String {
        self.welt
            .get::<&Name>(o)
            .map(|n| n.0.clone())
            .unwrap_or_else(|_| format!("Objekt {}", o.id()))
    }

    fn eltern(&self, o: Objekt) -> Option<Objekt> {
        self.welt
            .get::<&Eltern>(o)
            .ok()
            .map(|e| e.0)
            .filter(|e| self.welt.contains(*e))
    }

    /// Script-Objekt, das auf diese Datei verweist.
    fn script_objekt(&self, datei: &str) -> Option<Objekt> {
        self.welt
            .query::<(Objekt, &Script)>()
            .iter()
            .find(|(_, s)| s.datei == datei)
            .map(|(o, _)| o)
    }

    /// Freier Dateiname aus einem Objektnamen: „Mein Script!“ → „Mein_Script.luau“
    fn freie_datei(&self, name: &str, ausser: Option<&str>) -> String {
        let mut basis: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string();
        if basis.is_empty() {
            basis = "Script".into();
        }
        let belegt = |d: &str| Some(d) != ausser && (self.puffer.contains_key(d) || self.script_objekt(d).is_some());
        let mut datei = format!("{basis}.luau");
        let mut n = 2;
        while belegt(&datei) {
            datei = format!("{basis}{n}.luau");
            n += 1;
        }
        datei
    }

    /// Script mit diesem Objektnamen öffnen und zur Zeile springen.
    fn zu_fehler_springen(&mut self, name: &str, zeile: usize) {
        let datei = self
            .welt
            .query::<(&Name, &Script)>()
            .iter()
            .find(|(n, _)| n.0 == name)
            .map(|(_, s)| s.datei.clone());
        match datei {
            Some(d) => {
                self.script_oeffnen(&d);
                self.fehler_zeilen.insert(d.clone(), zeile);
                self.sprung = Some((d, zeile));
            }
            None => self.melden(Art::System, format!("Script '{name}' gibt es nicht mehr.")),
        }
    }

    fn script_oeffnen(&mut self, datei: &str) {
        if !self.offen.iter().any(|d| d == datei) {
            self.offen.push(datei.to_string());
        }
        self.tab = Tab::Script(datei.to_string());
    }

    fn script_anlegen(&mut self, eltern: Option<Objekt>) {
        // Scripts hängen nicht unter Scripts – dann unter dessen Eltern.
        let eltern = eltern.and_then(|e| {
            if self.welt.get::<&Script>(e).is_ok() {
                self.eltern(e)
            } else {
                Some(e)
            }
        });
        self.markieren();
        let datei = self.freie_datei("Script", None);
        self.puffer.insert(
            datei.clone(),
            Puffer {
                text: NEUES_SCRIPT.into(),
                gespeichert: None,
            },
        );
        let o = self.welt.spawn((
            Name(datei.trim_end_matches(".luau").into()),
            Script {
                datei: datei.clone(),
                aktiv: true,
            },
        ));
        if let Some(e) = eltern {
            let _ = self.welt.insert_one(o, Eltern(e));
        }
        self.auswahl = Some(o);
        self.script_oeffnen(&datei);
    }

    /// Script-Objekt umbenennen – die Datei wird beim Speichern mit umbenannt.
    fn script_umbenennen(&mut self, o: Objekt, name: String) {
        self.markieren();
        let _ = self.welt.insert_one(o, Name(name.clone()));
        let Ok(alt) = self.welt.get::<&Script>(o).map(|s| s.datei.clone()) else {
            return;
        };
        let neu = self.freie_datei(&name, Some(&alt));
        if neu == alt {
            return;
        }
        // Alten Puffer behalten – Rückgängig kann wieder auf den alten Namen zeigen.
        let text = self.puffer.get(&alt).map(|p| p.text.clone()).unwrap_or_default();
        self.puffer.insert(
            neu.clone(),
            Puffer {
                text,
                gespeichert: None,
            },
        );
        if let Ok(mut s) = self.welt.get::<&mut Script>(o) {
            s.datei = neu.clone();
        }
        for d in self.offen.iter_mut() {
            if *d == alt {
                *d = neu.clone();
            }
        }
        if self.tab == Tab::Script(alt) {
            self.tab = Tab::Script(neu);
        }
    }

    /// Objekt samt Kindern kopieren (Scripts bekommen eine eigene Datei mit gleichem Text).
    fn duplizieren(&mut self, o: Objekt) {
        self.markieren();
        let alle = welt::mit_nachkommen(&self.welt, o);
        let mut neu_von: HashMap<Objekt, Objekt> = HashMap::new();
        for alt in alle {
            let mut b = hecs::EntityBuilder::new();
            if let Ok(n) = self.welt.get::<&Name>(alt) {
                b.add(Name(n.0.clone()));
            }
            if let Ok(t) = self.welt.get::<&Transform>(alt) {
                let mut t = *t;
                if alt == o {
                    t.position += Vec2::splat(30.0); // Kopie leicht versetzt, damit man sie sieht
                }
                b.add(t);
            }
            if let Ok(x) = self.welt.get::<&Bild>(alt) {
                b.add(*x);
            }
            if let Ok(x) = self.welt.get::<&Bewegung>(alt) {
                b.add(*x);
            }
            if let Ok(s) = self.welt.get::<&Script>(alt).map(|s| (*s).clone()) {
                let datei = self.freie_datei(s.datei.trim_end_matches(".luau"), None);
                let text = self.puffer.get(&s.datei).map(|p| p.text.clone()).unwrap_or_default();
                self.puffer.insert(
                    datei.clone(),
                    Puffer {
                        text,
                        gespeichert: None,
                    },
                );
                b.add(Script { datei, aktiv: s.aktiv });
            }
            let eltern = if alt == o {
                self.eltern(o)
            } else {
                self.eltern(alt).and_then(|e| neu_von.get(&e).copied())
            };
            if let Some(e) = eltern {
                b.add(Eltern(e));
            }
            let kopie = self.welt.spawn(b.build());
            neu_von.insert(alt, kopie);
        }
        self.auswahl = neu_von.get(&o).copied();
    }

    fn loeschen(&mut self, o: Objekt) {
        self.markieren();
        welt::entfernen(&mut self.welt, o);
        if self.auswahl.is_some_and(|a| !self.welt.contains(a)) {
            self.auswahl = None;
        }
    }

    /// Oberstes sichtbares Objekt unter einem Weltpunkt.
    fn treffer(&self, punkt: Vec2) -> Option<Objekt> {
        let mut bester: Option<(i32, Objekt)> = None;
        for (o, b) in self.welt.query::<(Objekt, &Bild)>().iter() {
            let Some(t) = welt::welt_transform(&self.welt, o) else {
                continue;
            };
            let lokal = Vec2::from_angle(-t.drehung).rotate(punkt - t.position);
            let halb = b.groesse * t.skala * 0.5;
            if lokal.x.abs() <= halb.x.abs()
                && lokal.y.abs() <= halb.y.abs()
                && bester.is_none_or(|(e, _)| b.ebene >= e)
            {
                bester = Some((b.ebene, o));
            }
        }
        bester.map(|(_, o)| o)
    }

    fn objekt_anlegen(&mut self, name: &str, textur_name: &str, groesse: f32) {
        self.markieren();
        let Some(&textur) = self.assets.textur_namen().get(textur_name) else {
            return;
        };
        let o = self.welt.spawn((
            Name(name.into()),
            Transform::bei(self.kamera.position),
            Bild::new(textur, Vec2::splat(groesse)),
        ));
        self.auswahl = Some(o);
    }

    fn aktion(&mut self, a: Aktion) {
        match a {
            Aktion::Auswaehlen(o) => self.auswahl = Some(o),
            Aktion::Oeffnen(o) => {
                self.auswahl = Some(o);
                if let Ok(datei) = self.welt.get::<&Script>(o).map(|s| s.datei.clone()) {
                    self.script_oeffnen(&datei);
                } else if let Some(t) = welt::welt_transform(&self.welt, o) {
                    // Doppelklick auf ein sichtbares Objekt: Kamera hinfahren
                    self.tab = Tab::Szene;
                    self.kamera.position = t.position;
                    self.kamera_vorher = self.kamera;
                }
            }
            Aktion::ScriptHinzufuegen(eltern) => self.script_anlegen(eltern),
            Aktion::Duplizieren(o) => self.duplizieren(o),
            Aktion::Loeschen(o) => self.loeschen(o),
            Aktion::ZiehenBeginnen(o) => self.explorer_ziehen = Some(o),
            Aktion::Ablegen(o, ziel) => {
                if ziel != self.eltern(o) {
                    if welt::eltern_setzen(&mut self.welt, o, ziel) {
                        self.markieren();
                        self.auswahl = Some(o);
                    } else {
                        self.melden(
                            Art::System,
                            "Ein Objekt kann nicht unter sich selbst oder seine Kinder.",
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // ▶ / ■
    // -----------------------------------------------------------------------

    fn starten(&mut self) {
        if self.modus == Modus::Laeuft {
            return;
        }
        self.scripts_speichern();
        let id = self.szene_id.unwrap_or_default();
        let schnappschuss = match szene::speichern(&self.welt, &self.assets, id, &self.projekt_name) {
            Ok(s) => s,
            Err(e) => return self.melden(Art::Fehler, format!("Start nicht möglich: {e}")),
        };
        self.schnappschuss = Some(schnappschuss);
        self.kamera_vor_start = self.kamera;
        self.modus = Modus::Laeuft;
        self.tab = Tab::Szene;
        self.ziehen = None;
        self.melden(Art::System, "▶ Gestartet");
        self.fehler_zeilen.clear();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let laufzeit = match self.motor_holen() {
                Ok(m) => m.starten(),
                Err(e) => Err(e),
            };
            match laufzeit {
                Ok(mut lz) => {
                    lz.kontext().texturen = self.assets.textur_namen().clone();
                    // Alle aktiven Script-Objekte, nach Namen sortiert
                    let mut scripts: Vec<(Objekt, String, Script, Option<Objekt>)> = self
                        .welt
                        .query::<(Objekt, &Script)>()
                        .iter()
                        .filter(|(_, s)| s.aktiv)
                        .map(|(o, s)| (o, String::new(), s.clone(), None))
                        .collect();
                    for s in scripts.iter_mut() {
                        s.1 = self.name(s.0);
                        s.3 = self.eltern(s.0);
                    }
                    scripts.sort_by(|a, b| natuerlich(&a.1, &b.1));
                    let mut fehlend = Vec::new();
                    for (o, name, script, eltern) in &scripts {
                        match self.puffer.get(&script.datei) {
                            Some(p) => {
                                lz.script_laden(&mut self.welt, name, &p.text, Some(*o), *eltern);
                            }
                            None => fehlend.push(format!("{name}: Datei scripts/{} fehlt", script.datei)),
                        }
                    }
                    lz.start(&mut self.welt);
                    self.laufzeit = Some(lz);
                    for f in fehlend {
                        self.melden(Art::Fehler, f);
                    }
                    self.meldungen_uebernehmen();
                }
                Err(e) => self.melden(Art::Fehler, format!("Scripting nicht verfügbar: {e}")),
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.melden(Art::System, "Scripts laufen im Browser erst ab Meilenstein 8.");
    }

    fn stoppen(&mut self) {
        if self.modus != Modus::Laeuft {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.laufzeit = None;
        }
        // Auswahl über den Namen wiederfinden (Objekt-Nummern ändern sich beim Zurücksetzen)
        let auswahl_name = self.auswahl.map(|o| self.name(o));
        if let Some(s) = self.schnappschuss.take() {
            let mut welt = Welt::new();
            match szene::laden(&mut welt, &self.assets, &s) {
                Ok(_) => self.welt = welt,
                Err(e) => self.melden(Art::Fehler, format!("Zurücksetzen fehlgeschlagen: {e}")),
            }
        }
        self.auswahl = auswahl_name.and_then(|n| {
            self.welt
                .query::<(Objekt, &Name)>()
                .iter()
                .find(|(_, x)| x.0 == n)
                .map(|(o, _)| o)
        });
        self.kamera = self.kamera_vor_start;
        self.kamera_vorher = self.kamera;
        self.modus = Modus::Bearbeiten;
        self.stand = self.welt_text();
        self.geaendert = false;
        self.melden(Art::System, "■ Gestoppt – Szene zurückgesetzt");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn motor_holen(&mut self) -> Result<&ScriptMotor, String> {
        if let Motor::Laedt(rx) = &self.motor {
            // Notfalls warten, bis das Übersetzen fertig ist (dauert nur beim ersten Mal).
            self.motor = match rx.recv() {
                Ok(Ok(m)) => Motor::Bereit(m),
                Ok(Err(e)) => Motor::Kaputt(e),
                Err(_) => Motor::Kaputt("Übersetzen abgebrochen".into()),
            };
        }
        match &self.motor {
            Motor::Bereit(m) => Ok(m),
            Motor::Kaputt(e) => Err(e.clone()),
            Motor::Laedt(_) => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // Speichern
    // -----------------------------------------------------------------------

    /// Geänderte Scripts auf die Festplatte schreiben.
    fn scripts_speichern(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut dateien: Vec<String> = self.welt.query::<&Script>().iter().map(|s| s.datei.clone()).collect();
            dateien.sort();
            dateien.dedup();
            for d in dateien {
                let Some(p) = self.puffer.get(&d) else { continue };
                if !p.geaendert() {
                    continue;
                }
                let text = p.text.clone();
                match projekt::script_speichern(&self.ordner, &d, &text) {
                    Ok(()) => self.puffer.get_mut(&d).unwrap().gespeichert = Some(text),
                    Err(e) => self.melden(Art::Fehler, format!("Speichern fehlgeschlagen: {e}")),
                }
            }
        }
    }

    fn alles_speichern(&mut self) {
        self.scripts_speichern();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.modus == Modus::Laeuft {
                return self.melden(
                    Art::System,
                    "Scripts gespeichert. Die Szene wird nur im Bearbeiten-Modus gespeichert.",
                );
            }
            // Dateien ohne Script-Objekt (gelöscht oder umbenannt) → Papierkorb
            let mut weg = 0;
            for d in projekt::script_dateien(&self.ordner) {
                if self.script_objekt(&d).is_none() {
                    match projekt::in_papierkorb(&self.ordner, &d) {
                        Ok(()) => {
                            weg += 1;
                            // Falls Rückgängig das Script zurückholt, wird die Datei neu geschrieben.
                            if let Some(p) = self.puffer.get_mut(&d) {
                                p.gespeichert = None;
                            }
                        }
                        Err(e) => self.melden(Art::Fehler, format!("Papierkorb: {e}")),
                    }
                }
            }

            let id = *self.szene_id.get_or_insert_with(AssetId::neu);
            let ergebnis = szene::speichern(&self.welt, &self.assets, id, &self.projekt_name)
                .and_then(|s| projekt::szene_speichern(&self.ordner, &s));
            match ergebnis {
                Ok(()) => {
                    let mut text = format!("Gespeichert ({} Objekte)", self.welt.len());
                    if weg > 0 {
                        text += &format!(" – {weg} alte Script-Datei(en) nach scripts/.papierkorb verschoben");
                    }
                    self.melden(Art::System, text);
                }
                Err(e) => self.melden(Art::Fehler, format!("Speichern fehlgeschlagen: {e}")),
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.melden(Art::System, "Im Browser kann (noch) nicht gespeichert werden.");
    }

    // -----------------------------------------------------------------------
    // Logik-Schritt (60× pro Sekunde)
    // -----------------------------------------------------------------------

    pub fn schritt(&mut self, dt: f32, input: &Input, tastatur_frei: bool) {
        self.kamera_vorher = self.kamera;
        welt::schritt_beginnen(&mut self.welt);

        match self.modus {
            Modus::Bearbeiten => {
                if tastatur_frei {
                    let mut r = Vec2::ZERO;
                    if input.taste(KeyCode::KeyW) {
                        r.y += 1.0;
                    }
                    if input.taste(KeyCode::KeyS) {
                        r.y -= 1.0;
                    }
                    if input.taste(KeyCode::KeyA) {
                        r.x -= 1.0;
                    }
                    if input.taste(KeyCode::KeyD) {
                        r.x += 1.0;
                    }
                    self.kamera.position += r.normalize_or_zero() * 700.0 / self.kamera.zoom * dt;
                    if input.taste_neu(KeyCode::Delete) {
                        if let Some(o) = self.auswahl {
                            self.loeschen(o);
                            self.geaendert = true;
                        }
                    }
                }
            }
            Modus::Laeuft => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(lz) = self.laufzeit.as_mut() {
                    if !lz.abgestuerzt() {
                        let k = lz.kontext();
                        k.input = if tastatur_frei { input.clone() } else { Input::default() };
                        k.maus_welt = self.maus_welt;
                        lz.update(&mut self.welt, dt);
                        if let Some(kamera) = lz.kontext().kamera.take() {
                            self.kamera = kamera;
                        }
                        let abgestuerzt = lz.abgestuerzt();
                        self.meldungen_uebernehmen();
                        if abgestuerzt {
                            self.melden(
                                Art::System,
                                "Scripts angehalten. Mit ■ stoppen, Fehler beheben, erneut ▶.",
                            );
                        }
                    }
                }
                welt::bewegen(&mut self.welt, dt);
            }
        }
    }

    pub fn bereich(&self) -> Option<Bereich> {
        self.bereich
    }

    pub fn sprites_bauen(&mut self, alpha: f32) -> (Kamera2d, &[Sprite]) {
        welt::sprites_sammeln(&self.welt, alpha, &mut self.sprites);
        (self.kamera_vorher.lerp(&self.kamera, alpha), &self.sprites)
    }

    pub fn statistik_setzen(&mut self, s: FrameStatistik) {
        self.statistik = s;
        self.fps.frame();
    }

    // -----------------------------------------------------------------------
    // Oberfläche
    // -----------------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            if self.modus == Modus::Laeuft {
                self.stoppen()
            } else {
                self.starten()
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            self.alles_speichern();
        }
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::D)) {
                if let Some(o) = self.auswahl {
                    self.duplizieren(o);
                }
            }
            // Strg+Umschalt+Z zuerst prüfen, sonst schnappt Strg+Z es weg
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                || ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Y))
            {
                self.wiederholen();
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
                self.rueckgaengig();
            }
        }
        // Gelöschte Objekte (z. B. durch Scripts) nicht mehr auswählen
        if self.auswahl.is_some_and(|o| !self.welt.contains(o)) {
            self.auswahl = None;
        }
        // Reiter von Scripts schließen, die es nicht mehr gibt
        let weg: Vec<String> = self
            .offen
            .iter()
            .filter(|d| self.script_objekt(d).is_none())
            .cloned()
            .collect();
        self.offen.retain(|d| !weg.contains(d));
        if matches!(&self.tab, Tab::Script(d) if weg.contains(d)) {
            self.tab = Tab::Szene;
        }

        egui::Panel::top("leiste").show(ui, |ui| self.leiste_ui(ui));
        egui::Panel::bottom("konsole")
            .resizable(true)
            .default_size(170.0)
            .show(ui, |ui| self.konsole_ui(ui));
        egui::Panel::left("explorer")
            .resizable(true)
            .default_size(230.0)
            .show(ui, |ui| self.explorer_ui(ui));
        egui::Panel::right("eigenschaften")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| self.eigenschaften_ui(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.mitte_ui(ui));

        self.verlauf_festhalten(&ctx);
    }

    fn leiste_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🌲 Timbre Studio").strong());
            ui.label(RichText::new(&self.projekt_name).weak());
            ui.separator();
            match self.modus {
                Modus::Bearbeiten => {
                    let knopf = egui::Button::new(RichText::new("▶ Ausführen").color(Color32::WHITE))
                        .fill(Color32::from_rgb(0x3E, 0x7A, 0x3A));
                    if ui.add(knopf).on_hover_text("Scripts starten (F5)").clicked() {
                        self.starten();
                    }
                }
                Modus::Laeuft => {
                    let knopf = egui::Button::new(RichText::new("■ Stopp").color(Color32::WHITE))
                        .fill(Color32::from_rgb(0xA8, 0x3A, 0x2E));
                    if ui
                        .add(knopf)
                        .on_hover_text("Anhalten und Szene zurücksetzen (F5)")
                        .clicked()
                    {
                        self.stoppen();
                    }
                }
            }
            if ui
                .button("💾 Speichern")
                .on_hover_text("Scripts und Szene speichern (Strg+S)")
                .clicked()
            {
                self.alles_speichern();
            }
            let bearbeiten = self.modus == Modus::Bearbeiten;
            if ui
                .add_enabled(bearbeiten && !self.zurueck.is_empty(), egui::Button::new("↶"))
                .on_hover_text(format!("Rückgängig (Strg+Z) – {} Schritte", self.zurueck.len()))
                .clicked()
            {
                self.rueckgaengig();
            }
            if ui
                .add_enabled(bearbeiten && !self.vor.is_empty(), egui::Button::new("↷"))
                .on_hover_text("Wiederholen (Strg+Y)")
                .clicked()
            {
                self.wiederholen();
            }
            ui.separator();
            let (text, farbe) = match self.modus {
                Modus::Bearbeiten => ("Bearbeiten", ui.visuals().weak_text_color()),
                Modus::Laeuft => ("Läuft", Color32::from_rgb(0x7C, 0xC4, 0x6A)),
            };
            ui.label(RichText::new(text).color(farbe));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{:.0} FPS · {} Sprites · {} Draw Calls",
                        self.fps.wert, self.statistik.sprites, self.statistik.draw_calls
                    ))
                    .weak()
                    .monospace(),
                );
            });
        });
    }

    fn konsole_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Konsole").strong());
            if ui.small_button("Leeren").clicked() {
                self.konsole.clear();
            }
        });
        ui.separator();
        let fehler = Color32::from_rgb(0xE5, 0x6B, 0x5D);
        let system = ui.visuals().weak_text_color();
        let normal = ui.visuals().text_color();
        let mut springen = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for z in &self.konsole {
                    let (vorne, farbe) = match z.art {
                        Art::Info => ("", normal),
                        Art::System => ("· ", system),
                        Art::Fehler => ("✖ ", fehler),
                    };
                    let text = RichText::new(format!("{vorne}{}", z.text)).monospace().color(farbe);
                    // Fehler der Form „Name:Zeile: …“ führen per Klick zur Stelle im Script
                    match (z.art, fehler_ort(&z.text)) {
                        (Art::Fehler, Some(ort)) => {
                            let r = ui
                                .add(egui::Label::new(text.underline()).sense(Sense::click()))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text("Klicken: zur Fehlerzeile springen");
                            if r.clicked() {
                                springen = Some(ort);
                            }
                        }
                        _ => {
                            ui.label(text);
                        }
                    }
                }
            });
        if let Some((name, zeile)) = springen {
            self.zu_fehler_springen(&name, zeile);
        }
    }

    fn explorer_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Explorer").strong());
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("+ Baumscheibe").clicked() {
                self.objekt_anlegen("Baumscheibe", "jahresringe", 120.0);
            }
            if ui.small_button("+ Blatt").clicked() {
                self.objekt_anlegen("Blatt", "blatt", 64.0);
            }
            if ui
                .small_button("+ Script")
                .on_hover_text("Neues Script – unter dem ausgewählten Objekt, falls eins ausgewählt ist")
                .clicked()
            {
                self.script_anlegen(self.auswahl);
            }
        });
        ui.separator();

        // Baum aufbauen: Eltern → Kinder, Kinder natürlich sortiert
        let mut infos: HashMap<Objekt, Knoten> = HashMap::new();
        for (o, n, s, b) in self
            .welt
            .query::<(Objekt, Option<&Name>, Option<&Script>, Option<&Bild>)>()
            .iter()
        {
            let art = if s.is_some() {
                KnotenArt::Script {
                    aktiv: s.is_some_and(|s| s.aktiv),
                }
            } else if b.is_some() {
                KnotenArt::Bild
            } else {
                KnotenArt::Leer
            };
            let name = n.map(|n| n.0.clone()).unwrap_or_else(|| format!("Objekt {}", o.id()));
            infos.insert(
                o,
                Knoten {
                    name,
                    art,
                    eltern: None,
                },
            );
        }
        let objekte: Vec<Objekt> = infos.keys().copied().collect();
        for o in objekte {
            let e = self.eltern(o);
            infos.get_mut(&o).unwrap().eltern = e;
        }
        let mut baum: HashMap<Option<Objekt>, Vec<Objekt>> = HashMap::new();
        for (o, k) in &infos {
            baum.entry(k.eltern).or_default().push(*o);
        }
        for kinder in baum.values_mut() {
            kinder.sort_by(|a, b| {
                let (ka, kb) = (&infos[a], &infos[b]);
                // Scripts zuerst, dann nach Namen
                kb.art
                    .ist_script()
                    .cmp(&ka.art.ist_script())
                    .then(natuerlich(&ka.name, &kb.name))
                    .then(a.id().cmp(&b.id()))
            });
        }

        let mut aktionen = Vec::new();
        let b = Baum {
            kinder: &baum,
            infos: &infos,
            auswahl: self.auswahl,
            bearbeiten: self.modus == Modus::Bearbeiten,
            gezogen: self.explorer_ziehen,
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let leer = Vec::new();
            for o in baum.get(&None).unwrap_or(&leer) {
                knoten_ui(ui, *o, &b, &mut aktionen);
            }
            // Freie Fläche: Rechtsklick → neues Script; Objekt hierher ziehen → oberste Ebene
            let rest = ui.allocate_response(ui.available_size().max(egui::vec2(1.0, 40.0)), Sense::click());
            if let Some(o) = self.explorer_ziehen {
                if rest.contains_pointer() {
                    ui.painter().rect_stroke(
                        rest.rect.shrink(2.0),
                        3.0,
                        Stroke::new(1.0, Color32::from_rgb(0xF2, 0xC1, 0x4E)),
                        egui::StrokeKind::Inside,
                    );
                    if ui.input(|i| i.pointer.any_released()) {
                        aktionen.push(Aktion::Ablegen(o, None));
                    }
                }
            }
            rest.context_menu(|ui| {
                if ui.button("📜 Neues Script").clicked() {
                    aktionen.push(Aktion::ScriptHinzufuegen(None));
                }
            });
        });
        // Gezogenes Objekt neben der Maus anzeigen; beim Loslassen ist das Ziehen vorbei
        if let Some(o) = self.explorer_ziehen {
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("explorer_ziehen"))
                    .fixed_pos(p + egui::vec2(14.0, 4.0))
                    .order(egui::Order::Tooltip)
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(format!("📦 {}", self.name(o)));
                        });
                    });
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        let losgelassen = ui.input(|i| i.pointer.any_released());
        for a in aktionen {
            self.aktion(a);
        }
        if losgelassen {
            self.explorer_ziehen = None;
        }
    }

    fn eigenschaften_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Eigenschaften").strong());
        ui.separator();
        let Some(o) = self.auswahl else {
            ui.label(RichText::new("Klicke ein Objekt in der Szene oder im Explorer an.").weak());
            return;
        };
        if self.modus == Modus::Laeuft {
            ui.label(
                RichText::new("Läuft: Änderungen gehen bei ■ verloren.")
                    .weak()
                    .italics(),
            );
        }
        let script: Option<Script> = self.welt.get::<&Script>(o).ok().map(|s| (*s).clone());

        egui::Grid::new("allgemein").num_columns(2).show(ui, |ui| {
            // Name
            ui.label("Name");
            let mut name = self.name(o);
            if ui.text_edit_singleline(&mut name).changed() {
                if script.is_some() {
                    self.script_umbenennen(o, name);
                } else {
                    let _ = self.welt.insert_one(o, Name(name));
                    self.markieren();
                }
            }
            ui.end_row();

            // Eltern
            ui.label("Eltern");
            let jetzt = self.eltern(o);
            let mut wahl = jetzt;
            let text = jetzt.map(|e| self.name(e)).unwrap_or_else(|| "(keins)".into());
            egui::ComboBox::from_id_salt("eltern")
                .selected_text(text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut wahl, None, "(keins)");
                    let mut kandidaten: Vec<(Objekt, String)> = self
                        .welt
                        .query::<(Objekt, Option<&Name>)>()
                        .without::<&Script>()
                        .iter()
                        .filter(|(x, _)| !welt::ist_nachkomme(&self.welt, *x, o))
                        .map(|(x, n)| {
                            (
                                x,
                                n.map(|n| n.0.clone()).unwrap_or_else(|| format!("Objekt {}", x.id())),
                            )
                        })
                        .collect();
                    kandidaten.sort_by(|a, b| natuerlich(&a.1, &b.1));
                    for (x, n) in kandidaten {
                        ui.selectable_value(&mut wahl, Some(x), n);
                    }
                });
            if wahl != jetzt && welt::eltern_setzen(&mut self.welt, o, wahl) {
                self.markieren();
            }
            ui.end_row();
        });

        // Script
        if let Some(s) = script {
            ui.add_space(6.0);
            ui.label(RichText::new("📜 Script").strong());
            egui::Grid::new("script").num_columns(2).show(ui, |ui| {
                ui.label("Sprache");
                ui.label("Luau");
                ui.end_row();
                ui.label("Datei");
                ui.label(RichText::new(format!("scripts/{}", s.datei)).monospace());
                ui.end_row();
                ui.label("Aktiviert");
                let mut aktiv = s.aktiv;
                if ui.checkbox(&mut aktiv, "läuft bei ▶ mit").changed() {
                    if let Ok(mut x) = self.welt.get::<&mut Script>(o) {
                        x.aktiv = aktiv;
                    }
                    self.markieren();
                }
                ui.end_row();
                ui.label("Zeilen");
                ui.label(
                    self.puffer
                        .get(&s.datei)
                        .map(|p| p.text.lines().count())
                        .unwrap_or(0)
                        .to_string(),
                );
                ui.end_row();
            });
            if ui
                .button("Öffnen")
                .on_hover_text("Auch per Doppelklick im Explorer")
                .clicked()
            {
                self.script_oeffnen(&s.datei);
            }
        }

        // Transform
        if let Ok(mut t) = self.welt.get::<&Transform>(o).map(|t| *t) {
            ui.add_space(6.0);
            ui.label(RichText::new("Transform").strong());
            if self
                .eltern(o)
                .and_then(|e| welt::welt_transform(&self.welt, e))
                .is_some()
            {
                ui.label(RichText::new("relativ zum Eltern-Objekt").weak().small());
            }
            let mut grad = t.drehung.to_degrees();
            let vorher = (t, grad);
            egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
                ui.label("Position");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut t.position.x).speed(1.0).prefix("x "));
                    ui.add(egui::DragValue::new(&mut t.position.y).speed(1.0).prefix("y "));
                });
                ui.end_row();
                ui.label("Drehung");
                ui.add(egui::DragValue::new(&mut grad).speed(1.0).suffix("°"));
                ui.end_row();
                ui.label("Skala");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut t.skala.x).speed(0.01).prefix("x "));
                    ui.add(egui::DragValue::new(&mut t.skala.y).speed(0.01).prefix("y "));
                });
                ui.end_row();
            });
            if (t, grad) != vorher {
                t.drehung = grad.to_radians();
                let _ = self.welt.insert_one(o, t);
                self.markieren();
            }
        }

        // Bild
        if let Ok(mut b) = self.welt.get::<&Bild>(o).map(|b| *b) {
            ui.add_space(6.0);
            ui.label(RichText::new("Bild").strong());
            let vorher = b;
            let mut namen: Vec<(String, TexturId)> = self
                .assets
                .textur_namen()
                .iter()
                .map(|(n, t)| (n.clone(), *t))
                .collect();
            namen.sort();
            egui::Grid::new("bild").num_columns(2).show(ui, |ui| {
                ui.label("Textur");
                egui::ComboBox::from_id_salt("textur")
                    .selected_text(self.assets.name_von(b.textur).unwrap_or("?"))
                    .show_ui(ui, |ui| {
                        for (n, t) in &namen {
                            ui.selectable_value(&mut b.textur, *t, n);
                        }
                    });
                ui.end_row();
                ui.label("Größe");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut b.groesse.x).speed(1.0).prefix("b "));
                    ui.add(egui::DragValue::new(&mut b.groesse.y).speed(1.0).prefix("h "));
                });
                ui.end_row();
                ui.label("Farbe");
                ui.color_edit_button_rgba_unmultiplied(&mut b.farbe);
                ui.end_row();
                ui.label("Ebene");
                ui.add(egui::DragValue::new(&mut b.ebene).speed(0.1));
                ui.end_row();
            });
            if b != vorher {
                let _ = self.welt.insert_one(o, b);
                self.markieren();
            }
        }

        // Bewegung (nur für Objekte mit Platz in der Welt)
        if self.welt.get::<&Transform>(o).is_ok() {
            ui.add_space(6.0);
            match self.welt.get::<&Bewegung>(o).map(|b| *b) {
                Ok(mut bew) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Bewegung").strong());
                        if ui.small_button("entfernen").clicked() {
                            let _ = self.welt.remove_one::<Bewegung>(o);
                            self.markieren();
                        }
                    });
                    let vorher = bew;
                    egui::Grid::new("bewegung").num_columns(2).show(ui, |ui| {
                        ui.label("Tempo");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut bew.linear.x).speed(1.0).prefix("x "));
                            ui.add(egui::DragValue::new(&mut bew.linear.y).speed(1.0).prefix("y "));
                        });
                        ui.end_row();
                        ui.label("Drehen");
                        ui.add(egui::DragValue::new(&mut bew.drehung).speed(0.05).suffix(" rad/s"));
                        ui.end_row();
                    });
                    if bew != vorher && self.welt.get::<&Bewegung>(o).is_ok() {
                        let _ = self.welt.insert_one(o, bew);
                        self.markieren();
                    }
                }
                Err(_) => {
                    if ui
                        .small_button("+ Bewegung")
                        .on_hover_text("Bewegt sich beim Ausführen von selbst")
                        .clicked()
                    {
                        let _ = self.welt.insert_one(o, Bewegung::default());
                        self.markieren();
                    }
                }
            }
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("📋 Duplizieren").on_hover_text("Strg+D").clicked() {
                self.duplizieren(o);
            }
            let kinder = welt::mit_nachkommen(&self.welt, o).len() - 1;
            let tipp = if kinder > 0 {
                format!("Entf – löscht auch {kinder} Kind-Objekt(e)")
            } else {
                "Entf".into()
            };
            if ui.button("🗑 Löschen").on_hover_text(tipp).clicked() {
                self.loeschen(o);
            }
        });
    }

    fn mitte_ui(&mut self, ui: &mut egui::Ui) {
        // Reiter: Szene + offene Scripts
        let mut schliessen = None;
        egui::Frame::new()
            .fill(ui.visuals().panel_fill)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if ui.selectable_label(self.tab == Tab::Szene, "🎬 Szene").clicked() {
                        self.tab = Tab::Szene;
                    }
                    for d in &self.offen {
                        let name = self.script_objekt(d).map(|o| self.name(o)).unwrap_or_else(|| d.clone());
                        let geaendert = self.puffer.get(d).is_some_and(|p| p.geaendert());
                        let titel = if geaendert {
                            format!("📜 {name} •")
                        } else {
                            format!("📜 {name}")
                        };
                        let gewaehlt = self.tab == Tab::Script(d.clone());
                        if ui
                            .selectable_label(gewaehlt, titel)
                            .on_hover_text(format!("scripts/{d}"))
                            .clicked()
                        {
                            self.tab = Tab::Script(d.clone());
                        }
                        if ui.small_button("×").on_hover_text("Reiter schließen").clicked() {
                            schliessen = Some(d.clone());
                        }
                        ui.add_space(6.0);
                    }
                });
            });
        if let Some(d) = schliessen {
            self.offen.retain(|x| *x != d);
            if self.tab == Tab::Script(d) {
                self.tab = Tab::Szene;
            }
        }

        match self.tab.clone() {
            Tab::Szene => self.szene_ui(ui),
            Tab::Script(d) => {
                self.bereich = None;
                self.editor_ui(ui, &d);
            }
        }
    }

    fn editor_ui(&mut self, ui: &mut egui::Ui, datei: &str) {
        let Some(puffer) = self.puffer.get_mut(datei) else {
            ui.label(RichText::new(format!("scripts/{datei} fehlt.")).color(Color32::from_rgb(0xE5, 0x6B, 0x5D)));
            return;
        };
        let id = egui::Id::new(("editor", datei));
        let schrift = egui::FontId::monospace(luau_farben::SCHRIFTGROESSE);
        let zeilenhoehe = ui
            .painter()
            .layout_no_wrap("0".into(), schrift.clone(), Color32::WHITE)
            .size()
            .y;
        let fehler_zeile = self.fehler_zeilen.get(datei).copied();

        // Zur Fehlerzeile springen: Cursor setzen, Fokus geben, hinscrollen
        let mut scroll = egui::ScrollArea::both()
            .id_salt(("editor_scroll", datei))
            .auto_shrink([false, false]);
        if let Some((d, zeile)) = self.sprung.clone() {
            if d == datei {
                let zeichen: usize = puffer
                    .text
                    .split('\n')
                    .take(zeile.saturating_sub(1))
                    .map(|z| z.chars().count() + 1)
                    .sum();
                let mut zustand = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
                zustand
                    .cursor
                    .set_char_range(Some(egui::text_selection::CCursorRange::one(egui::text::CCursor::new(
                        zeichen,
                    ))));
                zustand.store(ui.ctx(), id);
                ui.ctx().memory_mut(|m| m.request_focus(id));
                scroll = scroll.vertical_scroll_offset(((zeile as f32 - 6.0) * zeilenhoehe).max(0.0));
                self.sprung = None;
            }
        }

        let mut geaendert = false;
        egui::Frame::new().fill(ui.visuals().extreme_bg_color).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            scroll.show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Zeilennummern
                    let anzahl = puffer.text.split('\n').count();
                    let nummern: String = (1..=anzahl).map(|n| format!("{n:>4} \n")).collect();
                    ui.add(
                        egui::Label::new(
                            RichText::new(nummern.trim_end())
                                .font(schrift.clone())
                                .color(ui.visuals().weak_text_color()),
                        )
                        .selectable(false),
                    );
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, breite: f32| {
                        luau_farben::layout(ui, text.as_str(), breite, fehler_zeile)
                    };
                    let r = ui.add(
                        egui::TextEdit::multiline(&mut puffer.text)
                            .id(id)
                            .code_editor()
                            .margin(egui::Margin::ZERO)
                            .desired_width(f32::INFINITY)
                            .desired_rows(40)
                            .frame(egui::Frame::NONE)
                            .layouter(&mut layouter),
                    );
                    geaendert = r.changed();
                });
            });
        });
        if geaendert {
            self.fehler_zeilen.remove(datei);
        }
    }

    fn szene_ui(&mut self, ui: &mut egui::Ui) {
        let (rect, antwort) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        self.bereich = Some(Bereich {
            x: rect.min.x * ppp,
            y: rect.min.y * ppp,
            breite: rect.width() * ppp,
            hoehe: rect.height() * ppp,
        });
        let groesse = vec2(rect.width() * ppp, rect.height() * ppp);
        let zu_pixel = |p: egui::Pos2| vec2((p.x - rect.min.x) * ppp, (p.y - rect.min.y) * ppp);

        if let Some(p) = ui.input(|i| i.pointer.latest_pos()) {
            self.maus_welt = self.kamera.bildschirm_zu_welt(zu_pixel(p), groesse);
        }

        // Zoomen mit dem Mausrad (Punkt unter der Maus bleibt stehen)
        if antwort.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let vorher = self.maus_welt;
                self.kamera.zoom = (self.kamera.zoom * 1.0015f32.powf(scroll)).clamp(0.05, 20.0);
                if let Some(p) = antwort.hover_pos() {
                    let nachher = self.kamera.bildschirm_zu_welt(zu_pixel(p), groesse);
                    self.kamera.position += vorher - nachher;
                }
                self.kamera_vorher = self.kamera;
            }
        }

        // Schieben mit rechter oder mittlerer Maustaste
        if antwort.dragged_by(PointerButton::Secondary) || antwort.dragged_by(PointerButton::Middle) {
            let d = antwort.drag_delta() * ppp;
            self.kamera.position += vec2(-d.x, d.y) / self.kamera.zoom;
            self.kamera_vorher = self.kamera;
        }

        // Auswählen und Verschieben mit der linken Maustaste (nur im Bearbeiten-Modus)
        if self.modus == Modus::Bearbeiten {
            let (gedrueckt, unten) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down()));
            if gedrueckt && antwort.hovered() {
                self.auswahl = self.treffer(self.maus_welt);
                self.ziehen = self
                    .auswahl
                    .and_then(|o| welt::welt_transform(&self.welt, o).map(|t| t.position - self.maus_welt));
            }
            if !unten {
                self.ziehen = None;
            }
            if let (Some(o), Some(versatz)) = (self.auswahl, self.ziehen) {
                if let Some(mut t) = welt::welt_transform(&self.welt, o) {
                    let ziel = self.maus_welt + versatz;
                    if (t.position - ziel).length_squared() > 1e-6 {
                        t.position = ziel;
                        welt::welt_transform_setzen(&mut self.welt, o, t);
                        self.markieren();
                    }
                }
            }
        }

        // Rahmen um das ausgewählte Objekt
        if let Some(o) = self.auswahl {
            if let (Some(t), Ok(b)) = (
                welt::welt_transform(&self.welt, o),
                self.welt.get::<&Bild>(o).map(|b| *b),
            ) {
                let mitte = self.kamera.welt_zu_bildschirm(t.position, groesse) / ppp;
                let halb = (b.groesse * t.skala).abs() * 0.5 * self.kamera.zoom / ppp;
                let r = egui::Rect::from_center_size(
                    egui::pos2(rect.min.x + mitte.x, rect.min.y + mitte.y),
                    egui::vec2(halb.x * 2.0, halb.y * 2.0),
                );
                ui.painter().with_clip_rect(rect).rect_stroke(
                    r.expand(3.0),
                    2.0,
                    Stroke::new(1.5, Color32::from_rgb(0xF2, 0xC1, 0x4E)),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Hinweis unten links
        let hinweis = match self.modus {
            Modus::Bearbeiten => {
                "Links: auswählen/verschieben · Rechts: Ansicht schieben · Rad: zoomen · WASD: Ansicht"
            }
            Modus::Laeuft => "Läuft – F5 oder ■ Stopp beendet und setzt die Szene zurück",
        };
        ui.painter().text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            hinweis,
            egui::FontId::proportional(12.0),
            Color32::from_white_alpha(110),
        );
    }
}

// ---------------------------------------------------------------------------
// Explorer-Baum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum KnotenArt {
    Script { aktiv: bool },
    Bild,
    Leer,
}

impl KnotenArt {
    fn ist_script(&self) -> bool {
        matches!(self, KnotenArt::Script { .. })
    }
}

struct Knoten {
    name: String,
    art: KnotenArt,
    eltern: Option<Objekt>,
}

/// Eine Zeile im Explorer – mit Kindern als aufklappbarer Ast.
/// Was jede Zeile des Explorer-Baums wissen muss.
struct Baum<'a> {
    kinder: &'a HashMap<Option<Objekt>, Vec<Objekt>>,
    infos: &'a HashMap<Objekt, Knoten>,
    auswahl: Option<Objekt>,
    bearbeiten: bool,
    gezogen: Option<Objekt>,
}

fn knoten_ui(ui: &mut egui::Ui, o: Objekt, b: &Baum, aktionen: &mut Vec<Aktion>) {
    let Baum {
        kinder: baum,
        infos,
        auswahl,
        bearbeiten,
        gezogen,
    } = *b;
    let k = &infos[&o];
    let symbol = match k.art {
        KnotenArt::Script { .. } => "📜",
        KnotenArt::Bild => "🖼",
        KnotenArt::Leer => "📦",
    };
    let mut text = RichText::new(format!("{symbol} {}", k.name));
    if k.art == (KnotenArt::Script { aktiv: false }) {
        text = text.weak().italics();
    }

    let zeile = |ui: &mut egui::Ui, aktionen: &mut Vec<Aktion>| {
        let r = ui
            .selectable_label(auswahl == Some(o), text.clone())
            .interact(Sense::drag());
        if r.drag_started() && bearbeiten {
            aktionen.push(Aktion::ZiehenBeginnen(o));
        }
        // Ein anderes Objekt wird über diese Zeile gezogen → Ablegeziel
        if let Some(g) = gezogen {
            if g != o && r.contains_pointer() {
                ui.painter().rect_stroke(
                    r.rect.expand(1.0),
                    3.0,
                    Stroke::new(1.5, Color32::from_rgb(0xF2, 0xC1, 0x4E)),
                    egui::StrokeKind::Outside,
                );
                if ui.input(|i| i.pointer.any_released()) {
                    aktionen.push(Aktion::Ablegen(g, Some(o)));
                }
            }
        }
        if r.double_clicked() {
            aktionen.push(Aktion::Oeffnen(o));
        } else if r.clicked() {
            aktionen.push(Aktion::Auswaehlen(o));
        }
        r.context_menu(|ui| {
            if k.art.ist_script() {
                if ui.button("Öffnen").clicked() {
                    aktionen.push(Aktion::Oeffnen(o));
                }
            } else if ui.button("📜 Script hinzufügen").clicked() {
                aktionen.push(Aktion::ScriptHinzufuegen(Some(o)));
            }
            if ui.button("📋 Duplizieren").clicked() {
                aktionen.push(Aktion::Duplizieren(o));
            }
            if ui.button("🗑 Löschen").clicked() {
                aktionen.push(Aktion::Loeschen(o));
            }
        });
    };

    match baum.get(&Some(o)) {
        Some(kinder) if !kinder.is_empty() => {
            let id = ui.make_persistent_id(("explorer", o.to_bits().get()));
            // Große Ordner und Ordner, die erst beim Laufen entstehen (z. B. hunderte Blätter), starten zugeklappt
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                bearbeiten && kinder.len() <= 20,
            )
            .show_header(ui, |ui| zeile(ui, aktionen))
            .body(|ui| {
                for kind in kinder {
                    knoten_ui(ui, *kind, b, aktionen);
                }
            });
        }
        _ => {
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().indent);
                zeile(ui, aktionen);
            });
        }
    }
}

/// „Steuerung:12: attempt to …“ → ("Steuerung", 12)
fn fehler_ort(text: &str) -> Option<(String, usize)> {
    let (name, rest) = text.split_once(':')?;
    let zahl: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if zahl.is_empty() || !rest[zahl.len()..].starts_with(':') || name.is_empty() {
        return None;
    }
    Some((name.to_string(), zahl.parse().ok()?))
}

#[cfg(not(target_arch = "wasm32"))]
fn motor_im_hintergrund() -> Motor {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(ScriptMotor::neu(LUAU_GAST));
    });
    Motor::Laedt(rx)
}

/// Vergleich, bei dem Zahlen als Zahlen zählen: „Baumstumpf 2“ vor „Baumstumpf 10“.
fn natuerlich(a: &str, b: &str) -> std::cmp::Ordering {
    let teile = |s: &str| -> Vec<(String, u64)> {
        let mut v = Vec::new();
        let mut text = String::new();
        let mut zahl = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() {
                zahl.push(c);
            } else {
                if !zahl.is_empty() {
                    v.push((std::mem::take(&mut text), zahl.parse().unwrap_or(0)));
                    zahl.clear();
                }
                text.extend(c.to_lowercase());
            }
        }
        v.push((text, zahl.parse().unwrap_or(0)));
        v
    };
    teile(a).cmp(&teile(b))
}

/// Bilder pro Sekunde, einmal pro Sekunde aktualisiert.
struct Fps {
    wert: f32,
    frames: u32,
    seit: web_time::Instant,
}

impl Default for Fps {
    fn default() -> Self {
        Self {
            wert: 0.0,
            frames: 0,
            seit: web_time::Instant::now(),
        }
    }
}

impl Fps {
    fn frame(&mut self) {
        self.frames += 1;
        let t = self.seit.elapsed().as_secs_f32();
        if t >= 1.0 {
            self.wert = self.frames as f32 / t;
            self.frames = 0;
            self.seit = web_time::Instant::now();
        }
    }
}
