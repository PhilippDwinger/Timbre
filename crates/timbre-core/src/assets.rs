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
    namen: HashMap<String, TexturId>,
    groessen: HashMap<TexturId, (u32, u32)>,
}

impl Assets {
    /// Lädt eine Textur aus `.tbr`-Bytes. Ist sie schon geladen, wird nichts doppelt hochgeladen.
    pub fn textur_laden(&mut self, renderer: &mut Renderer, tbr: &[u8]) -> Result<TexturId, TbrFehler> {
        let textur = Textur::aus_bytes(tbr)?;
        if let Some(&id) = self.texturen.get(&textur.id) {
            return Ok(id);
        }
        let id = renderer.textur_aus_tbr(&textur);
        self.texturen.insert(textur.id, id);
        self.rueckwaerts.insert(id, textur.id);
        self.namen.insert(textur.name.clone(), id);
        self.groessen.insert(id, (textur.breite, textur.hoehe));
        Ok(id)
    }

    /// Asset-ID → geladene Textur.
    pub fn textur(&self, asset: AssetId) -> Option<TexturId> {
        self.texturen.get(&asset).copied()
    }

    /// Texturname (aus der .tbr-Datei) → geladene Textur. Scripts benutzen Namen.
    pub fn textur_namen(&self) -> &HashMap<String, TexturId> {
        &self.namen
    }

    /// Name einer geladenen Textur.
    pub fn name_von(&self, textur: TexturId) -> Option<&str> {
        self.namen.iter().find(|(_, &t)| t == textur).map(|(n, _)| n.as_str())
    }

    /// Breite und Höhe einer geladenen Textur in Pixeln.
    pub fn pixel_groesse(&self, textur: TexturId) -> Option<(u32, u32)> {
        self.groessen.get(&textur).copied()
    }

    /// Geladene Textur → Asset-ID (zum Speichern von Szenen).
    pub fn asset_von(&self, textur: TexturId) -> Option<AssetId> {
        self.rueckwaerts.get(&textur).copied()
    }
}

/// Liest ein PNG beliebiger Art (Graustufen, Palette, 16 Bit …) als RGBA mit 8 Bit.
/// Ergebnis: (Breite, Höhe, Pixel).
pub fn png_lesen(daten: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(daten));
    decoder.set_transformations(
        png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().map_err(|e| format!("kein gültiges PNG: {e}"))?;
    let mut puffer = vec![0; reader.output_buffer_size().ok_or("PNG ist zu groß")?];
    let info = reader
        .next_frame(&mut puffer)
        .map_err(|e| format!("PNG kann nicht gelesen werden: {e}"))?;
    puffer.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => puffer,
        png::ColorType::GrayscaleAlpha => puffer.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        andere => return Err(format!("unerwartetes PNG-Farbformat {andere:?}")),
    };
    Ok((info.width, info.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_wird_zu_rgba() {
        let daten = include_bytes!("../../../assets/quellen/blatt.png");
        let (b, h, pixel) = png_lesen(daten).unwrap();
        assert!(b > 0 && h > 0);
        assert_eq!(pixel.len(), (b * h * 4) as usize);
        assert!(png_lesen(b"kein png").is_err());
    }
}
