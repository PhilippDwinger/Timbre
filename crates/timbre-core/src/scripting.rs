//! Scripting-Host: führt Scripts aus, die als WebAssembly-Module vorliegen.
//!
//! Jede Script-Sprache ist ein **Gast-Modul** (z. B. `assets/scripting/luau.wasm` – der
//! Luau-Interpreter, übersetzt nach WebAssembly). Die Engine lädt das Modul mit
//! **wasmtime**, gibt ihm die Script-Quelltexte und ruft jeden Logik-Schritt `update` auf.
//! Scripts erreichen die Engine nur über die Funktionen in [`importe_registrieren`]
//! (in Luau: `timbre.*`). Schnittstelle: `docs/scripting-abi.md`.
//!
//! Sicherheit: Ein Script kann die Engine nicht abstürzen lassen. Fehler werden gemeldet,
//! und ein „Treibstoff“-Zähler (fuel) bricht Endlosschleifen ab.

use std::collections::HashMap;

use glam::Vec2;
use wasmtime::{Caller, Config, Engine, Instance, Linker, Memory, Module, Store, Trap, TypedFunc};

use std::collections::BTreeMap;

use crate::input::{taste_aus_name, Input, MouseButton};
use crate::kamera::Kamera2d;
use crate::physik::{self, Beruehrungen, Paar};
use crate::renderer::TexturId;
use crate::welt::{
    eltern_setzen, entfernen, objekt_aus_nummer, objekt_nummer, welt_transform, welt_transform_setzen, Bewegung, Bild,
    Eltern, Form, Kollision, Mesh3d, Name, Objekt, Raum, Transform, Welt,
};

/// Wie viele WebAssembly-Anweisungen ein Aufruf höchstens ausführen darf.
const TREIBSTOFF_LADEN: u64 = 2_000_000_000;
const TREIBSTOFF_UPDATE: u64 = 500_000_000;

/// Eine Zeile für die Konsole.
#[derive(Clone, Debug, PartialEq)]
pub enum Meldung {
    Info(String),
    Fehler(String),
}

/// Alles, was Scripts während eines Aufrufs sehen und verändern dürfen.
#[derive(Default)]
pub struct Kontext {
    pub welt: Welt,
    /// Texturname (aus der .tbr-Datei) → geladene Textur
    pub texturen: HashMap<String, TexturId>,
    pub input: Input,
    /// Mausposition in Weltkoordinaten
    pub maus_welt: Vec2,
    /// Sekunden seit dem Start (▶)
    pub zeit: f64,
    pub meldungen: Vec<Meldung>,
    /// Von einem Script gewünschte Kamera
    pub kamera: Option<Kamera2d>,
    /// Kollisionen: wer berührt wen (für `get_touching`, `on_touch`)
    pub beruehrungen: Beruehrungen,
    /// Texte auf dem Bildschirm (`ui_text`), nach Schlüssel
    pub texte: BTreeMap<String, BildschirmText>,
    /// Namen der Sounds im Projekt (ohne Endung) – für `play_sound`
    pub sound_namen: std::collections::HashSet<String>,
    /// Von Scripts gewünschte Sound-Befehle; die Engine spielt sie nach dem Aufruf ab
    pub sound_befehle: Vec<SoundBefehl>,
    /// `timbre.broadcast` dieser Laufzeit – für Scripts anderer Sprachen (siehe [`ScriptLaufzeit::nachricht_zustellen`])
    pub ausgehend: Vec<Nachricht>,
    /// Schreibgeschützte Dateien für den Gast (z. B. Python-Standardbibliothek) und offene Dateien
    pub dateisystem: crate::wasi::Dateisystem,
    /// Zustand für `random_get`
    pub zufall: u64,
}

/// Eine Nachricht (`timbre.broadcast`) zwischen Script-Sprachen. Daten nur einfach – Tabellen werden zu nil.
#[derive(Clone, Debug, PartialEq)]
pub struct Nachricht {
    pub name: String,
    pub daten: Wert,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Wert {
    Nichts,
    Bool(bool),
    Zahl(f64),
    Ganzzahl(i64),
    Text(String),
}

/// Was ein Script mit Sounds machen will (`timbre.play_sound` / `timbre.stop_sounds`).
#[derive(Clone, Debug, PartialEq)]
pub enum SoundBefehl {
    Abspielen {
        name: String,
        lautstaerke: f32,
        schleife: bool,
    },
    /// Leerer Name = alle Sounds
    Stoppen(String),
}

/// Ein Text über dem Spiel (Punkte, Leben …). Position in Pixeln ab links oben im Spielbereich.
#[derive(Clone, Debug, PartialEq)]
pub struct BildschirmText {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub groesse: f32,
    pub farbe: [f32; 4],
}

/// Das übersetzte Gast-Modul. Das Übersetzen dauert etwas, darum nur einmal pro Sprache.
pub struct ScriptMotor {
    engine: Engine,
    modul: Module,
    /// Dateien, die der Gast lesen darf (Pfad ohne führenden `/` → Inhalt)
    dateien: std::sync::Arc<BTreeMap<String, std::sync::Arc<[u8]>>>,
}

impl ScriptMotor {
    pub fn neu(wasm: &[u8]) -> Result<Self, String> {
        Self::mit_dateien(wasm, BTreeMap::new())
    }

    /// Wie [`Self::neu`], dazu schreibgeschützte Dateien für den Gast – z. B.
    /// `"lib/python312.zip"` für Python (der Gast sieht sie als `/lib/python312.zip`).
    pub fn mit_dateien(wasm: &[u8], dateien: BTreeMap<String, std::sync::Arc<[u8]>>) -> Result<Self, String> {
        let mut cfg = Config::new();
        cfg.wasm_exceptions(true); // Luau meldet Fehler per WebAssembly-Exception
        cfg.consume_fuel(true); // Endlosschleifen abbrechen können
        let engine = Engine::new(&cfg).map_err(|e| format!("wasmtime: {e}"))?;
        let modul = modul_mit_zwischenspeicher(&engine, wasm)?;
        Ok(Self {
            engine,
            modul,
            dateien: std::sync::Arc::new(dateien),
        })
    }

    /// Frische Laufzeit für einen Durchlauf (▶). Jeder Start beginnt bei null.
    pub fn starten(&self) -> Result<ScriptLaufzeit, String> {
        let mut linker = Linker::new(&self.engine);
        importe_registrieren(&mut linker).map_err(|e| e.to_string())?;
        let kontext = Kontext {
            dateisystem: crate::wasi::Dateisystem::neu(self.dateien.clone()),
            zufall: web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E37_79B9_7F4A_7C15)
                | 1,
            ..Default::default()
        };
        let mut store = Store::new(&self.engine, kontext);
        store.set_fuel(TREIBSTOFF_LADEN).ok();
        let instanz = linker
            .instantiate(&mut store, &self.modul)
            .map_err(|e| format!("Gast starten: {e}"))?;
        if let Ok(init) = instanz.get_typed_func::<(), ()>(&mut store, "_initialize") {
            init.call(&mut store, ())
                .map_err(|e| format!("Gast initialisieren: {e}"))?;
        }
        let f = |name: &str| format!("Gast-Modul hat keine Funktion '{name}'");
        Ok(ScriptLaufzeit {
            alloc: instanz
                .get_typed_func(&mut store, "timbre_alloc")
                .map_err(|_| f("timbre_alloc"))?,
            free: instanz
                .get_typed_func(&mut store, "timbre_free")
                .map_err(|_| f("timbre_free"))?,
            laden: instanz
                .get_typed_func(&mut store, "timbre_laden")
                .map_err(|_| f("timbre_laden"))?,
            neu_laden: instanz.get_typed_func(&mut store, "timbre_neu_laden").ok(),
            nachricht: instanz.get_typed_func(&mut store, "timbre_nachricht").ok(),
            stoppen: instanz
                .get_typed_func(&mut store, "timbre_script_stoppen")
                .map_err(|_| f("timbre_script_stoppen"))?,
            beruehrung: instanz
                .get_typed_func(&mut store, "timbre_beruehrung")
                .map_err(|_| f("timbre_beruehrung"))?,
            geladen: Vec::new(),
            start: instanz
                .get_typed_func(&mut store, "timbre_start")
                .map_err(|_| f("timbre_start"))?,
            update: instanz
                .get_typed_func(&mut store, "timbre_update")
                .map_err(|_| f("timbre_update"))?,
            speicher: instanz
                .get_memory(&mut store, "memory")
                .ok_or("Gast-Modul hat keinen Speicher")?,
            _instanz: instanz,
            store,
            abgestuerzt: false,
        })
    }
}

/// Gast-Modul übersetzen – oder das Ergebnis vom letzten Mal nehmen.
///
/// Das Übersetzen großer Gäste (Python: ~11 MB) dauert einige Sekunden. Darum landet das
/// Ergebnis im Temp-Ordner (`%TEMP%\timbre-cache`). Der Dateiname enthält eine Prüfsumme des
/// Moduls und der wasmtime-Einstellungen; passt etwas nicht mehr, wird einfach neu übersetzt.
fn modul_mit_zwischenspeicher(engine: &Engine, wasm: &[u8]) -> Result<Module, String> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    wasm.hash(&mut h);
    engine.precompile_compatibility_hash().hash(&mut h);
    let ordner = std::env::temp_dir().join("timbre-cache");
    let datei = ordner.join(format!("{:016x}.cwasm", h.finish()));
    if datei.exists() {
        // SAFETY: Die Datei hat Timbre selbst mit derselben Engine-Einstellung geschrieben (siehe Prüfsumme).
        if let Ok(m) = unsafe { Module::deserialize_file(engine, &datei) } {
            return Ok(m);
        }
    }
    let modul = Module::new(engine, wasm).map_err(|e| format!("Gast-Modul ungültig: {e}"))?;
    if let Ok(bytes) = modul.serialize() {
        let _ = std::fs::create_dir_all(&ordner);
        let temp = datei.with_extension("tmp");
        if std::fs::write(&temp, bytes).is_ok() {
            let _ = std::fs::rename(&temp, &datei);
        }
    }
    Ok(modul)
}

/// Eine laufende Script-Umgebung (von ▶ bis ■).
pub struct ScriptLaufzeit {
    store: Store<Kontext>,
    _instanz: Instance,
    speicher: Memory,
    alloc: TypedFunc<i32, i32>,
    free: TypedFunc<i32, ()>,
    laden: TypedFunc<(i32, i32, i32, i32, i64, i64), i32>,
    /// Fehlt in älteren Gast-Modulen – dann geht kein Live-Neuladen
    neu_laden: Option<TypedFunc<(i32, i32, i32, i32, i64, i64), i32>>,
    stoppen: TypedFunc<i64, ()>,
    /// Fehlt in älteren Gast-Modulen – dann keine Nachrichten aus anderen Sprachen
    nachricht: Option<TypedFunc<(i32, i32, i32, f64, i32, i32), ()>>,
    beruehrung: TypedFunc<(i64, i64), ()>,
    /// Script-Objekte, deren Script läuft – verschwindet eins, wird sein Script angehalten.
    geladen: Vec<Objekt>,
    start: TypedFunc<(), ()>,
    update: TypedFunc<f32, ()>,
    /// Nach einem schweren Fehler (z. B. Endlosschleife) ist der Gast unbrauchbar.
    abgestuerzt: bool,
}

impl ScriptLaufzeit {
    /// Gibt dem Kontext die Welt, ruft `f` auf und holt die Welt zurück.
    /// So können Scripts während des Aufrufs auf die Welt zugreifen – ohne gemeinsame Zeiger.
    fn mit_welt<T>(
        &mut self,
        welt: &mut Welt,
        treibstoff: u64,
        f: impl FnOnce(&mut Self) -> wasmtime::Result<T>,
    ) -> Option<T> {
        if self.abgestuerzt {
            return None;
        }
        std::mem::swap(welt, &mut self.store.data_mut().welt);
        self.store.set_fuel(treibstoff).ok();
        let ergebnis = f(self);
        std::mem::swap(welt, &mut self.store.data_mut().welt);
        match ergebnis {
            Ok(wert) => Some(wert),
            Err(e) => {
                let text = match e.downcast_ref::<Trap>() {
                    Some(Trap::OutOfFuel) => {
                        "Script hat zu lange gebraucht und wurde abgebrochen – Endlosschleife?".to_string()
                    }
                    _ => format!("Script-Laufzeit abgestürzt: {e}"),
                };
                self.store.data_mut().meldungen.push(Meldung::Fehler(text));
                self.abgestuerzt = true;
                None
            }
        }
    }

    fn text_ablegen(&mut self, text: &str) -> wasmtime::Result<i32> {
        let p = self.alloc.call(&mut self.store, text.len() as i32)?;
        self.speicher.write(&mut self.store, p as usize, text.as_bytes())?;
        Ok(p)
    }

    /// Ein Script laden und seinen Hauptteil ausführen. `false` bei Fehler (steht dann in den Meldungen).
    ///
    /// `script` = das Script-Objekt (für `script.id`), `eltern` = Objekt, unter dem es hängt
    /// (für `script.parent`). Beides darf fehlen.
    pub fn script_laden(
        &mut self,
        welt: &mut Welt,
        name: &str,
        quelle: &str,
        script: Option<Objekt>,
        eltern: Option<Objekt>,
    ) -> bool {
        let id = script.map(|o| objekt_nummer(o) as i64).unwrap_or(0);
        let eltern_id = eltern.map(|o| objekt_nummer(o) as i64).unwrap_or(0);
        let ok = self
            .mit_welt(welt, TREIBSTOFF_LADEN, |s| {
                let n = s.text_ablegen(name)?;
                let q = s.text_ablegen(quelle)?;
                let r = s.laden.call(
                    &mut s.store,
                    (n, name.len() as i32, q, quelle.len() as i32, id, eltern_id),
                )?;
                s.free.call(&mut s.store, n)?;
                s.free.call(&mut s.store, q)?;
                Ok(r == 0)
            })
            .unwrap_or(false);
        if ok {
            self.geladen.extend(script);
        }
        ok
    }

    /// Während des Laufens eine neue Fassung eines Scripts übernehmen (Live-Neuladen).
    ///
    /// Die alte Fassung wird angehalten (auch ihre wartenden `task.wait`/`task.delay`), die neue
    /// geladen und – wenn das Spiel schon läuft – ihr `on_start` aufgerufen. Objekte, die die alte
    /// Fassung erzeugt hat, bleiben. Hat die neue Fassung einen Syntaxfehler, läuft die alte weiter.
    /// `Err`, wenn das Gast-Modul kein Neuladen kann; sonst `Ok(true)` bei Erfolg.
    pub fn script_neu_laden(
        &mut self,
        welt: &mut Welt,
        name: &str,
        quelle: &str,
        script: Objekt,
        eltern: Option<Objekt>,
    ) -> Result<bool, String> {
        let Some(neu_laden) = self.neu_laden.clone() else {
            return Err("Diese Script-Sprache kann (noch) nicht live neu laden".into());
        };
        let id = objekt_nummer(script) as i64;
        let eltern_id = eltern.map(|o| objekt_nummer(o) as i64).unwrap_or(0);
        let ok = self
            .mit_welt(welt, TREIBSTOFF_LADEN, |s| {
                let n = s.text_ablegen(name)?;
                let q = s.text_ablegen(quelle)?;
                let r = neu_laden.call(
                    &mut s.store,
                    (n, name.len() as i32, q, quelle.len() as i32, id, eltern_id),
                )?;
                s.free.call(&mut s.store, n)?;
                s.free.call(&mut s.store, q)?;
                Ok(r == 0)
            })
            .unwrap_or(false);
        if ok && !self.geladen.contains(&script) {
            self.geladen.push(script);
        }
        self.verwaiste_anhalten(welt);
        Ok(ok)
    }

    /// Nachricht aus einer anderen Sprache zustellen → `on_message(name, daten)` in allen Scripts.
    pub fn nachricht_zustellen(&mut self, welt: &mut Welt, n: &Nachricht) {
        let Some(f) = self.nachricht.clone() else { return };
        self.mit_welt(welt, TREIBSTOFF_UPDATE, |s| {
            let p = s.text_ablegen(&n.name)?;
            let (art, zahl, text) = match &n.daten {
                Wert::Nichts => (0, 0.0, ""),
                Wert::Bool(b) => (1, *b as i32 as f64, ""),
                Wert::Zahl(z) => (2, *z, ""),
                Wert::Ganzzahl(z) => (4, *z as f64, ""),
                Wert::Text(t) => (3, 0.0, t.as_str()),
            };
            let t = s.text_ablegen(text)?;
            f.call(&mut s.store, (p, n.name.len() as i32, art, zahl, t, text.len() as i32))?;
            s.free.call(&mut s.store, p)?;
            s.free.call(&mut s.store, t)?;
            Ok(())
        });
        self.verwaiste_anhalten(welt);
    }

    /// Scripts anhalten, deren Objekt nicht mehr existiert (z. B. mit dem Eltern-Objekt gelöscht).
    fn verwaiste_anhalten(&mut self, welt: &Welt) {
        let weg: Vec<Objekt> = self.geladen.iter().copied().filter(|o| !welt.contains(*o)).collect();
        if weg.is_empty() || self.abgestuerzt {
            return;
        }
        self.geladen.retain(|o| welt.contains(*o));
        for o in weg {
            let _ = self.stoppen.call(&mut self.store, objekt_nummer(o) as i64);
        }
    }

    /// `on_start()` aller Scripts.
    pub fn start(&mut self, welt: &mut Welt) {
        self.mit_welt(welt, TREIBSTOFF_LADEN, |s| s.start.call(&mut s.store, ()));
        self.verwaiste_anhalten(welt);
    }

    /// Ein Logik-Schritt: `on_update(dt)` aller Scripts.
    pub fn update(&mut self, welt: &mut Welt, dt: f32) {
        self.store.data_mut().zeit += dt as f64;
        self.mit_welt(welt, TREIBSTOFF_UPDATE, |s| s.update.call(&mut s.store, dt));
        self.verwaiste_anhalten(welt);
    }

    /// Kollisionen auflösen und neue Berührungen an die Scripts melden: Hängt ein Script unter
    /// einem der beiden Objekte, wird sein `on_touch(anderes)` aufgerufen.
    pub fn kollisionen(&mut self, welt: &mut Welt) {
        let paare = physik::schritt(welt);
        self.beruehrungen_melden(welt, &paare);
    }

    /// Wie [`Self::kollisionen`], aber die Physik ist schon gelaufen (`paare` = ihr Ergebnis).
    /// Für mehrere Laufzeiten (Sprachen) gleichzeitig: Physik einmal, melden an jede.
    pub fn beruehrungen_melden(&mut self, welt: &mut Welt, paare: &[Paar]) {
        let neu: Vec<Paar> = self.store.data_mut().beruehrungen.aktualisieren(paare);
        if neu.is_empty() || self.abgestuerzt {
            return;
        }
        // Welche geladenen Scripts hängen unter welchem Objekt?
        let mut aufrufe = Vec::new();
        for &s in &self.geladen {
            let Some(eltern) = welt.get::<&Eltern>(s).ok().map(|e| e.0) else {
                continue;
            };
            for &(a, b) in &neu {
                if eltern == a {
                    aufrufe.push((s, b));
                }
                if eltern == b {
                    aufrufe.push((s, a));
                }
            }
        }
        if aufrufe.is_empty() {
            return;
        }
        self.mit_welt(welt, TREIBSTOFF_UPDATE, |l| {
            for (s, anderes) in aufrufe {
                l.beruehrung
                    .call(&mut l.store, (objekt_nummer(s) as i64, objekt_nummer(anderes) as i64))?;
            }
            Ok(())
        });
        self.verwaiste_anhalten(welt);
    }

    /// Zugriff auf Eingabe, Texturen, Kamera usw. – vor dem Aufruf setzen, danach auslesen.
    pub fn kontext(&mut self) -> &mut Kontext {
        self.store.data_mut()
    }

    pub fn abgestuerzt(&self) -> bool {
        self.abgestuerzt
    }
}

// ---------------------------------------------------------------------------
// Die Funktionen, die Scripts aufrufen können (Modul "timbre")
// ---------------------------------------------------------------------------

fn speicher(c: &mut Caller<'_, Kontext>) -> wasmtime::Result<Memory> {
    c.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("kein Speicher"))
}

fn text(c: &mut Caller<'_, Kontext>, p: i32, n: i32) -> wasmtime::Result<String> {
    let m = speicher(c)?;
    let daten = m
        .data(&c)
        .get(p as usize..(p as usize).saturating_add(n as usize))
        .ok_or_else(|| wasmtime::Error::msg("Text außerhalb des Speichers"))?;
    Ok(String::from_utf8_lossy(daten).into_owned())
}

fn floats_schreiben(c: &mut Caller<'_, Kontext>, p: i32, werte: &[f32]) -> wasmtime::Result<()> {
    let bytes: Vec<u8> = werte.iter().flat_map(|w| w.to_le_bytes()).collect();
    speicher(c)?.write(c, p as usize, &bytes)?;
    Ok(())
}

fn objekt(id: i64) -> Option<crate::welt::Objekt> {
    objekt_aus_nummer(id as u64)
}

/// Lage eines Objekts in der Welt ändern (wird in die Lage relativ zum Eltern-Objekt umgerechnet).
fn welt_aendern(c: &mut Caller<'_, Kontext>, id: i64, f: impl FnOnce(&mut Transform)) {
    let Some(o) = objekt(id) else { return };
    let welt = &mut c.data_mut().welt;
    if let Some(mut t) = welt_transform(welt, o) {
        f(&mut t);
        welt_transform_setzen(welt, o, t);
    }
}

/// Komponente eines Objekts ändern (fehlende Objekte werden still ignoriert).
fn aendern<T: hecs::Component>(c: &mut Caller<'_, Kontext>, id: i64, f: impl FnOnce(&mut T)) {
    if let Some(o) = objekt(id) {
        if let Ok(mut k) = c.data_mut().welt.get::<&mut T>(o) {
            f(&mut k);
        }
    }
}

pub fn importe_registrieren(l: &mut Linker<Kontext>) -> wasmtime::Result<()> {
    const M: &str = "timbre";

    l.func_wrap(
        M,
        "log",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<()> {
            let t = text(&mut c, p, n)?;
            c.data_mut().meldungen.push(Meldung::Info(t));
            Ok(())
        },
    )?;
    l.func_wrap(
        M,
        "fehler",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<()> {
            let t = text(&mut c, p, n)?;
            c.data_mut().meldungen.push(Meldung::Fehler(t));
            Ok(())
        },
    )?;
    l.func_wrap(M, "zeit", |c: Caller<'_, Kontext>| -> f64 { c.data().zeit })?;

    l.func_wrap(
        M,
        "spawn",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<i64> {
            let name = text(&mut c, p, n)?;
            let o = c.data_mut().welt.spawn((Name(name), Transform::default()));
            Ok(objekt_nummer(o) as i64)
        },
    )?;
    l.func_wrap(
        M,
        "find",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<i64> {
            let name = text(&mut c, p, n)?;
            let welt = &c.data().welt;
            let mut q = welt.query::<(crate::welt::Objekt, &Name)>();
            Ok(q.iter()
                .find(|(_, nm)| nm.0 == name)
                .map(|(o, _)| objekt_nummer(o) as i64)
                .unwrap_or(0))
        },
    )?;
    l.func_wrap(M, "destroy", |mut c: Caller<'_, Kontext>, id: i64| {
        if let Some(o) = objekt(id) {
            entfernen(&mut c.data_mut().welt, o);
        }
    })?;

    // Position und Drehung: immer in der Welt (wie `Position` in Roblox), auch bei Kindern.
    l.func_wrap(
        M,
        "set_position",
        |mut c: Caller<'_, Kontext>, id: i64, x: f32, y: f32| {
            welt_aendern(&mut c, id, |t| t.position = Vec2::new(x, y));
        },
    )?;
    l.func_wrap(
        M,
        "get_position",
        |mut c: Caller<'_, Kontext>, id: i64, aus: i32| -> wasmtime::Result<i32> {
            match objekt(id).and_then(|o| welt_transform(&c.data().welt, o)) {
                Some(t) => {
                    floats_schreiben(&mut c, aus, &[t.position.x, t.position.y])?;
                    Ok(1)
                }
                None => Ok(0),
            }
        },
    )?;
    l.func_wrap(M, "set_rotation", |mut c: Caller<'_, Kontext>, id: i64, r: f32| {
        welt_aendern(&mut c, id, |t| t.drehung = r);
    })?;
    l.func_wrap(M, "get_rotation", |c: Caller<'_, Kontext>, id: i64| -> f32 {
        objekt(id)
            .and_then(|o| welt_transform(&c.data().welt, o))
            .map(|t| t.drehung)
            .unwrap_or(0.0)
    })?;
    l.func_wrap(M, "set_scale", |mut c: Caller<'_, Kontext>, id: i64, x: f32, y: f32| {
        aendern::<Transform>(&mut c, id, |t| t.skala = Vec2::new(x, y));
    })?;
    // 3D: `was` = 0 Position (Welt), 1 Drehung (Grad, eigene), 2 Skala (eigene)
    l.func_wrap(
        M,
        "set3d",
        |mut c: Caller<'_, Kontext>, id: i64, was: i32, x: f32, y: f32, z: f32| {
            let Some(o) = objekt(id) else { return };
            let welt = &mut c.data_mut().welt;
            let v = glam::Vec3::new(x, y, z);
            if was == 0 {
                crate::welt::welt_position3d_setzen(welt, o, v);
                return;
            }
            if welt.get::<&Raum>(o).is_err() {
                let _ = welt.insert_one(o, Raum::default());
            }
            if let Ok(mut r) = welt.get::<&mut Raum>(o) {
                match was {
                    1 => r.drehung = v,
                    2 => r.skala = v,
                    _ => {}
                }
            }
        },
    )?;
    l.func_wrap(
        M,
        "get3d",
        |mut c: Caller<'_, Kontext>, id: i64, was: i32, aus: i32| -> wasmtime::Result<i32> {
            let welt = &c.data().welt;
            let Some(o) = objekt(id).filter(|o| welt.get::<&Raum>(*o).is_ok()) else {
                return Ok(0);
            };
            let v = match was {
                0 => crate::welt::welt_raum(welt, o)
                    .map(|m| m.w_axis.truncate())
                    .unwrap_or_default(),
                1 => welt.get::<&Raum>(o).map(|r| r.drehung).unwrap_or_default(),
                _ => welt.get::<&Raum>(o).map(|r| r.skala).unwrap_or(glam::Vec3::ONE),
            };
            floats_schreiben(&mut c, aus, &v.to_array())?;
            Ok(1)
        },
    )?;
    l.func_wrap(
        M,
        "set_mesh",
        |mut c: Caller<'_, Kontext>, id: i64, p: i32, n: i32, r: f32, g: f32, b: f32| -> wasmtime::Result<()> {
            let form = text(&mut c, p, n)?;
            let Some(o) = objekt(id) else { return Ok(()) };
            let welt = &mut c.data_mut().welt;
            match crate::renderer3d::Form3d::aus_schluessel(&form) {
                Some(form) => {
                    if welt.get::<&Raum>(o).is_err() {
                        let _ = welt.insert_one(o, Raum::default());
                    }
                    let _ = welt.insert_one(
                        o,
                        Mesh3d {
                            form,
                            farbe: [r, g, b, 1.0],
                        },
                    );
                }
                None => {
                    let _ = welt.remove_one::<Mesh3d>(o);
                }
            }
            Ok(())
        },
    )?;
    l.func_wrap(
        M,
        "set_velocity",
        |mut c: Caller<'_, Kontext>, id: i64, vx: f32, vy: f32, dreh: f32| {
            if let Some(o) = objekt(id) {
                let _ = c.data_mut().welt.insert_one(
                    o,
                    Bewegung {
                        linear: Vec2::new(vx, vy),
                        drehung: dreh,
                    },
                );
            }
        },
    )?;
    l.func_wrap(
        M,
        "set_sprite",
        |mut c: Caller<'_, Kontext>, id: i64, p: i32, n: i32, b: f32, h: f32, ebene: i32| -> wasmtime::Result<i32> {
            let name = text(&mut c, p, n)?;
            let Some(&textur) = c.data().texturen.get(&name) else {
                return Ok(0);
            };
            if let Some(o) = objekt(id) {
                let farbe = c.data().welt.get::<&Bild>(o).map(|b| b.farbe).unwrap_or([1.0; 4]);
                let mut bild = Bild::new(textur, Vec2::new(b, h));
                bild.ebene = ebene;
                bild.farbe = farbe;
                let _ = c.data_mut().welt.insert_one(o, bild);
            }
            Ok(1)
        },
    )?;
    l.func_wrap(
        M,
        "set_color",
        |mut c: Caller<'_, Kontext>, id: i64, r: f32, g: f32, b: f32, a: f32| {
            aendern::<Bild>(&mut c, id, |bild| bild.farbe = [r, g, b, a]);
        },
    )?;

    l.func_wrap(
        M,
        "key_down",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<i32> {
            let name = text(&mut c, p, n)?;
            Ok(taste_aus_name(&name).is_some_and(|k| c.data().input.taste(k)) as i32)
        },
    )?;
    l.func_wrap(
        M,
        "key_pressed",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<i32> {
            let name = text(&mut c, p, n)?;
            Ok(taste_aus_name(&name).is_some_and(|k| c.data().input.taste_neu(k)) as i32)
        },
    )?;
    l.func_wrap(
        M,
        "mouse_position",
        |mut c: Caller<'_, Kontext>, aus: i32| -> wasmtime::Result<()> {
            let m = c.data().maus_welt;
            floats_schreiben(&mut c, aus, &[m.x, m.y])
        },
    )?;
    l.func_wrap(
        M,
        "set_camera",
        |mut c: Caller<'_, Kontext>, x: f32, y: f32, zoom: f32| {
            c.data_mut().kamera = Some(Kamera2d {
                position: Vec2::new(x, y),
                zoom: zoom.clamp(0.01, 100.0),
            });
        },
    )?;

    l.func_wrap(M, "set_parent", |mut c: Caller<'_, Kontext>, id: i64, eltern: i64| {
        if let Some(o) = objekt(id) {
            // Lage in der Welt bleibt gleich; Schleifen im Baum werden ignoriert
            eltern_setzen(&mut c.data_mut().welt, o, objekt(eltern));
        }
    })?;
    l.func_wrap(M, "get_parent", |c: Caller<'_, Kontext>, id: i64| -> i64 {
        objekt(id)
            .and_then(|o| c.data().welt.get::<&Eltern>(o).ok().map(|e| e.0))
            .map(|e| objekt_nummer(e) as i64)
            .unwrap_or(0)
    })?;

    l.func_wrap(
        M,
        "get_name",
        |mut c: Caller<'_, Kontext>, id: i64, aus: i32, platz: i32| -> wasmtime::Result<i32> {
            let Some(name) = objekt(id).and_then(|o| c.data().welt.get::<&Name>(o).ok().map(|n| n.0.clone())) else {
                return Ok(-1);
            };
            let bytes = name.as_bytes();
            if bytes.len() <= platz as usize {
                speicher(&mut c)?.write(&mut c, aus as usize, bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        M,
        "set_collision",
        |mut c: Caller<'_, Kontext>, id: i64, form: i32, fest: i32, verankert: i32| {
            let Some(o) = objekt(id) else { return };
            let welt = &mut c.data_mut().welt;
            let form = match form {
                1 => Form::Kreis,
                2 => Form::Rechteck,
                _ => {
                    let _ = welt.remove_one::<Kollision>(o);
                    return;
                }
            };
            let _ = welt.insert_one(
                o,
                Kollision {
                    form,
                    fest: fest != 0,
                    verankert: verankert != 0,
                },
            );
        },
    )?;
    l.func_wrap(
        M,
        "get_touching",
        |mut c: Caller<'_, Kontext>, id: i64, aus: i32, platz: i32| -> wasmtime::Result<i32> {
            let liste: Vec<Objekt> = objekt(id)
                .and_then(|o| c.data().beruehrungen.je_objekt.get(&o).cloned())
                .unwrap_or_default()
                .into_iter()
                .filter(|o| c.data().welt.contains(*o))
                .collect();
            let bytes: Vec<u8> = liste
                .iter()
                .take(platz.max(0) as usize)
                .flat_map(|o| (objekt_nummer(*o) as i64).to_le_bytes())
                .collect();
            speicher(&mut c)?.write(&mut c, aus as usize, &bytes)?;
            Ok(liste.len() as i32)
        },
    )?;
    let maustaste = |t: i32| match t {
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => MouseButton::Left,
    };
    l.func_wrap(M, "mouse_down", move |c: Caller<'_, Kontext>, t: i32| -> i32 {
        c.data().input.maustaste(maustaste(t)) as i32
    })?;
    l.func_wrap(M, "mouse_pressed", move |c: Caller<'_, Kontext>, t: i32| -> i32 {
        c.data().input.maustaste_neu(maustaste(t)) as i32
    })?;
    l.func_wrap(
        M,
        "ui_text",
        |mut c: Caller<'_, Kontext>,
         kp: i32,
         kn: i32,
         tp: i32,
         tn: i32,
         x: f32,
         y: f32,
         groesse: f32,
         r: f32,
         g: f32,
         b: f32,
         a: f32|
         -> wasmtime::Result<()> {
            let schluessel = text(&mut c, kp, kn)?;
            let inhalt = text(&mut c, tp, tn)?;
            c.data_mut().texte.insert(
                schluessel,
                BildschirmText {
                    text: inhalt,
                    x,
                    y,
                    groesse: groesse.clamp(4.0, 200.0),
                    farbe: [r, g, b, a],
                },
            );
            Ok(())
        },
    )?;
    l.func_wrap(
        M,
        "ui_clear",
        |mut c: Caller<'_, Kontext>, kp: i32, kn: i32| -> wasmtime::Result<()> {
            let schluessel = text(&mut c, kp, kn)?;
            if schluessel.is_empty() {
                c.data_mut().texte.clear();
            } else {
                c.data_mut().texte.remove(&schluessel);
            }
            Ok(())
        },
    )?;

    l.func_wrap(
        M,
        "play_sound",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32, lautstaerke: f32, schleife: i32| -> wasmtime::Result<i32> {
            let name = text(&mut c, p, n)?;
            let name = name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(name);
            if !c.data().sound_namen.contains(&name) {
                return Ok(0);
            }
            c.data_mut().sound_befehle.push(SoundBefehl::Abspielen {
                name,
                lautstaerke: lautstaerke.clamp(0.0, 4.0),
                schleife: schleife != 0,
            });
            Ok(1)
        },
    )?;
    l.func_wrap(
        M,
        "nachricht",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32, art: i32, zahl: f64, tp: i32, tn: i32| -> wasmtime::Result<()> {
            let name = text(&mut c, p, n)?;
            let daten = match art {
                1 => Wert::Bool(zahl != 0.0),
                2 => Wert::Zahl(zahl),
                3 => Wert::Text(text(&mut c, tp, tn)?),
                4 => Wert::Ganzzahl(zahl as i64),
                _ => Wert::Nichts,
            };
            let ausgehend = &mut c.data_mut().ausgehend;
            if ausgehend.len() < 10_000 {
                ausgehend.push(Nachricht { name, daten });
            }
            Ok(())
        },
    )?;
    l.func_wrap(
        M,
        "stop_sounds",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<()> {
            let name = text(&mut c, p, n)?;
            c.data_mut().sound_befehle.push(SoundBefehl::Stoppen(name));
            Ok(())
        },
    )?;

    // Systemfunktionen (WASI) der C-Bibliothek im Gast: nur ein schreibgeschütztes Dateisystem im Speicher
    crate::wasi::registrieren(l)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUAU: &[u8] = include_bytes!("../../../assets/scripting/luau.wasm");

    fn laufzeit() -> (ScriptLaufzeit, Welt) {
        let motor = ScriptMotor::neu(LUAU).unwrap();
        (motor.starten().unwrap(), Welt::new())
    }

    fn meldungen(l: &mut ScriptLaufzeit) -> Vec<Meldung> {
        std::mem::take(&mut l.kontext().meldungen)
    }

    #[test]
    fn script_erzeugt_und_bewegt_objekt() {
        let (mut l, mut welt) = laufzeit();
        let quelle = r#"
            local kiste
            function on_start()
                kiste = timbre.spawn("Kiste")
                print("erzeugt", kiste ~= nil)
            end
            function on_update(dt)
                local x, y = timbre.get_position(kiste)
                timbre.set_position(kiste, x + 10 * dt, y)
            end
        "#;
        assert!(l.script_laden(&mut welt, "test.luau", quelle, None, None));
        l.start(&mut welt);
        for _ in 0..60 {
            l.update(&mut welt, 1.0 / 60.0);
        }
        assert_eq!(meldungen(&mut l), vec![Meldung::Info("erzeugt true".into())]);
        let mut q = welt.query::<(&Name, &Transform)>();
        let (name, t) = q.iter().next().unwrap();
        assert_eq!(name.0, "Kiste");
        assert!((t.position.x - 10.0).abs() < 0.01, "x = {}", t.position.x);
    }

    #[test]
    fn fehler_werden_gemeldet_und_halten_nur_das_script_an() {
        let (mut l, mut welt) = laufzeit();
        assert!(!l.script_laden(&mut welt, "kaputt.luau", "das ist ((kein luau", None, None));
        assert!(l.script_laden(
            &mut welt,
            "fehler.luau",
            "function on_update() local t = nil; print(t.x) end",
            None,
            None
        ));
        assert!(l.script_laden(
            &mut welt,
            "gut.luau",
            "function on_update() print('gut') end",
            None,
            None
        ));
        l.update(&mut welt, 0.016);
        l.update(&mut welt, 0.016);
        let m = meldungen(&mut l);
        assert!(matches!(&m[0], Meldung::Fehler(t) if t.starts_with("kaputt.luau:1")));
        assert!(m
            .iter()
            .any(|x| matches!(x, Meldung::Fehler(t) if t.contains("fehler.luau:1") && t.contains("index nil"))));
        // "gut" läuft in beiden Schritten weiter, "fehler" nur einmal gemeldet
        assert_eq!(m.iter().filter(|x| **x == Meldung::Info("gut".into())).count(), 2);
        assert!(!l.abgestuerzt());
    }

    // --- Lua 5.4 (zweiter Gast, gleiche Schnittstelle) ---------------------------------

    const LUA: &[u8] = include_bytes!("../../../assets/scripting/lua.wasm");

    fn lua_laufzeit() -> (ScriptLaufzeit, Welt) {
        let motor = ScriptMotor::neu(LUA).unwrap();
        (motor.starten().unwrap(), Welt::new())
    }

    #[test]
    fn lua_script_bewegt_objekt_und_wartet() {
        let (mut l, mut welt) = lua_laufzeit();
        let quelle = r#"
            local kiste
            function on_start()
                kiste = timbre.spawn("Kiste")
                print("erzeugt", math.type(kiste))
                task.wait(0.05)
                print("gewartet")
            end
            function on_update(dt)
                local x, y = timbre.get_position(kiste)
                timbre.set_position(kiste, x + 10 * dt, y)
            end
        "#;
        assert!(l.script_laden(&mut welt, "test.lua", quelle, None, None));
        l.start(&mut welt);
        for _ in 0..60 {
            l.update(&mut welt, 1.0 / 60.0);
        }
        assert_eq!(
            meldungen(&mut l),
            vec![
                Meldung::Info("erzeugt integer".into()),
                Meldung::Info("gewartet".into())
            ]
        );
        let mut q = welt.query::<(&Name, &Transform)>();
        let (name, t) = q.iter().next().unwrap();
        assert_eq!(name.0, "Kiste");
        assert!((t.position.x - 10.0).abs() < 0.01, "x = {}", t.position.x);
    }

    #[test]
    fn lua_fehler_sandbox_und_endlosschleife() {
        let (mut l, mut welt) = lua_laufzeit();
        assert!(!l.script_laden(&mut welt, "kaputt.lua", "das ist ((kein lua", None, None));
        assert!(l.script_laden(
            &mut welt,
            "fehler.lua",
            "function on_update() local t = nil; print(t.x) end",
            None,
            None
        ));
        // Keine Dateien, kein Betriebssystem; eigene Umgebung pro Script
        assert!(l.script_laden(
            &mut welt,
            "sandbox.lua",
            "x = 1; print(io == nil, os == nil, require == nil, dofile == nil)",
            None,
            None
        ));
        assert!(l.script_laden(&mut welt, "andere.lua", "print(x == nil)", None, None));
        l.update(&mut welt, 0.016);
        let m = meldungen(&mut l);
        assert!(
            matches!(&m[0], Meldung::Fehler(t) if t.starts_with("kaputt.lua:1")),
            "{m:?}"
        );
        assert!(m.contains(&Meldung::Info("true true true true".into())), "{m:?}");
        assert!(m.contains(&Meldung::Info("true".into())), "{m:?}");
        assert!(m
            .iter()
            .any(|x| matches!(x, Meldung::Fehler(t) if t.contains("fehler.lua:1") && t.contains("nil"))));
        assert!(!l.abgestuerzt());
        assert!(l.script_laden(
            &mut welt,
            "s.lua",
            "function on_update() while true do end end",
            None,
            None
        ));
        l.update(&mut welt, 0.016);
        assert!(l.abgestuerzt());
    }

    #[test]
    fn lua_eltern_beruehrung_nachrichten_und_neu_laden() {
        use crate::welt::Script;
        let (mut l, mut welt) = lua_laufzeit();
        let kiste = welt.spawn((Name("Kiste".into()), Transform::default()));
        let s = welt.spawn((
            Name("S".into()),
            Eltern(kiste),
            Script {
                datei: "s.lua".into(),
                aktiv: true,
            },
        ));
        let quelle = r#"
            print(timbre.find("Kiste") == script.parent, timbre.get_name(script.parent))
            function on_start() timbre.broadcast("hallo", 7) end
            function on_message(m, d) print(m, d) end
        "#;
        assert!(l.script_laden(&mut welt, "s.lua", quelle, Some(s), Some(kiste)));
        l.start(&mut welt);
        let neu = "function on_start() print('neu') end";
        assert_eq!(l.script_neu_laden(&mut welt, "s.lua", neu, s, Some(kiste)), Ok(true));
        assert_eq!(
            meldungen(&mut l),
            vec![
                Meldung::Info("true Kiste".into()),
                Meldung::Info("hallo 7".into()),
                Meldung::Info("neu".into())
            ]
        );
    }

    // --- Python (CPython 3.12) ---------------------------------------------------------------

    fn python_laufzeit() -> (ScriptLaufzeit, Welt) {
        let wasm = include_bytes!("../../../assets/scripting/python.wasm");
        let zip: std::sync::Arc<[u8]> = include_bytes!("../../../assets/scripting/python312.zip")[..].into();
        let mut dateien = BTreeMap::new();
        dateien.insert("lib/python312.zip".to_string(), zip);
        let motor = ScriptMotor::mit_dateien(wasm, dateien).unwrap();
        (motor.starten().unwrap(), Welt::new())
    }

    #[test]
    fn python_script_bewegt_objekt_und_wartet() {
        let (mut l, mut welt) = python_laufzeit();
        let quelle = r#"
import math, random
kiste = None

async def on_start():
    global kiste
    kiste = timbre.spawn("Kiste")
    print("erzeugt", type(kiste).__name__, round(math.sqrt(16)))
    await task.wait(0.05)
    print("gewartet")

def on_update(dt):
    x, y = timbre.get_position(kiste)
    timbre.set_position(kiste, x + 10 * dt, y)
"#;
        assert!(
            l.script_laden(&mut welt, "test.py", quelle, None, None),
            "{:?}",
            meldungen(&mut l)
        );
        l.start(&mut welt);
        for _ in 0..60 {
            l.update(&mut welt, 1.0 / 60.0);
        }
        assert_eq!(
            meldungen(&mut l),
            vec![Meldung::Info("erzeugt int 4".into()), Meldung::Info("gewartet".into())]
        );
        let mut q = welt.query::<(&Name, &Transform)>();
        let (name, t) = q.iter().next().unwrap();
        assert_eq!(name.0, "Kiste");
        assert!((t.position.x - 10.0).abs() < 0.01, "x = {}", t.position.x);
    }

    #[test]
    fn python_fehler_mit_zeile_und_neu_laden() {
        use crate::welt::Script;
        let (mut l, mut welt) = python_laufzeit();
        assert!(!l.script_laden(&mut welt, "kaputt.py", "def f(:\n  pass", None, None));
        assert!(l.script_laden(
            &mut welt,
            "fehler.py",
            "def on_update(dt):\n    x = None\n    print(x.y)\n",
            None,
            None
        ));
        l.update(&mut welt, 0.016);
        l.update(&mut welt, 0.016);
        let m = meldungen(&mut l);
        assert!(
            matches!(&m[0], Meldung::Fehler(t) if t.starts_with("kaputt.py:1: Syntaxfehler")),
            "{m:?}"
        );
        assert!(
            matches!(&m[1], Meldung::Fehler(t) if t.starts_with("fehler.py:3: AttributeError")),
            "{m:?}"
        );
        assert_eq!(m.len(), 3, "nur einmal gemeldet + angehalten: {m:?}");
        // Live-Neuladen
        let s = welt.spawn((
            Name("S".into()),
            Script {
                datei: "s.py".into(),
                aktiv: true,
            },
        ));
        assert!(l.script_laden(
            &mut welt,
            "s.py",
            "def on_message(m, d): print('alt', m)",
            Some(s),
            None
        ));
        l.start(&mut welt);
        assert_eq!(
            l.script_neu_laden(
                &mut welt,
                "s.py",
                "def on_start(): timbre.broadcast('x', 1.5)\ndef on_message(m, d): print('neu', m, d)",
                s,
                None
            ),
            Ok(true)
        );
        assert_eq!(meldungen(&mut l), vec![Meldung::Info("neu x 1.5".into())]);
        assert_eq!(
            l.kontext().ausgehend,
            vec![Nachricht {
                name: "x".into(),
                daten: Wert::Zahl(1.5)
            }]
        );
    }

    #[test]
    fn script_object_in_allen_sprachen() {
        let mut welt = Welt::new();
        let kiste = welt.spawn((Name("Kiste".into()), Transform::default()));
        for (mut l, quelle) in [
            (
                laufzeit().0,
                "print(timbre.get_name(script.object), script.object == script.parent)",
            ),
            (
                lua_laufzeit().0,
                "print(timbre.get_name(script.object), script.object == script.parent)",
            ),
            (
                python_laufzeit().0,
                "print(timbre.get_name(script.object), script.object == script.parent)",
            ),
        ] {
            assert!(l.script_laden(&mut welt, "s", quelle, None, Some(kiste)));
            let m = meldungen(&mut l);
            assert!(
                m == vec![Meldung::Info("Kiste true".into())] || m == vec![Meldung::Info("Kiste True".into())],
                "{m:?}"
            );
        }
    }

    #[test]
    fn dreid_in_allen_sprachen() {
        let lua_quelle = "local k = timbre.spawn('Ball')\ntimbre.set_mesh(k, 'sphere', 1, 0, 0)\ntimbre.set_position3d(k, 1, 2, 3)\ntimbre.set_scale3d(k, 2)\ntimbre.set_rotation3d(k, 0, 90, 0)\nlocal x, y, z = timbre.get_position3d(k)\nlocal sx, sy, sz = timbre.get_scale3d(k)\nprint(x, y, z, sx, sz)";
        let py_quelle = "k = timbre.spawn('Ball')\ntimbre.set_mesh(k, 'sphere', 1, 0, 0)\ntimbre.set_position3d(k, 1, 2, 3)\ntimbre.set_scale3d(k, 2)\ntimbre.set_rotation3d(k, 0, 90, 0)\nx, y, z = timbre.get_position3d(k)\nsx, sy, sz = timbre.get_scale3d(k)\nprint(int(x), int(y), int(z), int(sx), int(sz))";
        for (mut l, mut welt, quelle) in [
            {
                let (l, w) = laufzeit();
                (l, w, lua_quelle)
            },
            {
                let (l, w) = lua_laufzeit();
                (l, w, lua_quelle)
            },
            {
                let (l, w) = python_laufzeit();
                (l, w, py_quelle)
            },
        ] {
            assert!(l.script_laden(&mut welt, "s", quelle, None, None));
            let m = meldungen(&mut l);
            let text = match &m[..] {
                [Meldung::Info(t)] => t.replace(".0", ""),
                _ => panic!("{m:?}"),
            };
            assert_eq!(
                text.split_whitespace().collect::<Vec<_>>(),
                ["1", "2", "3", "2", "2"],
                "{m:?}"
            );
            let mut q = welt.query::<(&crate::welt::Mesh3d, &Raum)>();
            let (mesh, raum) = q.iter().next().expect("Körper fehlt");
            assert_eq!(mesh.form, crate::renderer3d::Form3d::Kugel);
            assert_eq!(raum.drehung, glam::Vec3::new(0.0, 90.0, 0.0));
        }
    }

    #[test]
    fn nachrichten_zwischen_sprachen() {
        let (mut luau, mut welt) = laufzeit();
        let (mut lua, _) = lua_laufzeit();
        assert!(luau.script_laden(
            &mut welt,
            "a.luau",
            "function on_message(m, d) print('luau hat', m, d) end\nfunction on_start() timbre.broadcast('von_luau', {1}) end",
            None,
            None
        ));
        assert!(lua.script_laden(
            &mut welt,
            "b.lua",
            "function on_message(m, d) print('lua hat', m, d) if m == 'von_luau' then timbre.broadcast('antwort', 42) end end",
            None,
            None
        ));
        luau.start(&mut welt);
        lua.start(&mut welt);
        // So verteilt das Studio: ausgehende Nachrichten an die jeweils andere Laufzeit
        for _ in 0..3 {
            for n in std::mem::take(&mut luau.kontext().ausgehend) {
                lua.nachricht_zustellen(&mut welt, &n);
            }
            for n in std::mem::take(&mut lua.kontext().ausgehend) {
                luau.nachricht_zustellen(&mut welt, &n);
            }
        }
        let m_luau = meldungen(&mut luau);
        let m_lua = meldungen(&mut lua);
        // In der eigenen Sprache kommen auch Tabellen an, sofort
        assert!(
            matches!(&m_luau[0], Meldung::Info(t) if t.starts_with("luau hat von_luau table")),
            "{m_luau:?}"
        );
        // Aus Luau kommt die Tabelle in Lua als nil an; die eigene Antwort hört Lua auch selbst
        assert_eq!(
            m_lua,
            vec![
                Meldung::Info("lua hat von_luau nil".into()),
                Meldung::Info("lua hat antwort 42".into())
            ]
        );
        assert_eq!(m_luau[1], Meldung::Info("luau hat antwort 42".into()), "{m_luau:?}");
    }

    #[test]
    fn script_wird_live_neu_geladen() {
        use crate::welt::Script;
        let (mut l, mut welt) = laufzeit();
        let s = welt.spawn((
            Name("Zaehler".into()),
            Script {
                datei: "z.luau".into(),
                aktiv: true,
            },
        ));
        let alt = r#"
            local n = 0
            function on_start() task.delay(0.05, function() print("alte Aufgabe") end) end
            function on_update() n += 1 end
            function on_message(m) print("alt", n) end
        "#;
        assert!(l.script_laden(&mut welt, "z.luau", alt, Some(s), None));
        l.start(&mut welt);
        l.update(&mut welt, 0.016);
        // Syntaxfehler: alte Fassung läuft weiter
        assert_eq!(
            l.script_neu_laden(&mut welt, "z.luau", "das ist ((kaputt", s, None),
            Ok(false)
        );
        let m = meldungen(&mut l);
        assert!(
            matches!(&m[0], Meldung::Fehler(t) if t.contains("alte Fassung läuft weiter")),
            "{m:?}"
        );
        // Neue Fassung: on_start läuft sofort, alte Aufgabe (task.delay) endet
        let neu = r#"
            function on_start() print("neu gestartet") end
            function on_update() timbre.broadcast("x") end
            function on_message(m) print("neu", m) end
        "#;
        assert_eq!(l.script_neu_laden(&mut welt, "z.luau", neu, s, None), Ok(true));
        for _ in 0..10 {
            l.update(&mut welt, 0.016);
        }
        let m = meldungen(&mut l);
        assert_eq!(m[0], Meldung::Info("neu gestartet".into()));
        assert!(m.contains(&Meldung::Info("neu x".into())));
        assert!(
            !m.iter().any(|x| matches!(x, Meldung::Info(t) if t.starts_with("alt"))),
            "{m:?}"
        );
    }

    #[test]
    fn sounds_werden_angefordert() {
        let (mut l, mut welt) = laufzeit();
        l.kontext().sound_namen.insert("Muenze".into());
        let quelle = r#"
            timbre.play_sound("Muenze")
            timbre.play_sound("Muenze.wav", 0.5, true)
            timbre.stop_sounds("Muenze")
            timbre.stop_sounds()
            timbre.play_sound("Gibtsnicht")
        "#;
        assert!(!l.script_laden(&mut welt, "s.luau", quelle, None, None));
        assert_eq!(
            std::mem::take(&mut l.kontext().sound_befehle),
            vec![
                SoundBefehl::Abspielen {
                    name: "Muenze".into(),
                    lautstaerke: 1.0,
                    schleife: false
                },
                SoundBefehl::Abspielen {
                    name: "Muenze".into(),
                    lautstaerke: 0.5,
                    schleife: true
                },
                SoundBefehl::Stoppen("Muenze".into()),
                SoundBefehl::Stoppen(String::new()),
            ]
        );
        assert!(meldungen(&mut l)
            .iter()
            .any(|m| matches!(m, Meldung::Fehler(t) if t.contains("s.luau:6") && t.contains("Gibtsnicht"))));
    }

    #[test]
    fn endlosschleife_wird_abgebrochen() {
        let (mut l, mut welt) = laufzeit();
        assert!(l.script_laden(
            &mut welt,
            "schleife.luau",
            "function on_update() while true do end end",
            None,
            None
        ));
        l.update(&mut welt, 0.016);
        assert!(l.abgestuerzt());
        assert!(meldungen(&mut l)
            .iter()
            .any(|m| matches!(m, Meldung::Fehler(t) if t.contains("Endlosschleife"))));
    }

    #[test]
    fn script_kennt_eltern_und_endet_mit_ihm() {
        use crate::welt::{Eltern, Script};
        let (mut l, mut welt) = laufzeit();
        let kiste = welt.spawn((Name("Kiste".into()), Transform::default()));
        let s = welt.spawn((
            Name("Steuerung".into()),
            Eltern(kiste),
            Script {
                datei: "s.luau".into(),
                aktiv: true,
            },
        ));
        let quelle = r#"
            print(script.name, timbre.find("Kiste") == script.parent)
            function on_update() print("lebt") end
        "#;
        assert!(l.script_laden(&mut welt, "s.luau", quelle, Some(s), Some(kiste)));
        l.update(&mut welt, 0.016);
        entfernen(&mut welt, kiste); // Script-Objekt geht mit
        l.update(&mut welt, 0.016); // erkennt: Script-Objekt weg → anhalten
        l.update(&mut welt, 0.016);
        let m = meldungen(&mut l);
        assert_eq!(m[0], Meldung::Info("s.luau true".into()));
        assert_eq!(m.iter().filter(|x| **x == Meldung::Info("lebt".into())).count(), 2);
    }

    #[test]
    fn positionen_von_kindern_sind_weltkoordinaten() {
        let (mut l, mut welt) = laufzeit();
        let quelle = r#"
            local e = timbre.spawn("Eltern")
            timbre.set_position(e, 100, 0)
            local k = timbre.spawn("Kind")
            timbre.set_position(k, 110, 0)
            timbre.set_parent(k, e)      -- springt nicht
            local x, y = timbre.get_position(k)
            print(x, y)
            timbre.set_position(e, 200, 50) -- Kind geht mit
            x, y = timbre.get_position(k)
            print(x, y)
        "#;
        assert!(l.script_laden(&mut welt, "t.luau", quelle, None, None));
        assert_eq!(
            meldungen(&mut l),
            vec![Meldung::Info("110 0".into()), Meldung::Info("210 50".into())]
        );
    }

    #[test]
    fn task_wait_delay_und_broadcast() {
        let (mut l, mut welt) = laufzeit();
        let a = r#"
            function on_start()
                print("start")
                task.wait(0.5)
                print("nach wait")
                timbre.broadcast("hallo", 42)
            end
            task.delay(0.2, function() print("delay") end)
            shared.zaehler = 1
        "#;
        let b = r#"
            timbre.ui_text("t", "nur Text", 1, 2)
            function on_message(name, wert)
                print("nachricht", name, wert, shared.zaehler)
            end
        "#;
        assert!(l.script_laden(&mut welt, "a", a, None, None));
        assert!(l.script_laden(&mut welt, "b", b, None, None));
        assert!(l.script_laden(&mut welt, "c", "function on_update() task.wait(1) end", None, None));
        l.start(&mut welt);
        for _ in 0..40 {
            l.update(&mut welt, 1.0 / 60.0);
        }
        let m = meldungen(&mut l);
        let infos: Vec<String> = m
            .iter()
            .filter_map(|x| {
                if let Meldung::Info(t) = x {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(infos, vec!["start", "delay", "nach wait", "nachricht hallo 42 1"]);
        // task.wait in on_update ist verboten → verständlicher Fehler, Script c angehalten
        assert!(m
            .iter()
            .any(|x| matches!(x, Meldung::Fehler(t) if t.contains("nicht in on_update"))));
    }

    #[test]
    fn on_touch_wird_gemeldet() {
        use crate::renderer::TexturId;
        use crate::welt::{Bild, Eltern, Script};
        let (mut l, mut welt) = laufzeit();
        let spieler = welt.spawn((
            Name("Spieler".into()),
            Transform::default(),
            Bild::new(TexturId::test(0), Vec2::splat(40.0)),
            Kollision {
                form: Form::Kreis,
                fest: true,
                verankert: false,
            },
        ));
        let script = welt.spawn((
            Name("S".into()),
            Eltern(spieler),
            Script {
                datei: "s".into(),
                aktiv: true,
            },
        ));
        let muenze = welt.spawn((
            Name("Gold".into()),
            Transform::bei(Vec2::new(100.0, 0.0)),
            Bild::new(TexturId::test(0), Vec2::splat(20.0)),
            Kollision {
                form: Form::Kreis,
                fest: false,
                verankert: true,
            },
        ));
        let quelle = r#"
            function on_touch(anderes)
                print("berührt", timbre.get_name(anderes), #timbre.get_touching(script.parent))
                timbre.destroy(anderes)
            end
        "#;
        assert!(l.script_laden(&mut welt, "s", quelle, Some(script), Some(spieler)));
        l.start(&mut welt);
        l.kollisionen(&mut welt); // noch weit weg
        welt.get::<&mut Transform>(spieler).unwrap().position.x = 80.0;
        l.kollisionen(&mut welt);
        assert_eq!(meldungen(&mut l), vec![Meldung::Info("berührt Gold 1".into())]);
        assert!(!welt.contains(muenze));
    }

    #[test]
    fn scripts_sind_voneinander_getrennt() {
        let (mut l, mut welt) = laufzeit();
        assert!(l.script_laden(&mut welt, "a.luau", "zaehler = 1", None, None));
        assert!(l.script_laden(&mut welt, "b.luau", "print(zaehler)", None, None));
        assert_eq!(meldungen(&mut l), vec![Meldung::Info("nil".into())]);
    }
}
