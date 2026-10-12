//! Szenen: lesbarer Text in einer `.tbr`-Hülle.
//!
//! Der genaue Inhalt (welche Objekte, Positionen, Verweise per [`crate::AssetId`])
//! wird mit dem ECS in Meilenstein 4 festgelegt. Bis dahin ist eine Szene einfach Text.

use crate::{bloecke, text, AssetId, AssetTyp, DateiSchreiber, TbrFehler};

#[derive(Clone, Debug, PartialEq)]
pub struct Szene {
    pub id: AssetId,
    pub name: String,
    pub text: String,
}

impl Szene {
    pub fn zu_bytes(&self) -> Vec<u8> {
        let mut d = DateiSchreiber::new(AssetTyp::Szene, self.id, &self.name);
        d.block(b"TEXT", self.text.as_bytes());
        d.fertig()
    }

    pub fn aus_bytes(daten: &[u8]) -> Result<Self, TbrFehler> {
        let (kopf, bloecke) = bloecke(daten, AssetTyp::Szene)?;
        let mut name = String::new();
        let mut inhalt = None;
        for b in &bloecke {
            match &b.name {
                b"NAME" => name = text(b)?,
                b"TEXT" => inhalt = Some(text(b)?),
                _ => {}
            }
        }
        Ok(Self {
            id: kopf.id,
            name,
            text: inhalt.ok_or(TbrFehler::FehlenderBlock("TEXT"))?,
        })
    }
}
