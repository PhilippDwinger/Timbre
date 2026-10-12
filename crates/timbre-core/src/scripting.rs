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

use crate::input::{taste_aus_name, Input};
use crate::kamera::Kamera2d;
use crate::renderer::TexturId;
use crate::welt::{objekt_aus_nummer, objekt_nummer, Bewegung, Bild, Name, Transform, Welt};

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
}

/// Das übersetzte Gast-Modul. Das Übersetzen dauert etwas, darum nur einmal pro Sprache.
pub struct ScriptMotor {
    engine: Engine,
    modul: Module,
}

impl ScriptMotor {
    pub fn neu(wasm: &[u8]) -> Result<Self, String> {
        let mut cfg = Config::new();
        cfg.wasm_exceptions(true); // Luau meldet Fehler per WebAssembly-Exception
        cfg.consume_fuel(true); // Endlosschleifen abbrechen können
        let engine = Engine::new(&cfg).map_err(|e| format!("wasmtime: {e}"))?;
        let modul = Module::new(&engine, wasm).map_err(|e| format!("Gast-Modul ungültig: {e}"))?;
        Ok(Self { engine, modul })
    }

    /// Frische Laufzeit für einen Durchlauf (▶). Jeder Start beginnt bei null.
    pub fn starten(&self) -> Result<ScriptLaufzeit, String> {
        let mut linker = Linker::new(&self.engine);
        importe_registrieren(&mut linker).map_err(|e| e.to_string())?;
        let mut store = Store::new(&self.engine, Kontext::default());
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

/// Eine laufende Script-Umgebung (von ▶ bis ■).
pub struct ScriptLaufzeit {
    store: Store<Kontext>,
    _instanz: Instance,
    speicher: Memory,
    alloc: TypedFunc<i32, i32>,
    free: TypedFunc<i32, ()>,
    laden: TypedFunc<(i32, i32, i32, i32), i32>,
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
                        "Script hat zu lange gebraucht und wurde abgebrochen – Endlosschleife?"
                            .to_string()
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
        self.speicher
            .write(&mut self.store, p as usize, text.as_bytes())?;
        Ok(p)
    }

    /// Ein Script laden und seinen Hauptteil ausführen. `false` bei Fehler (steht dann in den Meldungen).
    pub fn script_laden(&mut self, welt: &mut Welt, name: &str, quelle: &str) -> bool {
        self.mit_welt(welt, TREIBSTOFF_LADEN, |s| {
            let n = s.text_ablegen(name)?;
            let q = s.text_ablegen(quelle)?;
            let r = s
                .laden
                .call(&mut s.store, (n, name.len() as i32, q, quelle.len() as i32))?;
            s.free.call(&mut s.store, n)?;
            s.free.call(&mut s.store, q)?;
            Ok(r == 0)
        })
        .unwrap_or(false)
    }

    /// `on_start()` aller Scripts.
    pub fn start(&mut self, welt: &mut Welt) {
        self.mit_welt(welt, TREIBSTOFF_LADEN, |s| s.start.call(&mut s.store, ()));
    }

    /// Ein Logik-Schritt: `on_update(dt)` aller Scripts.
    pub fn update(&mut self, welt: &mut Welt, dt: f32) {
        self.store.data_mut().zeit += dt as f64;
        self.mit_welt(welt, TREIBSTOFF_UPDATE, |s| s.update.call(&mut s.store, dt));
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
            let _ = c.data_mut().welt.despawn(o);
        }
    })?;

    l.func_wrap(
        M,
        "set_position",
        |mut c: Caller<'_, Kontext>, id: i64, x: f32, y: f32| {
            aendern::<Transform>(&mut c, id, |t| t.position = Vec2::new(x, y));
        },
    )?;
    l.func_wrap(
        M,
        "get_position",
        |mut c: Caller<'_, Kontext>, id: i64, aus: i32| -> wasmtime::Result<i32> {
            let pos = objekt(id)
                .and_then(|o| c.data().welt.get::<&Transform>(o).ok().map(|t| t.position));
            match pos {
                Some(p) => {
                    floats_schreiben(&mut c, aus, &[p.x, p.y])?;
                    Ok(1)
                }
                None => Ok(0),
            }
        },
    )?;
    l.func_wrap(
        M,
        "set_rotation",
        |mut c: Caller<'_, Kontext>, id: i64, r: f32| {
            aendern::<Transform>(&mut c, id, |t| t.drehung = r);
        },
    )?;
    l.func_wrap(
        M,
        "get_rotation",
        |c: Caller<'_, Kontext>, id: i64| -> f32 {
            objekt(id)
                .and_then(|o| c.data().welt.get::<&Transform>(o).ok().map(|t| t.drehung))
                .unwrap_or(0.0)
        },
    )?;
    l.func_wrap(
        M,
        "set_scale",
        |mut c: Caller<'_, Kontext>, id: i64, x: f32, y: f32| {
            aendern::<Transform>(&mut c, id, |t| t.skala = Vec2::new(x, y));
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
        |mut c: Caller<'_, Kontext>,
         id: i64,
         p: i32,
         n: i32,
         b: f32,
         h: f32,
         ebene: i32|
         -> wasmtime::Result<i32> {
            let name = text(&mut c, p, n)?;
            let Some(&textur) = c.data().texturen.get(&name) else {
                return Ok(0);
            };
            if let Some(o) = objekt(id) {
                let farbe = c
                    .data()
                    .welt
                    .get::<&Bild>(o)
                    .map(|b| b.farbe)
                    .unwrap_or([1.0; 4]);
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

    // Die wenigen Systemfunktionen, die die C-Bibliothek im Gast erwartet.
    // Dateien gibt es für Scripts nicht – alles meldet "nicht erlaubt".
    const W: &str = "wasi_snapshot_preview1";
    const NICHT_ERLAUBT: i32 = 63; // EPERM
    l.func_wrap(W, "fd_close", |_: i32| -> i32 { NICHT_ERLAUBT })?;
    l.func_wrap(W, "fd_fdstat_get", |_: i32, _: i32| -> i32 {
        NICHT_ERLAUBT
    })?;
    l.func_wrap(W, "fd_seek", |_: i32, _: i64, _: i32, _: i32| -> i32 {
        NICHT_ERLAUBT
    })?;
    l.func_wrap(W, "fd_write", |_: i32, _: i32, _: i32, _: i32| -> i32 {
        NICHT_ERLAUBT
    })?;
    l.func_wrap(
        W,
        "clock_time_get",
        |mut c: Caller<'_, Kontext>,
         _id: i32,
         _genauigkeit: i64,
         aus: i32|
         -> wasmtime::Result<i32> {
            let nanos = (c.data().zeit * 1e9) as u64;
            speicher(&mut c)?.write(&mut c, aus as usize, &nanos.to_le_bytes())?;
            Ok(0)
        },
    )?;
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
        assert!(l.script_laden(&mut welt, "test.luau", quelle));
        l.start(&mut welt);
        for _ in 0..60 {
            l.update(&mut welt, 1.0 / 60.0);
        }
        assert_eq!(
            meldungen(&mut l),
            vec![Meldung::Info("erzeugt true".into())]
        );
        let mut q = welt.query::<(&Name, &Transform)>();
        let (name, t) = q.iter().next().unwrap();
        assert_eq!(name.0, "Kiste");
        assert!((t.position.x - 10.0).abs() < 0.01, "x = {}", t.position.x);
    }

    #[test]
    fn fehler_werden_gemeldet_und_halten_nur_das_script_an() {
        let (mut l, mut welt) = laufzeit();
        assert!(!l.script_laden(&mut welt, "kaputt.luau", "das ist ((kein luau"));
        assert!(l.script_laden(
            &mut welt,
            "fehler.luau",
            "function on_update() local t = nil; print(t.x) end"
        ));
        assert!(l.script_laden(
            &mut welt,
            "gut.luau",
            "function on_update() print('gut') end"
        ));
        l.update(&mut welt, 0.016);
        l.update(&mut welt, 0.016);
        let m = meldungen(&mut l);
        assert!(matches!(&m[0], Meldung::Fehler(t) if t.starts_with("kaputt.luau:1")));
        assert!(m.iter().any(|x| matches!(x, Meldung::Fehler(t) if t.contains("fehler.luau:1") && t.contains("index nil"))));
        // "gut" läuft in beiden Schritten weiter, "fehler" nur einmal gemeldet
        assert_eq!(
            m.iter()
                .filter(|x| **x == Meldung::Info("gut".into()))
                .count(),
            2
        );
        assert!(!l.abgestuerzt());
    }

    #[test]
    fn endlosschleife_wird_abgebrochen() {
        let (mut l, mut welt) = laufzeit();
        assert!(l.script_laden(
            &mut welt,
            "schleife.luau",
            "function on_update() while true do end end"
        ));
        l.update(&mut welt, 0.016);
        assert!(l.abgestuerzt());
        assert!(meldungen(&mut l)
            .iter()
            .any(|m| matches!(m, Meldung::Fehler(t) if t.contains("Endlosschleife"))));
    }

    #[test]
    fn scripts_sind_voneinander_getrennt() {
        let (mut l, mut welt) = laufzeit();
        assert!(l.script_laden(&mut welt, "a.luau", "zaehler = 1"));
        assert!(l.script_laden(&mut welt, "b.luau", "print(zaehler)"));
        assert_eq!(meldungen(&mut l), vec![Meldung::Info("nil".into())]);
    }
}
