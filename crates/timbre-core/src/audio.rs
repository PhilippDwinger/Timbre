//! Sounds abspielen (nativ, mit `rodio`).
//!
//! Sounds sind Dateien im Ordner `sounds/` eines Projekts (WAV, OGG, MP3, FLAC). Scripts spielen
//! sie per Name ab (`timbre.play_sound("Muenze")` – Dateiname ohne Endung).
//! Gibt es kein Audiogerät, läuft alles weiter – nur ohne Ton.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

use rodio::{Decoder, MixerDeviceSink, Player};

pub struct Audio {
    /// Verbindung zum Lautsprecher; `None` = kein Audiogerät gefunden
    ausgang: Option<MixerDeviceSink>,
    /// Name → Dateiinhalt (wird bei jedem Abspielen neu dekodiert, ist also beliebig oft gleichzeitig spielbar)
    daten: HashMap<String, Arc<[u8]>>,
    /// Gerade laufende Sounds
    laufend: Vec<(String, Player)>,
}

impl Audio {
    /// Öffnet das Standard-Audiogerät. Klappt das nicht, kommt der Grund als zweiter Wert zurück.
    pub fn neu() -> (Self, Option<String>) {
        let (ausgang, fehler) = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut a) => {
                a.log_on_drop(false);
                (Some(a), None)
            }
            Err(e) => (None, Some(format!("Kein Audiogerät – Sounds bleiben stumm ({e})"))),
        };
        let audio = Self {
            ausgang,
            daten: HashMap::new(),
            laufend: Vec::new(),
        };
        (audio, fehler)
    }

    /// Lässt sich das als Sound lesen?
    pub fn pruefen(daten: &[u8]) -> Result<(), String> {
        let daten: Arc<[u8]> = daten.into();
        Decoder::new(Cursor::new(daten))
            .map(|_| ())
            .map_err(|e| format!("kein lesbarer Sound: {e}"))
    }

    /// Sound bekannt machen (prüft, ob er sich dekodieren lässt).
    pub fn sound_setzen(&mut self, name: &str, daten: Vec<u8>) -> Result<(), String> {
        let daten: Arc<[u8]> = daten.into();
        Decoder::new(Cursor::new(daten.clone()))
            .map_err(|e| format!("Sound '{name}' kann nicht gelesen werden: {e}"))?;
        self.daten.insert(name.to_string(), daten);
        Ok(())
    }

    pub fn hat(&self, name: &str) -> bool {
        self.daten.contains_key(name)
    }

    pub fn namen(&self) -> impl Iterator<Item = &String> {
        self.daten.keys()
    }

    /// Spielt einen Sound ab. `lautstaerke` 1.0 = normal; `schleife` = endlos wiederholen.
    pub fn abspielen(&mut self, name: &str, lautstaerke: f32, schleife: bool) -> Result<(), String> {
        let daten = self
            .daten
            .get(name)
            .ok_or_else(|| format!("unbekannter Sound '{name}'"))?
            .clone();
        self.aufraeumen();
        let Some(ausgang) = &self.ausgang else {
            return Ok(()); // stumm
        };
        let player = Player::connect_new(ausgang.mixer());
        let quelle = Cursor::new(daten);
        if schleife {
            player.append(Decoder::new_looped(quelle).map_err(|e| e.to_string())?);
        } else {
            player.append(Decoder::new(quelle).map_err(|e| e.to_string())?);
        }
        player.set_volume(lautstaerke as _);
        self.laufend.push((name.to_string(), player));
        Ok(())
    }

    /// Stoppt alle laufenden Sounds mit diesem Namen – leerer Name = alle.
    pub fn stoppen(&mut self, name: &str) {
        self.laufend.retain(|(n, p)| {
            let weg = name.is_empty() || n == name;
            if weg {
                p.stop();
            }
            !weg
        });
    }

    /// Fertig gespielte Sounds vergessen.
    fn aufraeumen(&mut self) {
        self.laufend.retain(|(_, p)| !p.empty());
    }
}

/// Dateiendungen, die als Sound gelten.
pub fn ist_sound_datei(endung: &str) -> bool {
    matches!(endung.to_ascii_lowercase().as_str(), "wav" | "ogg" | "mp3" | "flac")
}
