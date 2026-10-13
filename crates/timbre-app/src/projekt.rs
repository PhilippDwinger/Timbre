//! Ein Timbre-Projekt: ein Ordner mit `szene.tbr` und einem Unterordner `scripts/`.
//!
//! ```text
//! projekte/beispiel/
//!   szene.tbr               die Objekte der Szene – auch die Script-Objekte
//!   scripts/Steuerung.luau    Quelltext der Scripts (ein Script-Objekt verweist per Dateiname darauf)
//!   scripts/Laubregen.luau
//!   scripts/.papierkorb/    hierhin wandern Dateien gelöschter Scripts (nichts geht verloren)
//!   texturen/Muenze.tbr     importierte Bilder (PNG ins Studio ziehen → wird hier zur .tbr-Textur)
//!   sounds/Muenze.wav       Sounds (WAV, OGG, MP3, FLAC – ins Studio ziehen oder einfach hineinkopieren)
//! ```
//!
//! Im Browser gibt es kein Dateisystem – dort ist das Beispielprojekt eingebaut.

use timbre_core::tbr::Szene;

pub struct Script {
    pub name: String,
    pub text: String,
}

/// Script-Sprachen. Jede ist ein eigenes WebAssembly-Gast-Modul; die Dateiendung entscheidet.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sprache {
    Luau,
    Lua,
}

impl Sprache {
    pub const ALLE: [Sprache; 2] = [Sprache::Luau, Sprache::Lua];

    pub fn aus_datei(datei: &str) -> Option<Sprache> {
        if datei.ends_with(".luau") {
            Some(Sprache::Luau)
        } else if datei.ends_with(".lua") {
            Some(Sprache::Lua)
        } else {
            None
        }
    }

    pub fn endung(self) -> &'static str {
        match self {
            Sprache::Luau => "luau",
            Sprache::Lua => "lua",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sprache::Luau => "Luau",
            Sprache::Lua => "Lua 5.4",
        }
    }

    /// Text für ein neues Script.
    pub fn vorlage(self) -> &'static str {
        match self {
            Sprache::Luau => "-- Neues Script (Luau)\n-- script.parent = das Objekt, unter dem dieses Script hängt (oder nil)\n\nfunction on_start()\n\tprint(\"Hallo aus\", script.name)\nend\n\nfunction on_update(dt)\n\nend\n",
            Sprache::Lua => "-- Neues Script (Lua 5.4)\n-- script.parent = das Objekt, unter dem dieses Script hängt (oder nil)\n\nfunction on_start()\n\tprint(\"Hallo aus\", script.name)\nend\n\nfunction on_update(dt)\n\nend\n",
        }
    }
}

/// Dateiname ohne Script-Endung: `Steuerung.luau` → `Steuerung`.
pub fn ohne_script_endung(datei: &str) -> &str {
    datei
        .strip_suffix(".luau")
        .or_else(|| datei.strip_suffix(".lua"))
        .unwrap_or(datei)
}

pub struct ProjektInhalt {
    pub name: String,
    pub scripts: Vec<Script>,
    pub szene: Option<Szene>,
    /// Texturen des Projekts: (Dateiname, Inhalt der .tbr-Datei)
    pub texturen: Vec<(String, Vec<u8>)>,
    /// Sounds des Projekts: (Name ohne Endung, Dateiinhalt)
    pub sounds: Vec<(String, Vec<u8>)>,
}

#[cfg(not(target_arch = "wasm32"))]
pub use datei::*;

#[cfg(not(target_arch = "wasm32"))]
mod datei {
    use std::path::{Path, PathBuf};

    use super::*;

    /// Standard-Projekt, wenn beim Start keins angegeben wird (relativ zum Repo-Ordner).
    pub const STANDARD_ORDNER: &str = "projekte/beispiel";

    pub fn ordner_aus_argumenten() -> PathBuf {
        std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(STANDARD_ORDNER))
    }

    pub fn laden(ordner: &Path) -> Result<ProjektInhalt, String> {
        if !ordner.is_dir() {
            return Err(format!(
                "Projektordner '{}' nicht gefunden. Starte Timbre aus dem Repo-Ordner oder gib den Pfad an: \
                 cargo run -p timbre-app -- projekte\\beispiel",
                ordner.display()
            ));
        }
        let name = ordner
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let mut scripts = Vec::new();
        if let Ok(eintraege) = std::fs::read_dir(ordner.join("scripts")) {
            for e in eintraege.flatten() {
                let pfad = e.path();
                if Sprache::aus_datei(&pfad.to_string_lossy()).is_some() {
                    let text = std::fs::read_to_string(&pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
                    let name = pfad.file_name().unwrap().to_string_lossy().into_owned();
                    scripts.push(Script {
                        name,
                        text: text.replace("\r\n", "\n"),
                    });
                }
            }
        }
        scripts.sort_by(|a, b| a.name.cmp(&b.name));

        let szene_pfad = ordner.join("szene.tbr");
        let szene = match std::fs::read(&szene_pfad) {
            Ok(daten) => Some(Szene::aus_bytes(&daten).map_err(|e| format!("{}: {e}", szene_pfad.display()))?),
            Err(_) => None,
        };
        let texturen = dateien_lesen(&ordner.join(TEXTUR_ORDNER), |e| e == "tbr")?;
        let sounds = dateien_lesen(&ordner.join(SOUND_ORDNER), timbre_core::audio::ist_sound_datei)?
            .into_iter()
            .map(|(datei, daten)| (ohne_endung(&datei), daten))
            .collect();
        Ok(ProjektInhalt {
            name,
            scripts,
            szene,
            texturen,
            sounds,
        })
    }

    pub const TEXTUR_ORDNER: &str = "texturen";
    pub const SOUND_ORDNER: &str = "sounds";

    /// Alle Dateien eines Ordners mit passender Endung, nach Namen sortiert. Fehlt der Ordner: leer.
    fn dateien_lesen(ordner: &Path, endung_passt: impl Fn(&str) -> bool) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mut v = Vec::new();
        let Ok(eintraege) = std::fs::read_dir(ordner) else {
            return Ok(v);
        };
        for e in eintraege.flatten() {
            let pfad = e.path();
            let passt = pfad.is_file() && pfad.extension().is_some_and(|x| endung_passt(&x.to_string_lossy()));
            if passt {
                let daten = std::fs::read(&pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
                v.push((e.file_name().to_string_lossy().into_owned(), daten));
            }
        }
        v.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(v)
    }

    pub fn ohne_endung(datei: &str) -> String {
        Path::new(datei)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Freier Dateiname im Ordner: `Muenze.tbr`, sonst `Muenze2.tbr`, `Muenze3.tbr` …
    fn freier_pfad(ordner: &Path, name: &str, endung: &str) -> PathBuf {
        let mut pfad = ordner.join(format!("{name}.{endung}"));
        let mut n = 2;
        while pfad.exists() {
            pfad = ordner.join(format!("{name}{n}.{endung}"));
            n += 1;
        }
        pfad
    }

    /// Textur als `texturen/<name>.tbr` speichern. Gibt den Dateinamen zurück.
    pub fn textur_speichern(ordner: &Path, textur: &timbre_core::tbr::Textur) -> Result<String, String> {
        let ziel_ordner = ordner.join(TEXTUR_ORDNER);
        std::fs::create_dir_all(&ziel_ordner).map_err(|e| e.to_string())?;
        let pfad = freier_pfad(&ziel_ordner, &textur.name, "tbr");
        std::fs::write(&pfad, textur.zu_bytes()).map_err(|e| format!("{}: {e}", pfad.display()))?;
        Ok(pfad.file_name().unwrap().to_string_lossy().into_owned())
    }

    /// Sound-Datei nach `sounds/` kopieren. `name` = gewünschter Name (ohne Endung).
    /// Gibt den tatsächlichen Namen zurück (bei Namensgleichheit mit Nummer).
    pub fn sound_kopieren(ordner: &Path, quelle: &Path, name: &str) -> Result<String, String> {
        let ziel_ordner = ordner.join(SOUND_ORDNER);
        std::fs::create_dir_all(&ziel_ordner).map_err(|e| e.to_string())?;
        let endung = quelle
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let pfad = freier_pfad(&ziel_ordner, name, &endung);
        std::fs::copy(quelle, &pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
        Ok(ohne_endung(&pfad.file_name().unwrap().to_string_lossy()))
    }

    pub fn script_speichern(ordner: &Path, name: &str, text: &str) -> Result<(), String> {
        let pfad = ordner.join("scripts").join(name);
        std::fs::create_dir_all(pfad.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&pfad, text).map_err(|e| format!("{}: {e}", pfad.display()))
    }

    /// Alle Script-Dateien (`.luau`, `.lua`) im Ordner `scripts/`.
    pub fn script_dateien(ordner: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(ordner.join("scripts"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file() && Sprache::aus_datei(&e.path().to_string_lossy()).is_some())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    /// Script-Datei in `scripts/.papierkorb/` verschieben (statt sie zu löschen).
    pub fn in_papierkorb(ordner: &Path, datei: &str) -> Result<(), String> {
        let korb = ordner.join("scripts").join(".papierkorb");
        std::fs::create_dir_all(&korb).map_err(|e| e.to_string())?;
        let mut ziel = korb.join(datei);
        let mut n = 2;
        while ziel.exists() {
            ziel = korb.join(format!("{n}_{datei}"));
            n += 1;
        }
        std::fs::rename(ordner.join("scripts").join(datei), &ziel).map_err(|e| format!("{datei}: {e}"))
    }

    pub fn szene_speichern(ordner: &Path, szene: &Szene) -> Result<(), String> {
        let pfad = ordner.join("szene.tbr");
        std::fs::write(&pfad, szene.zu_bytes()).map_err(|e| format!("{}: {e}", pfad.display()))
    }
}

/// Das eingebaute Beispielprojekt (für den Browser).
#[cfg(target_arch = "wasm32")]
pub fn eingebaut() -> ProjektInhalt {
    let s = |name: &str, text: &str| Script {
        name: name.into(),
        text: text.into(),
    };
    ProjektInhalt {
        name: "beispiel".into(),
        scripts: vec![
            s(
                "Laubregen.luau",
                include_str!("../../../projekte/beispiel/scripts/Laubregen.luau"),
            ),
            s(
                "Steuerung.luau",
                include_str!("../../../projekte/beispiel/scripts/Steuerung.luau"),
            ),
        ],
        szene: Szene::aus_bytes(include_bytes!("../../../projekte/beispiel/szene.tbr")).ok(),
        texturen: Vec::new(),
        sounds: Vec::new(),
    }
}
