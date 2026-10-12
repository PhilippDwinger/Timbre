//! Asset-Verwaltung: merkt sich, welche `.tbr`-Datei (Asset-ID) zu welcher geladenen
//! Textur auf der Grafikkarte gehört. Szenen verweisen per Asset-ID, der Renderer
//! arbeitet mit `TexturId` – hier wird übersetzt.

use std::collections::HashMap;

use timbre_tbr::{AssetId, TbrFehler, Textur};

use crate::renderer::{Renderer, TexturId};

#[derive(Default)]
pub struct Assets {
    texturen: HashMap<AssetId, TexturId>,
    rueckwaerts: HashMap<TexturId, AssetId>,
}

impl Assets {
    /// Lädt eine Textur aus `.tbr`-Bytes. Ist sie schon geladen, wird nichts doppelt hochgeladen.
    pub fn textur_laden(
        &mut self,
        renderer: &mut Renderer,
        tbr: &[u8],
    ) -> Result<TexturId, TbrFehler> {
        let textur = Textur::aus_bytes(tbr)?;
        if let Some(&id) = self.texturen.get(&textur.id) {
            return Ok(id);
        }
        let id = renderer.textur_aus_tbr(&textur);
        self.texturen.insert(textur.id, id);
        self.rueckwaerts.insert(id, textur.id);
        Ok(id)
    }

    /// Asset-ID → geladene Textur.
    pub fn textur(&self, asset: AssetId) -> Option<TexturId> {
        self.texturen.get(&asset).copied()
    }

    /// Geladene Textur → Asset-ID (zum Speichern von Szenen).
    pub fn asset_von(&self, textur: TexturId) -> Option<AssetId> {
        self.rueckwaerts.get(&textur).copied()
    }
}
