//! Das Timbre-Asset-Format `.tbr`.
//!
//! Eine `.tbr`-Datei enthält genau ein Asset: ein [`Mesh`], eine [`Textur`] oder eine [`Szene`].
//! Aufbau (alle Zahlen Little Endian, Details in `docs/tbr-format.md`):
//!
//! ```text
//! Kopf (32 Bytes):  Kennung "\x89TBR\r\n\x1a\n" | Version u16 | Typ u8 | Flags u8 | reserviert u32 | Asset-ID 16 Bytes
//! Blöcke:           Name [4 ASCII] | Länge u32 | Daten …   (beliebig viele, unbekannte werden übersprungen)
//! ```
//!
//! Dieses Paket hat keine Abhängigkeiten und kennt weder Grafikkarte noch Fenster –
//! so können auch Werkzeuge, Grove oder Bridge `.tbr` lesen.

mod bytes;
mod fehler;
mod id;
mod mesh;
mod szene;
mod textur;

pub use fehler::TbrFehler;
pub use id::AssetId;
pub use mesh::Mesh;
pub use szene::Szene;
pub use textur::{Ebene, Mischmodus, Textur};

use bytes::{Leser, Schreiber};

/// Erkennungszeichen am Dateianfang. Das `\x89` und `\r\n\x1a\n` (Trick von PNG) machen
/// sichtbar, wenn die Datei versehentlich als Text übertragen und dabei verändert wurde.
pub const KENNUNG: [u8; 8] = *b"\x89TBR\r\n\x1a\n";

/// Version des Formats. 0 = Entwurf (Meilenstein 3); ab 1 gilt: ältere Dateien bleiben lesbar.
pub const FORMAT_VERSION: u16 = 0;

const KOPF_GROESSE: usize = 32;

/// Welche Art Asset in der Datei steckt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AssetTyp {
    Mesh = 1,
    Textur = 2,
    Szene = 3,
}

impl AssetTyp {
    fn aus_u8(wert: u8) -> Result<Self, TbrFehler> {
        match wert {
            1 => Ok(Self::Mesh),
            2 => Ok(Self::Textur),
            3 => Ok(Self::Szene),
            andere => Err(TbrFehler::UnbekannterTyp(andere)),
        }
    }
}

/// Was im Kopf jeder Datei steht.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kopf {
    pub version: u16,
    pub typ: AssetTyp,
    pub id: AssetId,
}

/// Ein beliebiges Asset, wenn man vorher nicht weiß, was in der Datei steckt.
#[derive(Clone, Debug, PartialEq)]
pub enum Asset {
    Mesh(Mesh),
    Textur(Textur),
    Szene(Szene),
}

impl Asset {
    pub fn id(&self) -> AssetId {
        match self {
            Asset::Mesh(m) => m.id,
            Asset::Textur(t) => t.id,
            Asset::Szene(s) => s.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Asset::Mesh(m) => &m.name,
            Asset::Textur(t) => &t.name,
            Asset::Szene(s) => &s.name,
        }
    }
}

/// Liest eine `.tbr`-Datei, egal welcher Typ.
pub fn lesen(daten: &[u8]) -> Result<Asset, TbrFehler> {
    let kopf = kopf_lesen(daten)?;
    Ok(match kopf.typ {
        AssetTyp::Mesh => Asset::Mesh(Mesh::aus_bytes(daten)?),
        AssetTyp::Textur => Asset::Textur(Textur::aus_bytes(daten)?),
        AssetTyp::Szene => Asset::Szene(Szene::aus_bytes(daten)?),
    })
}

/// Liest nur den Kopf (schnell, z. B. um eine Asset-Liste aufzubauen).
pub fn kopf_lesen(daten: &[u8]) -> Result<Kopf, TbrFehler> {
    if daten.len() < KOPF_GROESSE || daten[..8] != KENNUNG {
        return Err(TbrFehler::KeineTbrDatei);
    }
    let mut l = Leser::new(&daten[8..KOPF_GROESSE]);
    let version = l.u16()?;
    if version > FORMAT_VERSION {
        return Err(TbrFehler::ZuNeu(version));
    }
    let typ = AssetTyp::aus_u8(l.u8()?)?;
    let _flags = l.u8()?;
    let _reserviert = l.u32()?;
    let id = AssetId::aus_bytes(l.bytes(16)?.try_into().unwrap());
    Ok(Kopf { version, typ, id })
}

// ---------------------------------------------------------------------------
// Blöcke: gemeinsame Bausteine für Mesh, Textur und Szene
// ---------------------------------------------------------------------------

/// Ein Block aus der Datei: Name und Daten.
struct Block<'a> {
    name: [u8; 4],
    daten: &'a [u8],
}

/// Kopf prüfen und alle Blöcke einer Datei auflisten.
fn bloecke(daten: &[u8], erwartet: AssetTyp) -> Result<(Kopf, Vec<Block<'_>>), TbrFehler> {
    let kopf = kopf_lesen(daten)?;
    if kopf.typ != erwartet {
        return Err(TbrFehler::FalscherTyp {
            erwartet,
            gefunden: kopf.typ,
        });
    }
    let mut l = Leser::new(&daten[KOPF_GROESSE..]);
    let mut liste = Vec::new();
    while !l.leer() {
        let name: [u8; 4] = l.bytes(4)?.try_into().unwrap();
        let laenge = l.u32()? as usize;
        let daten = l.bytes(laenge)?;
        liste.push(Block { name, daten });
    }
    Ok((kopf, liste))
}

/// Schreibt Kopf und Blöcke zu einer fertigen Datei.
struct DateiSchreiber {
    s: Schreiber,
}

impl DateiSchreiber {
    fn new(typ: AssetTyp, id: AssetId, name: &str) -> Self {
        let mut s = Schreiber::default();
        s.bytes(&KENNUNG);
        s.u16(FORMAT_VERSION);
        s.u8(typ as u8);
        s.u8(0); // Flags (frei für später, z. B. Kompression)
        s.u32(0); // reserviert
        s.bytes(&id.zu_bytes());
        let mut d = Self { s };
        if !name.is_empty() {
            d.block(b"NAME", name.as_bytes());
        }
        d
    }

    fn block(&mut self, name: &[u8; 4], daten: &[u8]) {
        self.s.bytes(name);
        self.s
            .u32(u32::try_from(daten.len()).expect("Block größer als 4 GB"));
        self.s.bytes(daten);
    }

    fn fertig(self) -> Vec<u8> {
        self.s.daten
    }
}

fn text(block: &Block) -> Result<String, TbrFehler> {
    String::from_utf8(block.daten.to_vec())
        .map_err(|_| TbrFehler::Kaputt("Text ist kein gültiges UTF-8"))
}

fn f32_liste(daten: &[u8]) -> Result<Vec<f32>, TbrFehler> {
    if !daten.len().is_multiple_of(4) {
        return Err(TbrFehler::Kaputt("Zahlenliste hat falsche Länge"));
    }
    Ok(daten
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect())
}

fn f32_bytes(werte: &[f32]) -> Vec<u8> {
    werte.iter().flat_map(|w| w.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_fremde_dateien() {
        assert_eq!(kopf_lesen(b"hallo"), Err(TbrFehler::KeineTbrDatei));
        assert_eq!(kopf_lesen(&[0u8; 64]), Err(TbrFehler::KeineTbrDatei));
    }

    #[test]
    fn ueberspringt_unbekannte_bloecke() {
        let id = AssetId::aus_bytes([7; 16]);
        let mut d = DateiSchreiber::new(AssetTyp::Szene, id, "Test");
        d.block(b"ZUKU", b"etwas aus einer spaeteren Version");
        d.block(b"TEXT", b"inhalt");
        let szene = Szene::aus_bytes(&d.fertig()).unwrap();
        assert_eq!(szene.text, "inhalt");
        assert_eq!(szene.name, "Test");
    }

    #[test]
    fn falscher_typ_wird_gemeldet() {
        let s = Szene {
            id: AssetId::aus_bytes([1; 16]),
            name: String::new(),
            text: String::new(),
        };
        let fehler = Textur::aus_bytes(&s.zu_bytes()).unwrap_err();
        assert_eq!(
            fehler,
            TbrFehler::FalscherTyp {
                erwartet: AssetTyp::Textur,
                gefunden: AssetTyp::Szene
            }
        );
    }

    #[test]
    fn abgeschnittene_datei_wird_erkannt() {
        let s = Szene {
            id: AssetId::aus_bytes([1; 16]),
            name: "x".into(),
            text: "inhalt".into(),
        };
        let bytes = s.zu_bytes();
        assert!(Szene::aus_bytes(&bytes[..bytes.len() - 3]).is_err());
    }
}
