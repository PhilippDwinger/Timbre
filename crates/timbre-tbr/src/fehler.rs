use std::fmt;

use crate::AssetTyp;

/// Alles, was beim Lesen einer `.tbr`-Datei schiefgehen kann.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TbrFehler {
    /// Die Datei beginnt nicht mit der `.tbr`-Kennung.
    KeineTbrDatei,
    /// Die Datei stammt aus einer neueren Timbre-Version.
    ZuNeu(u16),
    UnbekannterTyp(u8),
    FalscherTyp {
        erwartet: AssetTyp,
        gefunden: AssetTyp,
    },
    /// Die Datei endet mitten in einem Block.
    Abgeschnitten,
    /// Ein Pflicht-Block fehlt.
    FehlenderBlock(&'static str),
    /// Inhalt ist unstimmig (z. B. Index zeigt auf einen Punkt, den es nicht gibt).
    Kaputt(&'static str),
}

impl fmt::Display for TbrFehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeineTbrDatei => write!(f, "keine .tbr-Datei"),
            Self::ZuNeu(v) => write!(f, "Formatversion {v} ist neuer als diese Timbre-Version"),
            Self::UnbekannterTyp(t) => write!(f, "unbekannter Asset-Typ {t}"),
            Self::FalscherTyp { erwartet, gefunden } => {
                write!(f, "erwartet {erwartet:?}, Datei enthält {gefunden:?}")
            }
            Self::Abgeschnitten => write!(f, "Datei ist abgeschnitten"),
            Self::FehlenderBlock(b) => write!(f, "Pflicht-Block {b} fehlt"),
            Self::Kaputt(grund) => write!(f, "Datei ist beschädigt: {grund}"),
        }
    }
}

impl std::error::Error for TbrFehler {}
