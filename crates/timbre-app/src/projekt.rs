//! Ein Timbre-Projekt: ein Ordner mit `szene.tbr` und einem Unterordner `scripts/`.
//!
//! ```text
//! projekte/beispiel/
//!   szene.tbr               die Objekte der Szene – auch die Script-Objekte
//!   scripts/Steuerung.luau    Quelltext der Scripts (ein Script-Objekt verweist per Dateiname darauf)
//!   scripts/Laubregen.luau
//!   scripts/.papierkorb/    hierhin wandern Dateien gelöschter Scripts (nichts geht verloren)
//! ```
//!
//! Im Browser gibt es kein Dateisystem – dort ist das Beispielprojekt eingebaut.

use timbre_core::tbr::Szene;

pub struct Script {
    pub name: String,
    pub text: String,
}

pub struct ProjektInhalt {
    pub name: String,
    pub scripts: Vec<Script>,
    pub szene: Option<Szene>,
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
                if pfad.extension().is_some_and(|x| x == "luau") {
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
        Ok(ProjektInhalt { name, scripts, szene })
    }

    pub fn script_speichern(ordner: &Path, name: &str, text: &str) -> Result<(), String> {
        let pfad = ordner.join("scripts").join(name);
        std::fs::create_dir_all(pfad.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&pfad, text).map_err(|e| format!("{}: {e}", pfad.display()))
    }

    /// Alle `.luau`-Dateien im Ordner `scripts/`.
    pub fn script_dateien(ordner: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(ordner.join("scripts"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file() && e.path().extension().is_some_and(|x| x == "luau"))
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
    }
}
