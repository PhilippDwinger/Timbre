//! Texturen: ein Bild (RGBA, 8 Bit pro Kanal, sRGB) und optional Ebenen.
//!
//! `pixel` ist immer das fertig zusammengerechnete Bild – das lädt die Engine.
//! `ebenen` braucht nur der Mal-Editor (Timbre Stain), damit ein Bild bearbeitbar bleibt.

use crate::bytes::{Leser, Schreiber};
use crate::{bloecke, text, AssetId, AssetTyp, DateiSchreiber, TbrFehler};

/// Pixelformat im Block TINF. Bisher nur eins; neue Formate bekommen neue Nummern.
const FORMAT_RGBA8_SRGB: u8 = 0;

#[derive(Clone, Debug, PartialEq)]
pub struct Textur {
    pub id: AssetId,
    pub name: String,
    pub breite: u32,
    pub hoehe: u32,
    /// RGBA, 4 Bytes pro Pixel, Zeile für Zeile von oben links.
    pub pixel: Vec<u8>,
    /// Ebenen von unten nach oben. Leer = einfaches Bild.
    pub ebenen: Vec<Ebene>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ebene {
    pub name: String,
    pub sichtbar: bool,
    /// 0.0 bis 1.0
    pub deckkraft: f32,
    pub mischmodus: Mischmodus,
    /// Gleiche Größe und gleiches Format wie die Textur.
    pub pixel: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mischmodus {
    Normal = 0,
    Multiplizieren = 1,
    Addieren = 2,
}

impl Mischmodus {
    fn aus_u8(w: u8) -> Self {
        match w {
            1 => Self::Multiplizieren,
            2 => Self::Addieren,
            _ => Self::Normal, // Unbekannt → sicherer Standard
        }
    }
}

impl Textur {
    /// Neue Textur mit frischer ID.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn neu(
        name: impl Into<String>,
        breite: u32,
        hoehe: u32,
        pixel: Vec<u8>,
    ) -> Result<Self, TbrFehler> {
        let t = Self {
            id: AssetId::neu(),
            name: name.into(),
            breite,
            hoehe,
            pixel,
            ebenen: Vec::new(),
        };
        t.pruefen()?;
        Ok(t)
    }

    fn erwartete_groesse(&self) -> Result<usize, TbrFehler> {
        (self.breite as usize)
            .checked_mul(self.hoehe as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(TbrFehler::Kaputt("Textur ist zu groß"))
    }

    /// Prüft, ob Größe und Pixeldaten zusammenpassen.
    pub fn pruefen(&self) -> Result<(), TbrFehler> {
        if self.breite == 0 || self.hoehe == 0 {
            return Err(TbrFehler::Kaputt("Textur hat Breite oder Höhe 0"));
        }
        let n = self.erwartete_groesse()?;
        if self.pixel.len() != n {
            return Err(TbrFehler::Kaputt("Pixeldaten passen nicht zur Größe"));
        }
        if self.ebenen.iter().any(|e| e.pixel.len() != n) {
            return Err(TbrFehler::Kaputt("Ebene passt nicht zur Größe"));
        }
        Ok(())
    }

    pub fn zu_bytes(&self) -> Vec<u8> {
        let mut d = DateiSchreiber::new(AssetTyp::Textur, self.id, &self.name);

        let mut info = Schreiber::default();
        info.u32(self.breite);
        info.u32(self.hoehe);
        info.u8(FORMAT_RGBA8_SRGB);
        info.bytes(&[0, 0, 0]); // reserviert
        d.block(b"TINF", &info.daten);
        d.block(b"PIXL", &self.pixel);

        for e in &self.ebenen {
            let mut s = Schreiber::default();
            s.text(&e.name);
            s.u8(e.sichtbar as u8);
            s.f32(e.deckkraft);
            s.u8(e.mischmodus as u8);
            s.bytes(&e.pixel);
            d.block(b"EBNE", &s.daten);
        }
        d.fertig()
    }

    pub fn aus_bytes(daten: &[u8]) -> Result<Self, TbrFehler> {
        let (kopf, bloecke) = bloecke(daten, AssetTyp::Textur)?;
        let mut name = String::new();
        let mut groesse = None;
        let mut pixel = None;
        let mut ebenen = Vec::new();

        for b in &bloecke {
            match &b.name {
                b"NAME" => name = text(b)?,
                b"TINF" => {
                    let mut l = Leser::new(b.daten);
                    let (breite, hoehe, format) = (l.u32()?, l.u32()?, l.u8()?);
                    if format != FORMAT_RGBA8_SRGB {
                        return Err(TbrFehler::Kaputt("unbekanntes Pixelformat"));
                    }
                    groesse = Some((breite, hoehe));
                }
                b"PIXL" => pixel = Some(b.daten.to_vec()),
                b"EBNE" => {
                    let mut l = Leser::new(b.daten);
                    ebenen.push(Ebene {
                        name: l.text()?,
                        sichtbar: l.u8()? != 0,
                        deckkraft: l.f32()?,
                        mischmodus: Mischmodus::aus_u8(l.u8()?),
                        pixel: l.rest().to_vec(),
                    });
                }
                _ => {} // unbekannter Block → überspringen
            }
        }

        let (breite, hoehe) = groesse.ok_or(TbrFehler::FehlenderBlock("TINF"))?;
        let pixel = pixel.ok_or(TbrFehler::FehlenderBlock("PIXL"))?;
        let t = Self {
            id: kopf.id,
            name,
            breite,
            hoehe,
            pixel,
            ebenen,
        };
        t.pruefen()?;
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hin_und_zurueck() {
        let mut t = Textur::neu("Blatt", 2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
        t.ebenen.push(Ebene {
            name: "Adern".into(),
            sichtbar: true,
            deckkraft: 0.5,
            mischmodus: Mischmodus::Multiplizieren,
            pixel: vec![1; 8],
        });
        let wieder = Textur::aus_bytes(&t.zu_bytes()).unwrap();
        assert_eq!(wieder, t);
    }

    #[test]
    fn falsche_groesse_wird_abgelehnt() {
        assert!(Textur::neu("x", 2, 2, vec![0; 4]).is_err());
        assert!(Textur::neu("x", 0, 2, vec![]).is_err());
    }
}
