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
use timbre_core::glam;
use timbre_core::tbr::{self, AssetId};
use timbre_core::welt::{
    self, Bewegung, Bild, Eltern, Ereignisse, Form, HitBoxGroesse, Instanz, Kollision, Live, Masse, Mesh3d, Name,
    Objekt, Raum, Script, ScriptVerweise, Transform, Welt,
};
use timbre_core::{
    szene, vec2, Assets, Bereich, FrameStatistik, Input, Kamera2d, KeyCode, Renderer, Sprite, TexturId, Vec2,
};
use timbre_core::{Form3d, Kamera3d, Modell};

use crate::eis;

mod ansicht3d;
use crate::luau_farben;
use crate::marke::{self, Marke};
use crate::projekt::{self, ProjektInhalt, Sprache};
use crate::script_quelle;
use crate::vervollstaendigung;

#[cfg(not(target_arch = "wasm32"))]
use timbre_core::audio::Audio;
#[cfg(not(target_arch = "wasm32"))]
use timbre_core::scripting::{BildschirmText, Meldung, ScriptLaufzeit, ScriptMotor, SoundBefehl};

/// Eingebaute Texturen – dazu kommen die des Projekts (`texturen/*.tbr`).
const TEXTUREN: [&[u8]; 4] = [
    include_bytes!("../../../assets/texturen/jahresringe.tbr"),
    include_bytes!("../../../assets/texturen/blatt.tbr"),
    include_bytes!("../../../assets/texturen/eisscholle.tbr"),
    include_bytes!("../../../assets/texturen/schneeflocke.tbr"),
];

/// Die Script-Sprachen als WebAssembly-Module (gebaut mit tools/luau-gast/bauen.sh bzw. tools/lua-gast/bauen.sh).
#[cfg(not(target_arch = "wasm32"))]
const LUAU_GAST: &[u8] = include_bytes!("../../../assets/scripting/luau.wasm");
#[cfg(not(target_arch = "wasm32"))]
const LUA_GAST: &[u8] = include_bytes!("../../../assets/scripting/lua.wasm");
/// Python: CPython 3.12 (tools/python-gast/bauen.sh) und seine Standardbibliothek.
#[cfg(not(target_arch = "wasm32"))]
const PYTHON_GAST: &[u8] = include_bytes!("../../../assets/scripting/python.wasm");
#[cfg(not(target_arch = "wasm32"))]
const PYTHON_BIBLIOTHEK: &[u8] = include_bytes!("../../../assets/scripting/python312.zip");

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
    ScriptHinzufuegen(Option<Objekt>, Sprache),
    Duplizieren(Objekt),
    Loeschen(Objekt),
    ZiehenBeginnen(Objekt),
    /// Objekt im Explorer unter ein anderes (oder nach oben: `None`) gezogen
    Ablegen(Objekt, Option<Objekt>),
}

/// Griff am ausgewählten Objekt, der gerade gezogen wird.
#[derive(Clone, Copy)]
enum GriffArt {
    Skalieren,
    Drehen,
}

#[derive(Clone, Copy)]
struct Griff {
    art: GriffArt,
    start_maus: Vec2,
    /// Lage in der Welt beim Anfassen
    start: Transform,
}

/// Das Übersetzen eines Sprach-Moduls dauert einen Moment – passiert im Hintergrund.
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
    /// Ein übersetztes Gast-Modul pro Sprache
    #[cfg(not(target_arch = "wasm32"))]
    motoren: HashMap<Sprache, Motor>,
    /// Laufende Scripts: eine Laufzeit pro Sprache, die in der Szene vorkommt
    #[cfg(not(target_arch = "wasm32"))]
    laufzeiten: Vec<(Sprache, ScriptLaufzeit)>,
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
    /// Gitter anzeigen und beim Verschieben/Drehen/Skalieren einrasten
    gitter: bool,
    gitter_groesse: f32,
    griff: Option<Griff>,
    /// Maus ist über dem Spielbereich (sonst bekommen Scripts keine Mausklicks)
    maus_ueber_szene: bool,
    /// Texte, die Scripts auf den Bildschirm schreiben (`timbre.ui_text`)
    #[cfg(not(target_arch = "wasm32"))]
    texte: std::collections::BTreeMap<String, BildschirmText>,
    /// Ins Fenster gezogene Dateien, die noch importiert werden müssen (braucht den Renderer)
    importe: Vec<std::path::PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    audio: Audio,
    /// Wann zuletzt nachgesehen wurde, ob Scripts außerhalb (z. B. in VS Code) geändert wurden
    platte_geprueft: web_time::Instant,
    /// Script-Datei → Inhalt auf der Platte, für den schon ein Konflikt gemeldet wurde
    konflikte: HashMap<String, String>,
    /// Autovervollständigung im Script-Editor (offen, wenn `Some`)
    vorschlag: Option<vervollstaendigung::Offen>,
    /// Logo & Startbild (beim ersten Frame geladen)
    marke: Option<Marke>,
    /// Seit wann das Studio läuft (für die Startanimation)
    gestartet_um: web_time::Instant,
    startanimation: bool,
    /// Szene in 3D statt 2D anzeigen und bearbeiten
    ansicht3d: bool,
    kamera3d: Kamera3d,
    modelle: Vec<Modell>,
    /// Beim Ziehen in 3D: Objekt, Abstand Treffpunkt → Objekt, Höhe der Zieh-Ebene
    ziehen3d: Option<(Objekt, glam::Vec3, f32)>,
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

        #[cfg(not(target_arch = "wasm32"))]
        let (audio, audio_fehler) = Audio::neu();

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
            // Luau und Lua sind klein – gleich übersetzen. Python (groß) erst, wenn es gebraucht wird.
            motoren: [Sprache::Luau, Sprache::Lua]
                .iter()
                .map(|&s| (s, motor_im_hintergrund(s)))
                .collect(),
            #[cfg(not(target_arch = "wasm32"))]
            laufzeiten: Vec::new(),
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
            gitter: false,
            gitter_groesse: 32.0,
            ansicht3d: false,
            kamera3d: Kamera3d::default(),
            modelle: Vec::new(),
            ziehen3d: None,
            griff: None,
            maus_ueber_szene: false,
            #[cfg(not(target_arch = "wasm32"))]
            texte: Default::default(),
            importe: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            audio,
            platte_geprueft: web_time::Instant::now(),
            konflikte: HashMap::new(),
            vorschlag: None,
            marke: None,
            gestartet_um: web_time::Instant::now(),
            startanimation: true,
        };
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(e) = audio_fehler {
            studio.melden(Art::System, e);
        }

        match geladen {
            Ok(inhalt) => studio.projekt_uebernehmen(renderer, inhalt),
            Err(e) => studio.melden(Art::Fehler, e),
        }
        studio.stand = studio.welt_text();
        studio
    }

    fn projekt_uebernehmen(&mut self, renderer: &mut Renderer, inhalt: ProjektInhalt) {
        self.projekt_name = inhalt.name;
        // Texturen zuerst – die Szene verweist darauf
        for (datei, daten) in &inhalt.texturen {
            if let Err(e) = self.assets.textur_laden(renderer, daten) {
                self.melden(Art::Fehler, format!("texturen/{datei}: {e}"));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        for (name, daten) in inhalt.sounds {
            if let Err(e) = self.audio.sound_setzen(&name, daten) {
                self.melden(Art::Fehler, e);
            }
        }
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
            let name = projekt::ohne_script_endung(datei).to_string();
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
        // Sprachen, die im Projekt vorkommen, schon im Hintergrund vorbereiten (Python ist groß)
        let sprachen: Vec<Sprache> = self
            .welt
            .query::<&Script>()
            .iter()
            .filter_map(|s| Sprache::aus_datei(&s.datei))
            .collect();
        for sp in sprachen {
            self.motor_vorbereiten(sp);
        }
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

    /// Quelltext eines Script-Objekts so, wie er in der Laufzeit läuft (ohne `live`, mit eigenen
    /// Ereignis-Namen). `None`, wenn die Datei fehlt.
    fn quelle_fuer(&self, o: Objekt) -> Option<String> {
        let s = self.welt.get::<&Script>(o).ok()?.datei.clone();
        let text = &self.puffer.get(&s)?.text;
        let sprache = Sprache::aus_datei(&s).unwrap_or(Sprache::Luau);
        let ereignisse = self.welt.get::<&Ereignisse>(o).map(|e| e.0.clone()).unwrap_or_default();
        Some(script_quelle::fuer_laufzeit(text, sprache, &ereignisse))
    }

    /// „▶ Run“ einer Live-Funktion: Script in einer eigenen, kurzen Laufzeit laden und die Funktion
    /// mit den Werten aus den Eigenschaften aufrufen – im Bearbeiten-Modus direkt auf der Szene
    /// (z. B. um eine Generierungs-Funktion auszuprobieren). Rückgängig macht alles wieder weg.
    #[cfg(not(target_arch = "wasm32"))]
    fn live_ausfuehren(&mut self, o: Objekt, funktion: &str) {
        let Ok(datei) = self.welt.get::<&Script>(o).map(|s| s.datei.clone()) else {
            return;
        };
        let Some(text) = self.puffer.get(&datei).map(|p| p.text.clone()) else {
            return;
        };
        let sprache = Sprache::aus_datei(&datei).unwrap_or(Sprache::Luau);
        let argumente: Vec<String> = self
            .welt
            .get::<&Live>(o)
            .ok()
            .and_then(|l| l.0.get(funktion).cloned())
            .unwrap_or_default();
        let quelle = script_quelle::fuer_live_aufruf(&text, sprache, funktion, &argumente);
        let name = self.name(o);
        let eltern = self.eltern(o);
        let mut lz = match self.motor_holen(sprache).and_then(|m| m.starten()) {
            Ok(lz) => lz,
            Err(e) => return self.melden(Art::Fehler, format!("Run: {e}")),
        };
        lz.kontext().texturen = self.assets.textur_namen().clone();
        lz.kontext().sound_namen = self.audio.namen().cloned().collect();
        if self.modus == Modus::Bearbeiten {
            self.markieren();
        }
        self.melden(
            Art::System,
            format!("⚡ Run: {name}.{funktion}({})", argumente.join(", ")),
        );
        if lz.script_laden(&mut self.welt, &name, &quelle, Some(o), eltern) {
            lz.start(&mut self.welt);
        }
        // Meldungen und Sounds wie beim Spielen übernehmen, danach ist die Laufzeit wieder weg
        self.laufzeiten.push((sprache, lz));
        self.meldungen_uebernehmen();
        self.laufzeiten.pop();
        self.geaendert = true;
    }

    /// Gast-Modul einer Sprache im Hintergrund übersetzen lassen, falls noch nicht geschehen.
    fn motor_vorbereiten(&mut self, sprache: Sprache) {
        #[cfg(not(target_arch = "wasm32"))]
        self.motoren
            .entry(sprache)
            .or_insert_with(|| motor_im_hintergrund(sprache));
        #[cfg(target_arch = "wasm32")]
        let _ = sprache;
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

    /// `timbre.broadcast` über Sprachgrenzen: Nachrichten jeder Laufzeit an alle anderen zustellen.
    /// Antworten werden in weiteren Runden verteilt (höchstens 8, gegen endloses Hin und Her).
    #[cfg(not(target_arch = "wasm32"))]
    fn nachrichten_verteilen(&mut self) {
        for _ in 0..8 {
            let mut post: Vec<(usize, Vec<timbre_core::scripting::Nachricht>)> = Vec::new();
            for (i, (_, lz)) in self.laufzeiten.iter_mut().enumerate() {
                let n = std::mem::take(&mut lz.kontext().ausgehend);
                if !n.is_empty() {
                    post.push((i, n));
                }
            }
            if post.is_empty() || self.laufzeiten.len() < 2 {
                return;
            }
            for (von, nachrichten) in post {
                for (i, (_, lz)) in self.laufzeiten.iter_mut().enumerate() {
                    if i != von && !lz.abgestuerzt() {
                        for n in &nachrichten {
                            lz.nachricht_zustellen(&mut self.welt, n);
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn meldungen_uebernehmen(&mut self) {
        self.nachrichten_verteilen();
        let mut meldungen = Vec::new();
        for (_, lz) in self.laufzeiten.iter_mut() {
            // Sound-Wünsche der Scripts ausführen
            for b in std::mem::take(&mut lz.kontext().sound_befehle) {
                match b {
                    SoundBefehl::Abspielen {
                        name,
                        lautstaerke,
                        schleife,
                    } => {
                        if let Err(e) = self.audio.abspielen(&name, lautstaerke, schleife) {
                            lz.kontext().meldungen.push(Meldung::Fehler(e));
                        }
                    }
                    SoundBefehl::Stoppen(name) => self.audio.stoppen(&name),
                }
            }
            meldungen.append(&mut lz.kontext().meldungen);
        }
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
    fn freie_datei(&self, name: &str, ausser: Option<&str>, sprache: Sprache) -> String {
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
        let endung = sprache.endung();
        let mut datei = format!("{basis}.{endung}");
        let mut n = 2;
        while belegt(&datei) {
            datei = format!("{basis}{n}.{endung}");
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

    fn script_anlegen(&mut self, eltern: Option<Objekt>, sprache: Sprache) {
        self.motor_vorbereiten(sprache);
        // Scripts hängen nicht unter Scripts – dann unter dessen Eltern.
        let eltern = eltern.and_then(|e| {
            if self.welt.get::<&Script>(e).is_ok() {
                self.eltern(e)
            } else {
                Some(e)
            }
        });
        self.markieren();
        let datei = self.freie_datei("Script", None, sprache);
        self.puffer.insert(
            datei.clone(),
            Puffer {
                text: sprache.vorlage().into(),
                gespeichert: None,
            },
        );
        let o = self.welt.spawn((
            Name(projekt::ohne_script_endung(&datei).into()),
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
        let sprache = Sprache::aus_datei(&alt).unwrap_or(Sprache::Luau);
        self.script_datei_wechseln(o, &alt, self.freie_datei(&name, Some(&alt), sprache));
    }

    /// Sprache eines Scripts wechseln (= andere Dateiendung). Der Text bleibt, wie er ist.
    fn sprache_wechseln(&mut self, o: Objekt, sprache: Sprache) {
        self.motor_vorbereiten(sprache);
        let Ok(alt) = self.welt.get::<&Script>(o).map(|s| s.datei.clone()) else {
            return;
        };
        self.markieren();
        let neu = self.freie_datei(projekt::ohne_script_endung(&alt), Some(&alt), sprache);
        self.script_datei_wechseln(o, &alt, neu);
    }

    /// Script-Objekt zeigt ab jetzt auf die Datei `neu` (wird beim Speichern geschrieben).
    fn script_datei_wechseln(&mut self, o: Objekt, alt: &str, neu: String) {
        if neu == alt {
            return;
        }
        // Alten Puffer behalten – Rückgängig kann wieder auf den alten Namen zeigen.
        let text = self.puffer.get(alt).map(|p| p.text.clone()).unwrap_or_default();
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
        if self.tab == Tab::Script(alt.to_string()) {
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
            if let Ok(r) = self.welt.get::<&Raum>(alt) {
                let mut r = *r;
                if alt == o {
                    r.position.x += 1.5;
                }
                b.add(r);
            }
            if let Ok(x) = self.welt.get::<&Mesh3d>(alt) {
                b.add(*x);
            }
            if let Ok(x) = self.welt.get::<&Bewegung>(alt) {
                b.add(*x);
            }
            if let Ok(x) = self.welt.get::<&Kollision>(alt) {
                b.add(*x);
            }
            if let Ok(x) = self.welt.get::<&HitBoxGroesse>(alt) {
                b.add(*x);
            }
            if let Ok(x) = self.welt.get::<&Masse>(alt) {
                b.add(*x);
            }
            if let Ok(x) = self.welt.get::<&ScriptVerweise>(alt) {
                b.add((*x).clone());
            }
            if let Ok(x) = self.welt.get::<&Ereignisse>(alt) {
                b.add((*x).clone());
            }
            if let Ok(x) = self.welt.get::<&Live>(alt) {
                b.add((*x).clone());
            }
            if let Ok(s) = self.welt.get::<&Script>(alt).map(|s| (*s).clone()) {
                let sprache = Sprache::aus_datei(&s.datei).unwrap_or(Sprache::Luau);
                let datei = self.freie_datei(projekt::ohne_script_endung(&s.datei), None, sprache);
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
        if let Some((_, o)) = bester {
            return Some(o);
        }
        // Leere Objekte (ohne Bild): kleine Raute an ihrer Position
        let radius = 10.0 / self.kamera.zoom;
        self.welt
            .query::<(Objekt, &Transform)>()
            .without::<&Bild>()
            .without::<&Instanz>()
            .without::<&Raum>()
            .iter()
            .filter_map(|(o, _)| welt::welt_transform(&self.welt, o).map(|t| (o, t)))
            .filter(|(_, t)| t.position.distance(punkt) <= radius)
            .map(|(o, _)| o)
            .next()
    }

    /// Neues leeres Objekt: nur Name, Position (Transform) und Eltern – alles andere über Properties.
    fn leeres_objekt_anlegen(&mut self, eltern: Option<Objekt>) {
        self.markieren();
        let o = if self.ansicht3d {
            self.welt.spawn((
                Name("Objekt".into()),
                Raum {
                    position: self.kamera3d.ziel,
                    ..Default::default()
                },
            ))
        } else {
            self.welt
                .spawn((Name("Objekt".into()), Transform::bei(self.kamera.position)))
        };
        if let Some(e) = eltern.filter(|e| self.welt.get::<&Script>(*e).is_err()) {
            welt::eltern_setzen(&mut self.welt, o, Some(e));
        }
        self.auswahl = Some(o);
        self.tab = Tab::Szene;
    }

    fn objekt_anlegen(&mut self, name: &str, textur_name: &str, groesse: Vec2) {
        let Some(&textur) = self.assets.textur_namen().get(textur_name) else {
            return;
        };
        self.markieren();
        let o = self.welt.spawn((
            Name(name.into()),
            Transform::bei(self.kamera.position),
            Bild::new(textur, groesse),
        ));
        self.auswahl = Some(o);
    }

    /// Größe, in der ein Bild neu in die Szene kommt: Pixelgröße, aber höchstens 256 an der längsten Seite.
    fn start_groesse(&self, textur: TexturId) -> Vec2 {
        let (b, h) = self.assets.pixel_groesse(textur).unwrap_or((64, 64));
        let g = vec2(b as f32, h as f32);
        g * (256.0 / g.max_element()).min(1.0)
    }

    // -----------------------------------------------------------------------
    // Import: Dateien ins Fenster ziehen
    // -----------------------------------------------------------------------

    /// Gezogene Dateien importieren: PNG → Textur (+ Objekt in der Szene), WAV/OGG/MP3/FLAC → Sound.
    /// Wird nach der Oberfläche aufgerufen, weil Texturen auf die Grafikkarte müssen.
    pub fn importieren(&mut self, renderer: &mut Renderer) {
        let dateien = std::mem::take(&mut self.importe);
        #[cfg(not(target_arch = "wasm32"))]
        for (i, pfad) in dateien.iter().enumerate() {
            let endung = pfad
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            let ergebnis = if endung == "png" {
                self.bild_importieren(renderer, pfad, i)
            } else if timbre_core::audio::ist_sound_datei(&endung) {
                self.sound_importieren(pfad)
            } else {
                Err(format!(
                    "{}: unbekannte Dateiart – Timbre kennt PNG (Bilder) und WAV/OGG/MP3/FLAC (Sounds)",
                    pfad.display()
                ))
            };
            match ergebnis {
                Ok(text) => self.melden(Art::System, text),
                Err(e) => self.melden(Art::Fehler, format!("Import: {e}")),
            }
        }
        #[cfg(target_arch = "wasm32")]
        if !dateien.is_empty() {
            let _ = renderer;
            self.melden(Art::System, "Import geht im Browser noch nicht.");
        }
    }

    /// Freier Name: `name`, sonst `name2`, `name3` …
    fn freier_name(name: &str, belegt: impl Fn(&str) -> bool) -> String {
        let mut n = 2;
        let mut kandidat = name.to_string();
        while belegt(&kandidat) {
            kandidat = format!("{name}{n}");
            n += 1;
        }
        kandidat
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn bild_importieren(
        &mut self,
        renderer: &mut Renderer,
        pfad: &std::path::Path,
        nr: usize,
    ) -> Result<String, String> {
        let daten = std::fs::read(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
        let (breite, hoehe, pixel) =
            timbre_core::assets::png_lesen(&daten).map_err(|e| format!("{}: {e}", pfad.display()))?;
        let wunsch = projekt::ohne_endung(&pfad.to_string_lossy());
        let name = Self::freier_name(&wunsch, |n| self.assets.textur_namen().contains_key(n));
        let textur = tbr::Textur::neu(name.clone(), breite, hoehe, pixel).map_err(|e| e.to_string())?;
        let datei = projekt::textur_speichern(&self.ordner, &textur)?;
        let id = self
            .assets
            .textur_laden(renderer, &textur.zu_bytes())
            .map_err(|e| e.to_string())?;

        let mut text = format!("🖼 Bild '{name}' importiert ({breite} × {hoehe}) → texturen/{datei}");
        if self.modus == Modus::Bearbeiten {
            // Neues Objekt mitten im Bild; mehrere Dateien auf einmal leicht versetzt
            self.markieren();
            let versatz = vec2(nr as f32, -(nr as f32)) * 24.0 / self.kamera.zoom;
            let o = self.welt.spawn((
                Name(name.clone()),
                Transform::bei(self.kamera.position + versatz),
                Bild::new(id, self.start_groesse(id)),
            ));
            self.auswahl = Some(o);
            self.tab = Tab::Szene;
        } else {
            text += " – Objekt nur im Bearbeiten-Modus, Textur ist aber schon nutzbar";
        }
        Ok(text)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn sound_importieren(&mut self, pfad: &std::path::Path) -> Result<String, String> {
        let daten = std::fs::read(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
        let wunsch = projekt::ohne_endung(&pfad.to_string_lossy());
        let name = Self::freier_name(&wunsch, |n| self.audio.hat(n));
        // Erst prüfen, ob sich der Sound lesen lässt, dann kopieren
        Audio::pruefen(&daten).map_err(|e| format!("{}: {e}", pfad.display()))?;
        let name_datei = projekt::sound_kopieren(&self.ordner, pfad, &name)?;
        self.audio.sound_setzen(&name_datei, daten)?;
        let _ = self.audio.abspielen(&name_datei, 1.0, false); // einmal anspielen
        Ok(format!(
            "🔊 Sound '{name_datei}' importiert → sounds/ – im Script: timbre.play_sound(\"{name_datei}\")"
        ))
    }

    fn aktion(&mut self, a: Aktion) {
        match a {
            Aktion::Auswaehlen(o) => self.auswahl = Some(o),
            Aktion::Oeffnen(o) => {
                self.auswahl = Some(o);
                if let Ok(datei) = self.welt.get::<&Script>(o).map(|s| s.datei.clone()) {
                    self.script_oeffnen(&datei);
                } else if self.welt.get::<&Raum>(o).is_ok()
                    && (self.ansicht3d || self.welt.get::<&Transform>(o).is_err())
                {
                    // 3D-Objekt: Kamera darauf richten
                    self.tab = Tab::Szene;
                    self.ansicht3d = true;
                    self.kamera3d_zu(o);
                } else if let Some(t) = welt::welt_transform(&self.welt, o) {
                    // Doppelklick auf ein sichtbares Objekt: Kamera hinfahren
                    self.tab = Tab::Szene;
                    self.kamera.position = t.position;
                    self.kamera_vorher = self.kamera;
                }
            }
            Aktion::ScriptHinzufuegen(eltern, sprache) => self.script_anlegen(eltern, sprache),
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
            // Property „Scripts“: Für jedes Objekt eine eigene Kopie des Scripts (Instanz), die unter
            // dem Objekt hängt → script.object / script.parent = dieses Objekt. Die Szene ist schon
            // gemerkt (Schnappschuss), bei ■ verschwinden die Kopien wieder.
            let verweise: Vec<(Objekt, Vec<String>)> = self
                .welt
                .query::<(Objekt, &ScriptVerweise)>()
                .iter()
                .map(|(o, v)| (o, v.0.clone()))
                .collect();
            let mut verlinkt: Vec<String> = Vec::new();
            for (ziel, dateien) in verweise {
                for datei in dateien {
                    let Some(vorlage) = self.script_objekt(&datei) else {
                        self.melden(
                            Art::Fehler,
                            format!("{}: verknüpftes Script scripts/{datei} gibt es nicht", self.name(ziel)),
                        );
                        continue;
                    };
                    let mut b = hecs::EntityBuilder::new();
                    b.add(Name(self.name(vorlage)));
                    b.add(Script {
                        datei: datei.clone(),
                        aktiv: true,
                    });
                    b.add(Eltern(ziel));
                    b.add(Instanz);
                    if let Ok(e) = self.welt.get::<&Ereignisse>(vorlage).map(|e| (*e).clone()) {
                        b.add(e);
                    }
                    self.welt.spawn(b.build());
                    verlinkt.push(datei);
                }
            }
            // Alle aktiven Script-Objekte, nach Namen sortiert. Verknüpfte Scripts laufen nur als Kopien.
            let mut scripts: Vec<(Objekt, String, Script, Option<Objekt>)> = self
                .welt
                .query::<(Objekt, &Script, Option<&Instanz>)>()
                .iter()
                .filter(|(_, s, i)| s.aktiv && (i.is_some() || !verlinkt.contains(&s.datei)))
                .map(|(o, s, _)| (o, String::new(), s.clone(), None))
                .collect();
            for s in scripts.iter_mut() {
                s.1 = self.name(s.0);
                s.3 = self.eltern(s.0);
            }
            scripts.sort_by(|a, b| natuerlich(&a.1, &b.1));

            // Eine Laufzeit pro Sprache, die vorkommt (Luau immer – auch ohne Scripts billig)
            let mut sprachen: Vec<Sprache> = vec![Sprache::Luau];
            for (_, _, script, _) in &scripts {
                if let Some(sp) = Sprache::aus_datei(&script.datei) {
                    if !sprachen.contains(&sp) {
                        sprachen.push(sp);
                    }
                }
            }
            self.laufzeiten.clear();
            for sp in sprachen {
                match self.motor_holen(sp).and_then(|m| m.starten()) {
                    Ok(mut lz) => {
                        lz.kontext().texturen = self.assets.textur_namen().clone();
                        lz.kontext().sound_namen = self.audio.namen().cloned().collect();
                        self.laufzeiten.push((sp, lz));
                    }
                    Err(e) => self.melden(Art::Fehler, format!("Scripting ({}) nicht verfügbar: {e}", sp.name())),
                }
            }

            let quellen: HashMap<Objekt, String> = scripts
                .iter()
                .filter_map(|(o, _, _, _)| self.quelle_fuer(*o).map(|q| (*o, q)))
                .collect();
            let mut fehlend = Vec::new();
            for (o, name, script, eltern) in &scripts {
                let sp = Sprache::aus_datei(&script.datei);
                let Some((_, lz)) = self.laufzeiten.iter_mut().find(|(s, _)| Some(*s) == sp) else {
                    continue; // Sprache nicht verfügbar – Fehler wurde schon gemeldet
                };
                match quellen.get(o) {
                    Some(q) => {
                        lz.script_laden(&mut self.welt, name, q, Some(*o), *eltern);
                    }
                    None => fehlend.push(format!("{name}: Datei scripts/{} fehlt", script.datei)),
                }
            }
            for (_, lz) in self.laufzeiten.iter_mut() {
                lz.start(&mut self.welt);
            }
            for f in fehlend {
                self.melden(Art::Fehler, f);
            }
            self.meldungen_uebernehmen();
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
            self.laufzeiten.clear();
            self.texte.clear();
            self.audio.stoppen("");
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
    fn motor_holen(&mut self, sprache: Sprache) -> Result<&ScriptMotor, String> {
        let motor = self
            .motoren
            .entry(sprache)
            .or_insert_with(|| motor_im_hintergrund(sprache));
        if let Motor::Laedt(rx) = motor {
            // Notfalls warten, bis das Übersetzen fertig ist (dauert nur beim ersten Mal).
            *motor = match rx.recv() {
                Ok(Ok(m)) => Motor::Bereit(m),
                Ok(Err(e)) => Motor::Kaputt(e),
                Err(_) => Motor::Kaputt("Übersetzen abgebrochen".into()),
            };
        }
        match motor {
            Motor::Bereit(m) => Ok(m),
            Motor::Kaputt(e) => Err(e.clone()),
            Motor::Laedt(_) => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // Speichern
    // -----------------------------------------------------------------------

    /// Geänderte Scripts auf die Festplatte schreiben. Gibt die gespeicherten Dateien zurück.
    fn scripts_speichern(&mut self) -> Vec<String> {
        #[allow(unused_mut)]
        let mut gespeichert = Vec::new();
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
                    Ok(()) => {
                        self.puffer.get_mut(&d).unwrap().gespeichert = Some(text);
                        gespeichert.push(d);
                    }
                    Err(e) => self.melden(Art::Fehler, format!("Speichern fehlgeschlagen: {e}")),
                }
            }
        }
        gespeichert
    }

    /// Läuft das Spiel, übernimmt es die neue Fassung dieser Script-Datei sofort (Live-Neuladen).
    #[cfg(not(target_arch = "wasm32"))]
    fn live_neu_laden(&mut self, datei: &str) {
        if self.modus != Modus::Laeuft {
            return;
        }
        if !self.puffer.contains_key(datei) {
            return;
        }
        let objekte: Vec<Objekt> = self
            .welt
            .query::<(Objekt, &Script)>()
            .iter()
            .filter(|(_, s)| s.datei == datei && s.aktiv)
            .map(|(o, _)| o)
            .collect();
        for o in objekte {
            let name = self.name(o);
            let eltern = self.eltern(o);
            let Some(text) = self.quelle_fuer(o) else { continue };
            let sprache = Sprache::aus_datei(datei);
            let Some((_, lz)) = self.laufzeiten.iter_mut().find(|(s, _)| Some(*s) == sprache) else {
                return self.melden(
                    Art::System,
                    format!("{name}: Diese Sprache lief beim ▶ noch nicht mit – ■ Stopp und neu ▶ starten."),
                );
            };
            if lz.abgestuerzt() {
                return self.melden(Art::System, "Scripts sind angehalten – ■ Stopp und neu ▶ starten.");
            }
            match lz.script_neu_laden(&mut self.welt, &name, &text, o, eltern) {
                Ok(true) => self.melden(Art::System, format!("🔄 {name} neu geladen")),
                Ok(false) => {} // Fehler steht schon in den Meldungen
                Err(e) => self.melden(Art::System, e),
            }
            self.fehler_zeilen.remove(datei);
            self.meldungen_uebernehmen();
        }
    }

    /// Wurden Scripts außerhalb des Studios geändert (z. B. in VS Code)? Dann übernehmen –
    /// und wenn das Spiel läuft, sofort neu laden. Hat man dasselbe Script auch im Studio
    /// ungespeichert geändert, gewinnt das Studio (mit Hinweis).
    #[cfg(not(target_arch = "wasm32"))]
    fn platte_abgleichen(&mut self) {
        let mut dateien: Vec<String> = self.welt.query::<&Script>().iter().map(|s| s.datei.clone()).collect();
        dateien.sort();
        dateien.dedup();
        for d in dateien {
            let Ok(platte) = std::fs::read_to_string(self.ordner.join("scripts").join(&d)) else {
                continue;
            };
            let platte = platte.replace("\r\n", "\n");
            let Some(p) = self.puffer.get_mut(&d) else { continue };
            let Some(gespeichert) = p.gespeichert.as_ref() else {
                continue;
            };
            if *gespeichert == platte {
                continue;
            }
            if !p.geaendert() {
                p.text = platte.clone();
                p.gespeichert = Some(platte);
                self.konflikte.remove(&d);
                self.melden(
                    Art::System,
                    format!("📝 scripts/{d} wurde außerhalb geändert – übernommen"),
                );
                self.live_neu_laden(&d);
            } else if self.konflikte.get(&d) != Some(&platte) {
                self.konflikte.insert(d.clone(), platte);
                self.melden(
                    Art::Fehler,
                    format!(
                        "scripts/{d} wurde außerhalb geändert, ist aber auch im Studio ungespeichert geändert. \
                         Strg+S überschreibt die Datei mit der Studio-Fassung."
                    ),
                );
            }
        }
    }

    fn alles_speichern(&mut self) {
        let gespeichert = self.scripts_speichern();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.modus == Modus::Laeuft {
                if gespeichert.is_empty() {
                    return self.melden(
                        Art::System,
                        "Nichts zu speichern. Die Szene wird nur im Bearbeiten-Modus gespeichert.",
                    );
                }
                for d in gespeichert {
                    self.live_neu_laden(&d);
                }
                return;
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
                    if self.ansicht3d {
                        self.kamera3d_bewegen(r, dt);
                    } else {
                        self.kamera.position += r.normalize_or_zero() * 700.0 / self.kamera.zoom * dt;
                    }
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
                {
                    let mut eingabe = input.clone();
                    if !tastatur_frei {
                        eingabe.tasten_loeschen();
                    }
                    if !self.maus_ueber_szene {
                        eingabe.maus_loeschen();
                    }
                    let mut abgestuerzt = Vec::new();
                    for (sp, lz) in self.laufzeiten.iter_mut() {
                        if lz.abgestuerzt() {
                            continue;
                        }
                        let k = lz.kontext();
                        k.input = eingabe.clone();
                        k.maus_welt = self.maus_welt;
                        lz.update(&mut self.welt, dt);
                        if let Some(kamera) = lz.kontext().kamera.take() {
                            self.kamera = kamera;
                        }
                        if lz.abgestuerzt() {
                            abgestuerzt.push(sp.name());
                        }
                    }
                    self.meldungen_uebernehmen();
                    for name in abgestuerzt {
                        self.melden(
                            Art::System,
                            format!("{name}-Scripts angehalten. Mit ■ stoppen, Fehler beheben, erneut ▶."),
                        );
                    }
                }
                welt::bewegen(&mut self.welt, dt);

                // Kollisionen auflösen (einmal) und Berührungen an alle Laufzeiten melden (on_touch)
                let paare = timbre_core::physik::schritt(&mut self.welt);
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.texte.clear();
                    for (_, lz) in self.laufzeiten.iter_mut() {
                        lz.beruehrungen_melden(&mut self.welt, &paare);
                        self.texte.extend(lz.kontext().texte.clone());
                    }
                    self.meldungen_uebernehmen();
                }
                #[cfg(target_arch = "wasm32")]
                let _ = paare;
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
        if self.marke.is_none() {
            self.marke = Some(Marke::laden(&ctx));
        }
        // Dateien ins Fenster gezogen? (PNG → Bild, WAV/OGG/MP3/FLAC → Sound)
        let (gezogen, schwebt) = ctx.input(|i| {
            (
                i.raw
                    .dropped_files
                    .iter()
                    .map(|d| d.path().to_path_buf())
                    .filter(|p| !p.as_os_str().is_empty())
                    .collect::<Vec<_>>(),
                !i.raw.hovered_files.is_empty(),
            )
        });
        self.importe.extend(gezogen);
        #[cfg(not(target_arch = "wasm32"))]
        if self.platte_geprueft.elapsed().as_secs_f32() > 1.0 {
            self.platte_geprueft = web_time::Instant::now();
            self.platte_abgleichen();
        }
        if schwebt {
            let r = ctx.content_rect();
            let maler = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("import")));
            maler.rect_filled(r, 0.0, Color32::from_black_alpha(150));
            maler.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "Loslassen zum Importieren\nPNG → Bild · WAV / OGG / MP3 / FLAC → Sound",
                egui::FontId::proportional(24.0),
                Color32::WHITE,
            );
        }
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

        // Startanimation (Eis, Schnee, Logo) über allem – ein Klick überspringt
        if self.startanimation {
            let t = self.gestartet_um.elapsed().as_secs_f32();
            if let Some(m) = &self.marke {
                self.startanimation = marke::startanimation(&ctx, m, t);
            }
        }
    }

    fn leiste_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Logo: dreht sich langsam, solange das Spiel läuft
            if let Some(m) = &self.marke {
                let zeit = ui.input(|i| i.time) as f32;
                let winkel = if self.modus == Modus::Laeuft { zeit * 0.8 } else { 0.0 };
                ui.add(
                    egui::Image::new(&m.logo)
                        .fit_to_exact_size(egui::vec2(20.0, 20.0))
                        .rotate(winkel, egui::vec2(0.5, 0.5)),
                );
            }
            ui.label(RichText::new("Timbre Studio").strong().color(eis::EIS));
            ui.label(RichText::new(&self.projekt_name).weak());
            ui.separator();
            match self.modus {
                Modus::Bearbeiten => {
                    let knopf = egui::Button::new(RichText::new("▶ Ausführen").color(Color32::WHITE))
                        .fill(eis::NORDLICHT.gamma_multiply(0.85));
                    if ui.add(knopf).on_hover_text("Scripts starten (F5)").clicked() {
                        self.starten();
                    }
                }
                Modus::Laeuft => {
                    // Stopp-Knopf pulsiert sanft, solange das Spiel läuft
                    let puls = 0.8 + 0.2 * ((ui.input(|i| i.time) as f32) * 3.0).sin();
                    let knopf = egui::Button::new(RichText::new("■ Stopp").color(Color32::WHITE))
                        .fill(eis::GLUT.gamma_multiply(puls));
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
                .add_enabled(bearbeiten && !self.zurueck.is_empty(), egui::Button::new("↩"))
                .on_hover_text(format!("Rückgängig (Strg+Z) – {} Schritte", self.zurueck.len()))
                .clicked()
            {
                self.rueckgaengig();
            }
            if ui
                .add_enabled(bearbeiten && !self.vor.is_empty(), egui::Button::new("↪"))
                .on_hover_text("Wiederholen (Strg+Y)")
                .clicked()
            {
                self.wiederholen();
            }
            ui.separator();
            // 2D / 3D
            let mut drei = self.ansicht3d;
            ui.selectable_value(&mut drei, false, "2D")
                .on_hover_text("Szene in 2D: Sprites, Kamera von oben");
            ui.selectable_value(&mut drei, true, "3D")
                .on_hover_text("Szene in 3D: Körper (Mesh-Property), Kamera frei drehbar");
            if drei != self.ansicht3d {
                self.ansicht3d = drei;
                self.ziehen = None;
                self.ziehen3d = None;
                self.griff = None;
            }
            ui.separator();
            ui.checkbox(&mut self.gitter, "Gitter").on_hover_text(
                "Gitter anzeigen; Verschieben, Drehen (15°) und Skalieren rasten ein. Alt gedrückt = frei",
            );
            if self.gitter {
                ui.add(
                    egui::DragValue::new(&mut self.gitter_groesse)
                        .speed(1.0)
                        .range(4.0..=1024.0)
                        .suffix(" px"),
                );
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
            if ui
                .small_button("+ Objekt")
                .on_hover_text(
                    "Neues leeres Objekt (Name, Position, Eltern) – Bild, HitBox, Masse … über „Property hinzufügen“",
                )
                .clicked()
            {
                self.leeres_objekt_anlegen(None);
            }
            ui.menu_button("+ Körper", |ui| {
                for f in Form3d::ALLE {
                    if ui.button(f.name()).clicked() {
                        self.koerper_anlegen(f);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("Neuer 3D-Körper (Transform 3D + Mesh) – wechselt in die 3D-Ansicht");
            if ui
                .small_button("+ Script")
                .on_hover_text("Neues Script – unter dem ausgewählten Objekt, falls eins ausgewählt ist")
                .clicked()
            {
                self.script_anlegen(self.auswahl, Sprache::Luau);
            }
        });
        self.assets_ui(ui);
        ui.separator();

        // Baum aufbauen: Eltern → Kinder, Kinder natürlich sortiert
        let mut infos: HashMap<Objekt, Knoten> = HashMap::new();
        for (o, n, s, b) in self
            .welt
            .query::<(Objekt, Option<&Name>, Option<&Script>, Option<&Bild>)>()
            .without::<&Instanz>() // laufende Script-Kopien nicht zeigen
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
                for sprache in Sprache::ALLE {
                    if ui.button(format!("📜 Neues Script ({})", sprache.name())).clicked() {
                        aktionen.push(Aktion::ScriptHinzufuegen(None, sprache));
                    }
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
            if let Some(m) = &self.marke {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    // Kristall „atmet“ leicht
                    let t = ui.input(|i| i.time) as f32;
                    let g = 110.0 + (t * 0.8).sin() * 6.0;
                    ui.add(
                        egui::Image::new(&m.kristall)
                            .fit_to_exact_size(egui::vec2(g, g))
                            .tint(Color32::WHITE.gamma_multiply(0.12)),
                    );
                });
            }
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
            let mut sprache_neu = None;
            ui.add_space(6.0);
            ui.label(RichText::new("📜 Script").strong());
            egui::Grid::new("script").num_columns(2).show(ui, |ui| {
                ui.label("Sprache");
                let jetzt = Sprache::aus_datei(&s.datei).unwrap_or(Sprache::Luau);
                let mut wahl = jetzt;
                egui::ComboBox::from_id_salt("sprache")
                    .selected_text(jetzt.name())
                    .show_ui(ui, |ui| {
                        for sp in Sprache::ALLE {
                            ui.selectable_value(&mut wahl, sp, sp.name());
                        }
                    })
                    .response
                    .on_hover_text(
                        "Andere Sprache = andere Dateiendung. Der Text bleibt – Luau und Lua sind sich sehr ähnlich.",
                    );
                if wahl != jetzt {
                    sprache_neu = Some(wahl);
                }
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
            if let Some(sp) = sprache_neu {
                self.sprache_wechseln(o, sp);
            }
            self.script_ereignisse_ui(ui, o, &s.datei);
            self.live_ui(ui, o, &s.datei);
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
            let mut grad = t.drehung.to_degrees().rem_euclid(360.0);
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

        // Properties (wie Komponenten in Unity): jede als eigener Block, mit 🗙 entfernbar
        if self.welt.get::<&Script>(o).is_err() {
            self.properties_ui(ui, o);
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

    /// Ereignisse eines Scripts: welche Funktion bei Start/Update/Berührung/Nachricht aufgerufen wird.
    /// Beim Tippen werden passende Funktionen aus dem Script vorgeschlagen.
    fn script_ereignisse_ui(&mut self, ui: &mut egui::Ui, o: Objekt, datei: &str) {
        let sprache = Sprache::aus_datei(datei).unwrap_or(Sprache::Luau);
        let funktionen: Vec<String> = self
            .puffer
            .get(datei)
            .map(|p| script_quelle::funktionen(&p.text, sprache))
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.name)
            .collect();
        let vorher = self.welt.get::<&Ereignisse>(o).map(|e| e.0.clone()).unwrap_or_default();
        let mut karte = vorher.clone();
        ui.add_space(4.0);
        egui::Frame::new()
            .fill(eis::BLOCK)
            .stroke(Stroke::new(1.0, eis::RAND))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("⚑ Ereignisse").strong().color(eis::EIS))
                    .on_hover_text("Welche Funktion des Scripts wird wann aufgerufen? Leer = Ereignis aus.");
                for (ereignis, beschriftung) in script_quelle::EREIGNISSE {
                    let mut wert = karte.get(ereignis).cloned().unwrap_or_else(|| ereignis.to_string());
                    let feld_id = egui::Id::new(("ereignis", datei, ereignis));
                    ui.horizontal(|ui| {
                        ui.add_sized([96.0, 18.0], egui::Label::new(RichText::new(beschriftung).small()));
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut wert)
                                .id(feld_id)
                                .hint_text("(aus)")
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                        if r.changed() {
                            karte.insert(ereignis.to_string(), wert.clone());
                        }
                    });
                    let name = wert.trim();
                    // Warnen nur bei selbst gewählten Namen (Standardnamen dürfen fehlen = Ereignis unbenutzt)
                    if !name.is_empty() && name != ereignis && !funktionen.iter().any(|f| f == name) {
                        ui.label(
                            RichText::new(format!("   ⚠ „{name}“ gibt es (noch) nicht im Script"))
                                .small()
                                .color(Color32::from_rgb(0xF0, 0xB4, 0x5A)),
                        );
                    }
                    // Vorschläge, solange das Feld Fokus hat
                    if ui.memory(|m| m.has_focus(feld_id)) {
                        let klein = name.to_lowercase();
                        let passend: Vec<&String> = funktionen
                            .iter()
                            .filter(|f| f.to_lowercase().starts_with(&klein) && f.as_str() != name)
                            .take(6)
                            .collect();
                        if !passend.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.add_space(100.0);
                                for f in passend {
                                    if ui
                                        .add(egui::Button::new(RichText::new(f).monospace().small()).small())
                                        .clicked()
                                    {
                                        karte.insert(ereignis.to_string(), f.clone());
                                    }
                                }
                            });
                        }
                    }
                }
            });
        // Nur Abweichungen vom Standard speichern
        karte.retain(|k, v| v.trim() != k);
        if karte != vorher {
            if karte.is_empty() {
                let _ = self.welt.remove_one::<Ereignisse>(o);
            } else {
                let _ = self.welt.insert_one(o, Ereignisse(karte));
            }
            self.markieren();
        }
    }

    /// Live-Script: Schalter, und für jede `live`-Funktion ihre Argumente als Felder + „▶ Run“.
    fn live_ui(&mut self, ui: &mut egui::Ui, o: Objekt, datei: &str) {
        let sprache = Sprache::aus_datei(datei).unwrap_or(Sprache::Luau);
        let vorher = self.welt.get::<&Live>(o).ok().map(|l| l.0.clone());
        let mut live = vorher.clone();
        let mut ausfuehren: Option<String> = None;
        ui.add_space(4.0);
        egui::Frame::new()
            .fill(eis::BLOCK)
            .stroke(Stroke::new(1.0, if live.is_some() { eis::AKZENT } else { eis::RAND }))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut an = live.is_some();
                if ui
                    .checkbox(&mut an, RichText::new("⚡ Live-Script").strong().color(eis::EIS))
                    .on_hover_text(
                        "Funktionen mit „live“ davor bekommen hier Felder für ihre Argumente und einen ▶ Run-Knopf – \
                         zum Ausprobieren ohne das Spiel zu starten",
                    )
                    .changed()
                {
                    live = an.then(Default::default);
                }
                let Some(werte) = live.as_mut() else { return };
                let funktionen: Vec<script_quelle::Funktion> = self
                    .puffer
                    .get(datei)
                    .map(|p| script_quelle::funktionen(&p.text, sprache))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|f| f.live)
                    .collect();
                if funktionen.is_empty() {
                    let beispiel = match sprache {
                        Sprache::Python => "live def baue(anzahl, abstand):",
                        _ => "live function baue(anzahl, abstand)",
                    };
                    ui.label(
                        RichText::new(format!("Noch keine Live-Funktion. Schreibe z. B.\n{beispiel}"))
                            .small()
                            .weak(),
                    );
                }
                for f in funktionen {
                    let args = werte.entry(f.name.clone()).or_default();
                    args.resize(f.parameter.len(), String::new());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("{}({})", f.name, f.parameter.join(", "))).monospace(),
                            )
                            .truncate(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let knopf = egui::Button::new(RichText::new("▶ Run").color(Color32::WHITE))
                                .fill(eis::GLETSCHER);
                            if ui
                                .add(knopf)
                                .on_hover_text("Funktion jetzt aufrufen – Ergebnis bleibt in der Szene (Strg+Z macht es rückgängig)")
                                .clicked()
                            {
                                ausfuehren = Some(f.name.clone());
                            }
                        });
                    });
                    egui::Grid::new(("live", datei, &f.name)).num_columns(2).show(ui, |ui| {
                        for (p, wert) in f.parameter.iter().zip(args.iter_mut()) {
                            ui.label(RichText::new(p).small());
                            ui.add(
                                egui::TextEdit::singleline(wert)
                                    .hint_text("Zahl, Text, true/false")
                                    .desired_width(f32::INFINITY),
                            );
                            ui.end_row();
                        }
                    });
                }
            });
        if live != vorher {
            match &live {
                Some(l) => {
                    let _ = self.welt.insert_one(o, Live(l.clone()));
                }
                None => {
                    let _ = self.welt.remove_one::<Live>(o);
                }
            }
            self.markieren();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(f) = ausfuehren {
            self.live_ausfuehren(o, &f);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = ausfuehren;
    }

    /// Die Properties eines Objekts als Blöcke + „Property hinzufügen“.
    fn properties_ui(&mut self, ui: &mut egui::Ui, o: Objekt) {
        let mut entfernen: Option<Property> = None;

        // Bild
        if let Ok(mut b) = self.welt.get::<&Bild>(o).map(|b| *b) {
            let vorher = b;
            let mut namen: Vec<(String, TexturId)> = self
                .assets
                .textur_namen()
                .iter()
                .map(|(n, t)| (n.clone(), *t))
                .collect();
            namen.sort();
            let textur_name = self.assets.name_von(b.textur).unwrap_or("?").to_string();
            if property_block(ui, Property::Bild, |ui| {
                egui::Grid::new("bild").num_columns(2).show(ui, |ui| {
                    ui.label("Textur");
                    egui::ComboBox::from_id_salt("textur")
                        .selected_text(textur_name)
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
            }) {
                entfernen = Some(Property::Bild);
            }
            if b != vorher {
                let _ = self.welt.insert_one(o, b);
                self.markieren();
            }
        }

        // HitBox (Kollision)
        if let Ok(mut k) = self.welt.get::<&Kollision>(o).map(|k| *k) {
            let vorher = k;
            let hat_bild = self.welt.get::<&Bild>(o).is_ok();
            let eigene = self.welt.get::<&HitBoxGroesse>(o).ok().map(|g| g.0);
            let mut groesse = eigene;
            if property_block(ui, Property::HitBox, |ui| {
                egui::Grid::new("hitbox").num_columns(2).show(ui, |ui| {
                    ui.label("Form");
                    egui::ComboBox::from_id_salt("form")
                        .selected_text(match k.form {
                            Form::Kreis => "Kreis",
                            Form::Rechteck => "Rechteck",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut k.form, Form::Kreis, "Kreis");
                            ui.selectable_value(&mut k.form, Form::Rechteck, "Rechteck");
                        });
                    ui.end_row();
                    ui.label("Größe");
                    ui.horizontal(|ui| {
                        let mut eigen = groesse.is_some() || !hat_bild;
                        if hat_bild
                            && ui
                                .checkbox(&mut eigen, "eigene")
                                .on_hover_text("Aus = so groß wie das Bild")
                                .changed()
                        {
                            groesse = eigen.then_some(groesse.unwrap_or(Vec2::splat(64.0)));
                        }
                        if eigen {
                            let g = groesse.get_or_insert(Vec2::splat(64.0));
                            ui.add(egui::DragValue::new(&mut g.x).speed(1.0).prefix("b "));
                            ui.add(egui::DragValue::new(&mut g.y).speed(1.0).prefix("h "));
                        } else {
                            ui.label(RichText::new("wie das Bild").weak());
                        }
                    });
                    ui.end_row();
                    ui.label("Fest");
                    ui.checkbox(&mut k.fest, "blockiert")
                        .on_hover_text("Aus = nur Auslöser: meldet Berührungen (on_touch), blockiert aber nichts");
                    ui.end_row();
                    ui.label("Verankert");
                    ui.checkbox(&mut k.verankert, "bleibt stehen")
                        .on_hover_text("Wird bei Zusammenstößen nie weggeschoben (wie Anchored in Roblox)");
                    ui.end_row();
                });
            }) {
                entfernen = Some(Property::HitBox);
            }
            if k != vorher {
                let _ = self.welt.insert_one(o, k);
                self.markieren();
            }
            if groesse != eigene {
                match groesse {
                    Some(g) => {
                        let _ = self.welt.insert_one(o, HitBoxGroesse(g));
                    }
                    None => {
                        let _ = self.welt.remove_one::<HitBoxGroesse>(o);
                    }
                }
                self.markieren();
            }
        }

        // Masse
        if let Ok(mut m) = self.welt.get::<&Masse>(o).map(|m| *m) {
            let vorher = m;
            if property_block(ui, Property::Masse, |ui| {
                egui::Grid::new("masse").num_columns(2).show(ui, |ui| {
                    ui.label("Masse");
                    // Masselos → Feld ausgegraut
                    ui.add_enabled(
                        !m.masselos,
                        egui::DragValue::new(&mut m.wert)
                            .speed(0.05)
                            .range(0.001..=1_000_000.0)
                            .suffix(" kg"),
                    );
                    ui.end_row();
                    ui.label("Masselos");
                    ui.checkbox(&mut m.masselos, "").on_hover_text(
                        "Zählt wie keine Masse: wird bei Zusammenstößen ganz weggeschoben, schiebt selbst nichts",
                    );
                    ui.end_row();
                });
            }) {
                entfernen = Some(Property::Masse);
            }
            if m != vorher {
                let _ = self.welt.insert_one(o, m);
                self.markieren();
            }
        }

        // Bewegung
        if let Ok(mut bew) = self.welt.get::<&Bewegung>(o).map(|b| *b) {
            let vorher = bew;
            if property_block(ui, Property::Bewegung, |ui| {
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
            }) {
                entfernen = Some(Property::Bewegung);
            }
            if bew != vorher {
                let _ = self.welt.insert_one(o, bew);
                self.markieren();
            }
        }

        // Scripts (verlinkt)
        if let Ok(mut v) = self.welt.get::<&ScriptVerweise>(o).map(|v| v.0.clone()) {
            let vorher = v.clone();
            let mut dateien: Vec<String> = self
                .welt
                .query::<&Script>()
                .without::<&Instanz>()
                .iter()
                .map(|s| s.datei.clone())
                .collect();
            dateien.sort_by(|a, b| natuerlich(a, b));
            dateien.dedup();
            if property_block(ui, Property::Scripts, |ui| {
                ui.label(
                    RichText::new("Jedes Script läuft für dieses Objekt als eigene Kopie – im Script ist script.object dieses Objekt.")
                        .weak()
                        .small(),
                );
                let mut weg = None;
                for (i, d) in v.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("📜 {}", projekt::ohne_script_endung(d))).monospace());
                        if ui.small_button("🗙").on_hover_text("Verknüpfung lösen").clicked() {
                            weg = Some(i);
                        }
                    });
                }
                if let Some(i) = weg {
                    v.remove(i);
                }
                let mut neu: Option<String> = None;
                egui::ComboBox::from_id_salt("script_verknuepfen")
                    .selected_text("+ Script verknüpfen")
                    .show_ui(ui, |ui| {
                        for d in dateien.iter().filter(|d| !v.contains(d)) {
                            if ui.selectable_label(false, projekt::ohne_script_endung(d)).clicked() {
                                neu = Some(d.clone());
                            }
                        }
                    });
                v.extend(neu);
            }) {
                entfernen = Some(Property::Scripts);
            }
            if v != vorher {
                let _ = self.welt.insert_one(o, ScriptVerweise(v));
                self.markieren();
            }
        }

        // 3D: Transform 3D + Mesh
        if let Some(p) = self.properties3d_ui(ui, o) {
            entfernen = Some(p);
        }

        if let Some(p) = entfernen {
            self.property_entfernen(o, p);
        }

        // Property hinzufügen
        ui.add_space(6.0);
        let fehlend: Vec<Property> = Property::ALLE
            .into_iter()
            .filter(|p| !self.hat_property(o, *p))
            .collect();
        if !fehlend.is_empty() {
            let mut neu = None;
            ui.menu_button(RichText::new("+ Property hinzufügen").color(eis::AKZENT), |ui| {
                for p in fehlend {
                    if ui
                        .button(format!("{} {}", p.symbol(), p.name()))
                        .on_hover_text(p.beschreibung())
                        .clicked()
                    {
                        neu = Some(p);
                        ui.close();
                    }
                }
            });
            if let Some(p) = neu {
                self.property_hinzufuegen(o, p);
            }
        }
    }

    fn hat_property(&self, o: Objekt, p: Property) -> bool {
        match p {
            Property::Bild => self.welt.get::<&Bild>(o).is_ok(),
            Property::HitBox => self.welt.get::<&Kollision>(o).is_ok(),
            Property::Masse => self.welt.get::<&Masse>(o).is_ok(),
            Property::Bewegung => self.welt.get::<&Bewegung>(o).is_ok(),
            Property::Scripts => self.welt.get::<&ScriptVerweise>(o).is_ok(),
            Property::Raum => self.welt.get::<&Raum>(o).is_ok(),
            Property::Mesh => self.welt.get::<&Mesh3d>(o).is_ok(),
        }
    }

    fn property_hinzufuegen(&mut self, o: Objekt, p: Property) {
        let _ = match p {
            Property::Bild => {
                let textur = self
                    .assets
                    .textur_namen()
                    .get("jahresringe")
                    .or_else(|| self.assets.textur_namen().values().next())
                    .copied();
                match textur {
                    Some(t) => self.welt.insert_one(o, Bild::new(t, Vec2::splat(64.0))),
                    None => return,
                }
            }
            Property::HitBox => {
                if self.welt.get::<&Bild>(o).is_err() {
                    let _ = self.welt.insert_one(o, HitBoxGroesse(Vec2::splat(64.0)));
                }
                self.welt.insert_one(o, Kollision::default())
            }
            Property::Masse => self.welt.insert_one(o, Masse::default()),
            Property::Bewegung => self.welt.insert_one(o, Bewegung::default()),
            Property::Scripts => self.welt.insert_one(o, ScriptVerweise::default()),
            Property::Raum | Property::Mesh => {
                if self.welt.get::<&Raum>(o).is_err() {
                    let _ = self.welt.insert_one(
                        o,
                        Raum {
                            position: self.kamera3d.ziel,
                            ..Default::default()
                        },
                    );
                }
                if p == Property::Mesh {
                    let _ = self.welt.insert_one(o, Mesh3d::default());
                }
                Ok(())
            }
        };
        // Objekte ohne Platz in der Welt brauchen eine Position für die meisten Properties
        let dreid = matches!(p, Property::Scripts | Property::Raum | Property::Mesh);
        if self.welt.get::<&Transform>(o).is_err() && !dreid {
            let _ = self.welt.insert_one(o, Transform::bei(self.kamera.position));
        }
        self.markieren();
    }

    fn property_entfernen(&mut self, o: Objekt, p: Property) {
        match p {
            Property::Bild => {
                let _ = self.welt.remove_one::<Bild>(o);
            }
            Property::HitBox => {
                let _ = self.welt.remove_one::<Kollision>(o);
                let _ = self.welt.remove_one::<HitBoxGroesse>(o);
            }
            Property::Masse => {
                let _ = self.welt.remove_one::<Masse>(o);
            }
            Property::Bewegung => {
                let _ = self.welt.remove_one::<Bewegung>(o);
            }
            Property::Scripts => {
                let _ = self.welt.remove_one::<ScriptVerweise>(o);
            }
            Property::Raum => {
                let _ = self.welt.remove_one::<Raum>(o);
                let _ = self.welt.remove_one::<Mesh3d>(o);
            }
            Property::Mesh => {
                let _ = self.welt.remove_one::<Mesh3d>(o);
            }
        }
        self.markieren();
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

    /// Namen aus dem Projekt für Vorschläge in Texten (Sounds, Bilder, Objekte, Nachrichten).
    fn projektnamen(&self) -> vervollstaendigung::Projektnamen {
        let mut objekte: Vec<String> = self.welt.query::<&Name>().iter().map(|n| n.0.clone()).collect();
        objekte.sort_by(|a, b| natuerlich(a, b));
        objekte.dedup();
        let mut texturen: Vec<String> = self.assets.textur_namen().keys().cloned().collect();
        texturen.sort();
        #[cfg(not(target_arch = "wasm32"))]
        let mut sounds: Vec<String> = self.audio.namen().cloned().collect();
        #[cfg(target_arch = "wasm32")]
        let mut sounds: Vec<String> = Vec::new();
        sounds.sort();
        vervollstaendigung::Projektnamen {
            sounds,
            texturen,
            objekte,
            nachrichten: vervollstaendigung::Projektnamen::nachrichten_aus(
                self.puffer.values().map(|p| p.text.as_str()),
            ),
        }
    }

    fn editor_ui(&mut self, ui: &mut egui::Ui, datei: &str) {
        let id = egui::Id::new(("editor", datei));
        // Nur berechnen, wenn der Editor den Fokus hat (sonst gibt es keine Vorschläge)
        let projektnamen = if ui.ctx().memory(|m| m.has_focus(id)) {
            self.projektnamen()
        } else {
            Default::default()
        };
        let Some(puffer) = self.puffer.get_mut(datei) else {
            ui.label(RichText::new(format!("scripts/{datei} fehlt.")).color(Color32::from_rgb(0xE5, 0x6B, 0x5D)));
            return;
        };
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

        // Autovervollständigung: Tasten abfangen, bevor der Editor sie bekommt
        let fokus = ui.ctx().memory(|m| m.has_focus(id));
        let mut uebernehmen = false;
        let erzwungen = fokus && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Space));
        if !fokus || self.vorschlag.as_ref().is_some_and(|v| v.datei != datei) {
            self.vorschlag = None;
        }
        if let Some(v) = self.vorschlag.as_mut() {
            let n = v.eintraege.len();
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    v.auswahl = (v.auswahl + 1) % n;
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    v.auswahl = (v.auswahl + n - 1) % n;
                }
                if i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Enter) {
                    uebernehmen = true;
                }
            });
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.vorschlag = None;
            }
        }
        let vorschlag = &mut self.vorschlag;
        let mut gewaehlt: Option<usize> = None;

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
                    let python = datei.ends_with(".py");
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, breite: f32| {
                        luau_farben::layout(ui, text.as_str(), breite, fehler_zeile, python)
                    };
                    let ausgabe = egui::TextEdit::multiline(&mut puffer.text)
                        .id(id)
                        .code_editor()
                        .margin(egui::Margin::ZERO)
                        .desired_width(f32::INFINITY)
                        .desired_rows(40)
                        .frame(egui::Frame::NONE)
                        .layouter(&mut layouter)
                        .show(ui);
                    geaendert = ausgabe.response.changed();
                    let Some(cursor) = ausgabe.cursor_range.map(|c| c.primary) else {
                        *vorschlag = None;
                        return;
                    };

                    // Übernehmen (Tab/Enter)
                    if uebernehmen {
                        if let Some(v) = vorschlag.take() {
                            let neu = vervollstaendigung::einsetzen(
                                &mut puffer.text,
                                v.anfang,
                                cursor.index.0,
                                &v.eintraege[v.auswahl],
                            );
                            let mut zustand = ausgabe.state.clone();
                            zustand
                                .cursor
                                .set_char_range(Some(egui::text_selection::CCursorRange::one(
                                    egui::text::CCursor::new(neu),
                                )));
                            zustand.store(ui.ctx(), id);
                            geaendert = true;
                            return;
                        }
                    }

                    // Liste neu berechnen, wenn getippt wurde; schließen, wenn der Cursor woandershin springt
                    if geaendert || erzwungen {
                        *vorschlag = vervollstaendigung::vorschlaege(
                            &puffer.text,
                            cursor.index.0,
                            erzwungen,
                            Sprache::aus_datei(datei).unwrap_or(Sprache::Luau),
                            &projektnamen,
                        )
                        .map(|(anfang, eintraege)| vervollstaendigung::Offen {
                            datei: datei.to_string(),
                            anfang,
                            eintraege,
                            auswahl: 0,
                        });
                    } else if let Some(v) = vorschlag.as_ref() {
                        let im_text = v.eintraege.first().is_some_and(|e| e.text);
                        let passt = cursor.index.0 >= v.anfang
                            && puffer
                                .text
                                .chars()
                                .skip(v.anfang)
                                .take(cursor.index.0 - v.anfang)
                                .all(|c| {
                                    if im_text {
                                        !matches!(c, '"' | '\'' | '\n')
                                    } else {
                                        c.is_alphanumeric() || c == '_'
                                    }
                                });
                        if !passt {
                            *vorschlag = None;
                        }
                    }

                    // Liste unter dem Cursor zeichnen
                    if let Some(v) = vorschlag.as_ref() {
                        let stelle = ausgabe
                            .galley
                            .pos_from_cursor(egui::text::CCursor::new(v.anfang))
                            .translate(ausgabe.galley_pos.to_vec2());
                        egui::Area::new(egui::Id::new(("vorschlaege", datei)))
                            .fixed_pos(stelle.left_bottom() + egui::vec2(0.0, 2.0))
                            .order(egui::Order::Foreground)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    ui.set_max_width(520.0);
                                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                        for (i, e) in v.eintraege.iter().enumerate() {
                                            let text = RichText::new(match e.klammern {
                                                Some(_) => format!("{}()", e.name),
                                                None if e.text => format!("\"{}\"", e.name),
                                                None => e.name.clone(),
                                            })
                                            .monospace();
                                            let zeile = ui.horizontal(|ui| {
                                                let r = ui.selectable_label(i == v.auswahl, text);
                                                ui.label(RichText::new(&e.info).weak().small());
                                                r
                                            });
                                            if i == v.auswahl {
                                                zeile.inner.scroll_to_me(None);
                                            }
                                            if zeile.inner.clicked() {
                                                gewaehlt = Some(i);
                                            }
                                        }
                                    });
                                    ui.label(
                                        RichText::new("Pfeiltasten wählen · Tab/Enter übernehmen · Esc schließen")
                                            .weak()
                                            .small(),
                                    );
                                });
                            });
                    } else if ui.ctx().memory(|m| m.has_focus(id)) {
                        // Keine Liste offen: Signatur der Funktion zeigen, in deren Klammern der Cursor steht
                        if let Some((info, argument)) = vervollstaendigung::hinweis(
                            &puffer.text,
                            cursor.index.0,
                            Sprache::aus_datei(datei).unwrap_or(Sprache::Luau),
                        ) {
                            let stelle = ausgabe
                                .galley
                                .pos_from_cursor(cursor)
                                .translate(ausgabe.galley_pos.to_vec2());
                            egui::Area::new(egui::Id::new(("signatur", datei)))
                                .fixed_pos(stelle.left_top() - egui::vec2(0.0, 26.0))
                                .order(egui::Order::Foreground)
                                .interactable(false)
                                .show(ui.ctx(), |ui| {
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(info).monospace().small());
                                            ui.label(
                                                RichText::new(format!("· Argument {}", argument + 1)).weak().small(),
                                            );
                                        });
                                    });
                                });
                        }
                    }
                });
            });
        });
        // Mit der Maus gewählt: wie Tab, aber erst im nächsten Frame (Cursor-Stand von dort)
        if let Some(i) = gewaehlt {
            if let (Some(v), Some(p)) = (self.vorschlag.take(), self.puffer.get_mut(datei)) {
                let zustand = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
                let cursor = zustand
                    .cursor
                    .char_range()
                    .map_or(p.text.chars().count(), |c| c.primary.index.0);
                let neu = vervollstaendigung::einsetzen(&mut p.text, v.anfang, cursor, &v.eintraege[i]);
                let mut zustand = zustand;
                zustand
                    .cursor
                    .set_char_range(Some(egui::text_selection::CCursorRange::one(egui::text::CCursor::new(
                        neu,
                    ))));
                zustand.store(ui.ctx(), id);
                ui.ctx().memory_mut(|m| m.request_focus(id));
                geaendert = true;
            }
        }
        if geaendert {
            self.fehler_zeilen.remove(datei);
        }
    }

    /// Lage in der Welt, die 4 Ecken des Bildes (gedreht) und der Dreh-Griff über der Oberkante.
    fn rahmen(&self, o: Objekt) -> Option<(Transform, [Vec2; 4], Vec2)> {
        let t = welt::welt_transform(&self.welt, o)?;
        let b = self.welt.get::<&Bild>(o).ok()?;
        let halb = (b.groesse * t.skala).abs() * 0.5;
        let drehen = Vec2::from_angle(t.drehung);
        let ecke = |x: f32, y: f32| t.position + drehen.rotate(Vec2::new(x, y));
        let ecken = [
            ecke(-halb.x, -halb.y),
            ecke(halb.x, -halb.y),
            ecke(halb.x, halb.y),
            ecke(-halb.x, halb.y),
        ];
        let dreh = ecke(0.0, halb.y + 28.0 / self.kamera.zoom);
        Some((t, ecken, dreh))
    }

    fn gitter_zeichnen(
        &self,
        painter: &egui::Painter,
        groesse: Vec2,
        zu_punkt: &dyn Fn(Vec2) -> egui::Pos2,
        rect: egui::Rect,
    ) {
        let g = self.gitter_groesse.max(1.0);
        let ppp = groesse.x / rect.width().max(1.0);
        if g * self.kamera.zoom / ppp < 6.0 {
            return; // zu eng – nur Rauschen
        }
        let lo = self.kamera.bildschirm_zu_welt(Vec2::ZERO, groesse);
        let ru = self.kamera.bildschirm_zu_welt(groesse, groesse);
        let linie = |achse: bool| Stroke::new(1.0, Color32::from_white_alpha(if achse { 45 } else { 14 }));
        let mut x = (lo.x / g).floor() * g;
        while x <= ru.x {
            let px = zu_punkt(Vec2::new(x, 0.0)).x;
            painter.line_segment(
                [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
                linie(x.abs() < g * 0.5),
            );
            x += g;
        }
        let mut y = (ru.y / g).floor() * g;
        while y <= lo.y {
            let py = zu_punkt(Vec2::new(0.0, y)).y;
            painter.line_segment(
                [egui::pos2(rect.left(), py), egui::pos2(rect.right(), py)],
                linie(y.abs() < g * 0.5),
            );
            y += g;
        }
    }

    /// Bilder und Sounds des Projekts. Neue kommen per Drag & Drop ins Fenster.
    fn assets_ui(&mut self, ui: &mut egui::Ui) {
        let bearbeiten = self.modus == Modus::Bearbeiten;
        let mut bilder: Vec<(String, TexturId)> = self
            .assets
            .textur_namen()
            .iter()
            .map(|(n, t)| (n.clone(), *t))
            .collect();
        bilder.sort_by(|a, b| natuerlich(&a.0, &b.0));
        egui::CollapsingHeader::new(format!("🖼 Bilder ({})", bilder.len()))
            .id_salt("assets_bilder")
            .show(ui, |ui| {
                ui.label(RichText::new("PNG ins Fenster ziehen = neues Bild").weak().small());
                egui::ScrollArea::vertical()
                    .id_salt("bilder_liste")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for (name, textur) in &bilder {
                            let (b, h) = self.assets.pixel_groesse(*textur).unwrap_or((0, 0));
                            let knopf = ui
                                .add_enabled(bearbeiten, egui::Button::new(format!("🖼 {name}")).small())
                                .on_hover_text(format!(
                                    "{b} × {h} Pixel – klicken: in die Szene setzen\nIm Script: timbre.set_sprite(objekt, \"{name}\", breite, höhe)"
                                ));
                            if knopf.clicked() {
                                let g = self.start_groesse(*textur);
                                self.objekt_anlegen(name, name, g);
                            }
                        }
                    });
            });
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut sounds: Vec<String> = self.audio.namen().cloned().collect();
            sounds.sort_by(|a, b| natuerlich(a, b));
            egui::CollapsingHeader::new(format!("🔊 Sounds ({})", sounds.len()))
                .id_salt("assets_sounds")
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("WAV / OGG / MP3 / FLAC ins Fenster ziehen")
                            .weak()
                            .small(),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("sounds_liste")
                        .max_height(140.0)
                        .show(ui, |ui| {
                            for name in &sounds {
                                ui.horizontal(|ui| {
                                    if ui.small_button("▶").on_hover_text("Anhören").clicked() {
                                        let _ = self.audio.abspielen(name, 1.0, false);
                                    }
                                    ui.label(name)
                                        .on_hover_text(format!("Im Script: timbre.play_sound(\"{name}\")"));
                                });
                            }
                            if !sounds.is_empty() && ui.small_button("■ Alle stoppen").clicked() {
                                self.audio.stoppen("");
                            }
                        });
                });
        }
    }

    fn szene_ui(&mut self, ui: &mut egui::Ui) {
        if self.ansicht3d {
            self.szene3d_ui(ui);
            return;
        }
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

        self.maus_ueber_szene = antwort.hovered();
        let painter = ui.painter().with_clip_rect(rect);
        let kamera = self.kamera;
        let zu_punkt = |w: Vec2| {
            let p = kamera.welt_zu_bildschirm(w, groesse) / ppp;
            egui::pos2(rect.min.x + p.x, rect.min.y + p.y)
        };
        let gelb = Color32::from_rgb(0xF2, 0xC1, 0x4E);
        let bearbeiten = self.modus == Modus::Bearbeiten;
        let frei = ui.input(|i| i.modifiers.alt); // Alt = ohne Einrasten
        let einrasten = self.gitter && !frei;

        // Gitter
        if self.gitter && bearbeiten {
            self.gitter_zeichnen(&painter, groesse, &zu_punkt, rect);
        }

        // Leere Objekte (ohne Bild) als kleine Eis-Raute zeigen, damit man sie findet und anklicken kann
        if bearbeiten {
            let leere: Vec<(Objekt, Vec2)> = self
                .welt
                .query::<(Objekt, &Transform)>()
                .without::<&Bild>()
                .without::<&Instanz>()
                .without::<&Raum>()
                .iter()
                .filter_map(|(o, _)| welt::welt_transform(&self.welt, o).map(|t| (o, t.position)))
                .collect();
            for (o, p) in leere {
                let m = zu_punkt(p);
                let r = 6.0;
                let ausgewaehlt = self.auswahl == Some(o);
                let farbe = if ausgewaehlt {
                    eis::EIS
                } else {
                    eis::AKZENT.gamma_multiply(0.8)
                };
                let punkte = vec![
                    m + egui::vec2(0.0, -r),
                    m + egui::vec2(r, 0.0),
                    m + egui::vec2(0.0, r),
                    m + egui::vec2(-r, 0.0),
                ];
                painter.add(egui::Shape::convex_polygon(
                    punkte,
                    eis::GLETSCHER.gamma_multiply(0.35),
                    Stroke::new(1.5, farbe),
                ));
                painter.text(
                    m + egui::vec2(9.0, -9.0),
                    egui::Align2::LEFT_BOTTOM,
                    self.name(o),
                    egui::FontId::proportional(11.0),
                    farbe,
                );
            }
        }

        // Griffe des ausgewählten Objekts: Ecken = skalieren, Kreis oben = drehen
        let rahmen = self.auswahl.and_then(|o| self.rahmen(o));
        let zeiger = ui.input(|i| i.pointer.latest_pos());
        let griff_unter_maus = match (bearbeiten, &rahmen, zeiger) {
            (true, Some((_, ecken, dreh)), Some(z)) => {
                if zu_punkt(*dreh).distance(z) < 9.0 {
                    Some(GriffArt::Drehen)
                } else if ecken.iter().any(|e| zu_punkt(*e).distance(z) < 8.0) {
                    Some(GriffArt::Skalieren)
                } else {
                    None
                }
            }
            _ => None,
        };
        match griff_unter_maus.or(self.griff.map(|g| g.art)) {
            Some(GriffArt::Drehen) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
            Some(GriffArt::Skalieren) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe),
            None => {}
        }

        // Auswählen, Verschieben, Drehen, Skalieren mit der linken Maustaste (nur im Bearbeiten-Modus)
        if bearbeiten {
            let (gedrueckt, unten) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down()));
            if gedrueckt && antwort.hovered() {
                match (griff_unter_maus, &rahmen) {
                    (Some(art), Some((t, _, _))) => {
                        self.griff = Some(Griff {
                            art,
                            start_maus: self.maus_welt,
                            start: *t,
                        });
                        self.ziehen = None;
                    }
                    _ => {
                        self.auswahl = self.treffer(self.maus_welt);
                        self.ziehen = self
                            .auswahl
                            .and_then(|o| welt::welt_transform(&self.welt, o).map(|t| t.position - self.maus_welt));
                    }
                }
            }
            if !unten {
                self.ziehen = None;
                self.griff = None;
            }
            if let (Some(o), Some(versatz)) = (self.auswahl, self.ziehen) {
                if let Some(mut t) = welt::welt_transform(&self.welt, o) {
                    let mut ziel = self.maus_welt + versatz;
                    if einrasten {
                        ziel = (ziel / self.gitter_groesse).round() * self.gitter_groesse;
                    }
                    if (t.position - ziel).length_squared() > 1e-6 {
                        t.position = ziel;
                        welt::welt_transform_setzen(&mut self.welt, o, t);
                        self.markieren();
                    }
                }
            }
            if let (Some(o), Some(g)) = (self.auswahl, self.griff) {
                let mut t = g.start;
                match g.art {
                    GriffArt::Drehen => {
                        let a0 = (g.start_maus - t.position).to_angle();
                        let a1 = (self.maus_welt - t.position).to_angle();
                        let mut d = g.start.drehung + (a1 - a0);
                        if einrasten {
                            let schritt = 15f32.to_radians();
                            d = (d / schritt).round() * schritt;
                        }
                        t.drehung = d.rem_euclid(std::f32::consts::TAU);
                    }
                    GriffArt::Skalieren => {
                        let d0 = (g.start_maus - t.position).length().max(1e-3);
                        let faktor = (self.maus_welt - t.position).length() / d0;
                        let mut s = g.start.skala * faktor;
                        if einrasten {
                            s = (s * 20.0).round() / 20.0; // 0,05er-Schritte
                        }
                        // nie ganz auf 0 (sonst ist das Objekt unsichtbar und nicht mehr greifbar)
                        s = Vec2::new(
                            if s.x.abs() < 0.05 {
                                0.05f32.copysign(g.start.skala.x)
                            } else {
                                s.x
                            },
                            if s.y.abs() < 0.05 {
                                0.05f32.copysign(g.start.skala.y)
                            } else {
                                s.y
                            },
                        );
                        t.skala = s;
                    }
                }
                if welt::welt_transform(&self.welt, o) != Some(t) {
                    welt::welt_transform_setzen(&mut self.welt, o, t);
                    self.markieren();
                }
            }
        }

        // Rahmen, Griffe und Kollisionsform des ausgewählten Objekts
        if let Some(o) = self.auswahl {
            if let Some((t, ecken, dreh)) = self.rahmen(o) {
                let punkte: Vec<egui::Pos2> = ecken.iter().map(|e| zu_punkt(*e)).collect();
                painter.add(egui::Shape::closed_line(punkte.clone(), Stroke::new(1.5, gelb)));
                if bearbeiten {
                    for p in &punkte {
                        painter.rect_filled(egui::Rect::from_center_size(*p, egui::vec2(8.0, 8.0)), 1.0, gelb);
                    }
                    let kante = egui::pos2((punkte[2].x + punkte[3].x) * 0.5, (punkte[2].y + punkte[3].y) * 0.5);
                    painter.line_segment([kante, zu_punkt(dreh)], Stroke::new(1.0, gelb));
                    painter.circle_filled(zu_punkt(dreh), 5.5, gelb);
                }
                if let (Ok(k), Ok(b)) = (
                    self.welt.get::<&Kollision>(o).map(|k| *k),
                    self.welt.get::<&Bild>(o).map(|b| *b),
                ) {
                    let gruen = Color32::from_rgb(0x7C, 0xC4, 0x6A);
                    let groesse_welt = (b.groesse * t.skala).abs();
                    let strich = Stroke::new(1.2, gruen);
                    match k.form {
                        Form::Kreis => {
                            painter.circle_stroke(
                                zu_punkt(t.position),
                                groesse_welt.min_element() * 0.5 * kamera.zoom / ppp,
                                strich,
                            );
                        }
                        Form::Rechteck => {
                            let a = zu_punkt(t.position - groesse_welt * 0.5);
                            let b = zu_punkt(t.position + groesse_welt * 0.5);
                            painter.rect_stroke(egui::Rect::from_two_pos(a, b), 0.0, strich, egui::StrokeKind::Middle);
                        }
                    }
                }
            }
        }

        // Texte der Scripts (Punkte, Leben …)
        #[cfg(not(target_arch = "wasm32"))]
        if self.modus == Modus::Laeuft {
            for t in self.texte.values() {
                let pos = rect.min + egui::vec2(t.x, t.y) / ppp;
                let schrift = egui::FontId::proportional(t.groesse);
                let [r, g, b, a] = t.farbe.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
                painter.text(
                    pos + egui::vec2(1.5, 1.5),
                    egui::Align2::LEFT_TOP,
                    &t.text,
                    schrift.clone(),
                    Color32::from_black_alpha(160),
                );
                painter.text(
                    pos,
                    egui::Align2::LEFT_TOP,
                    &t.text,
                    schrift,
                    Color32::from_rgba_unmultiplied(r, g, b, a),
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
            } else {
                for sprache in Sprache::ALLE {
                    if ui
                        .button(format!("📜 Script hinzufügen ({})", sprache.name()))
                        .clicked()
                    {
                        aktionen.push(Aktion::ScriptHinzufuegen(Some(o), sprache));
                    }
                }
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
fn motor_im_hintergrund(sprache: Sprache) -> Motor {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let motor = match sprache {
            Sprache::Luau => ScriptMotor::neu(LUAU_GAST),
            Sprache::Lua => ScriptMotor::neu(LUA_GAST),
            Sprache::Python => {
                // Die Standardbibliothek sieht Python als /lib/python312.zip (schreibgeschützt)
                let mut dateien = std::collections::BTreeMap::new();
                dateien.insert("lib/python312.zip".to_string(), std::sync::Arc::from(PYTHON_BIBLIOTHEK));
                ScriptMotor::mit_dateien(PYTHON_GAST, dateien)
            }
        };
        let _ = tx.send(motor.map_err(|e| format!("{}: {e}", sprache.name())));
    });
    Motor::Laedt(rx)
}

/// Properties, die man Objekten geben kann (wie Komponenten in Unity).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Property {
    Bild,
    HitBox,
    Masse,
    Bewegung,
    Scripts,
    Raum,
    Mesh,
}

impl Property {
    const ALLE: [Property; 7] = [
        Property::Bild,
        Property::HitBox,
        Property::Masse,
        Property::Bewegung,
        Property::Scripts,
        Property::Raum,
        Property::Mesh,
    ];

    fn name(self) -> &'static str {
        match self {
            Property::Bild => "Bild",
            Property::HitBox => "HitBox",
            Property::Masse => "Masse",
            Property::Bewegung => "Bewegung",
            Property::Scripts => "Scripts",
            Property::Raum => "Transform 3D",
            Property::Mesh => "Mesh (3D)",
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            Property::Bild => "🖼",
            Property::HitBox => "⬚",
            Property::Masse => "⚖",
            Property::Bewegung => "➜",
            Property::Scripts => "📜",
            Property::Raum => "📐",
            Property::Mesh => "🔷",
        }
    }

    fn beschreibung(self) -> &'static str {
        match self {
            Property::Bild => "Zeigt eine Textur (Sprite)",
            Property::HitBox => "Stößt mit anderen zusammen und meldet Berührungen (on_touch)",
            Property::Masse => "Wie schwer das Objekt bei Zusammenstößen ist",
            Property::Bewegung => "Bewegt sich beim Ausführen von selbst",
            Property::Scripts => "Scripts, die für dieses Objekt laufen (script.object)",
            Property::Raum => "Position, Drehung und Skala im 3D-Raum",
            Property::Mesh => "Zeigt einen 3D-Körper (Würfel, Kugel, Ebene, Zylinder) in der 3D-Ansicht",
        }
    }
}

/// Ein Property-Block in den Eigenschaften: Kopfzeile mit Symbol, Name und 🗙, darunter der Inhalt.
/// Liefert `true`, wenn 🗙 (entfernen) geklickt wurde.
fn property_block(ui: &mut egui::Ui, p: Property, inhalt: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut weg = false;
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(eis::BLOCK)
        .stroke(Stroke::new(1.0, eis::RAND))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} {}", p.symbol(), p.name()))
                        .strong()
                        .color(eis::EIS),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("🗙")
                        .on_hover_text(format!("{} entfernen", p.name()))
                        .clicked()
                    {
                        weg = true;
                    }
                });
            });
            inhalt(ui);
        });
    weg
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
