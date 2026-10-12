//! Meshes: Punkte und Flächen.
//!
//! Flächen sind **Vielecke** (3, 4 oder mehr Ecken), nicht nur Dreiecke – so verliert der
//! Mesh-Editor (Timbre Carve) beim Speichern keine Vierecke. Zum Zeichnen zerlegt
//! [`Mesh::dreiecke`] sie in Dreiecke.
//!
//! Normalen und Texturkoordinaten gehören zu den Punkten (gleiche Anzahl wie `positionen`)
//! oder fehlen ganz.

use crate::bytes::Schreiber;
use crate::{bloecke, f32_bytes, f32_liste, text, AssetId, AssetTyp, DateiSchreiber, TbrFehler};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub id: AssetId,
    pub name: String,
    pub positionen: Vec<[f32; 3]>,
    /// Leer oder gleich lang wie `positionen`.
    pub normalen: Vec<[f32; 3]>,
    /// Texturkoordinaten, leer oder gleich lang wie `positionen`.
    pub uvs: Vec<[f32; 2]>,
    /// Anzahl Ecken je Fläche.
    pub eckenzahl: Vec<u32>,
    /// Punkt-Indizes aller Flächen hintereinander.
    pub indizes: Vec<u32>,
}

impl Mesh {
    /// Leeres Mesh mit frischer ID.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn neu(name: impl Into<String>) -> Self {
        Self {
            id: AssetId::neu(),
            name: name.into(),
            ..Default::default()
        }
    }

    /// Fläche hinzufügen (Indizes auf `positionen`).
    pub fn flaeche(&mut self, ecken: &[u32]) {
        self.eckenzahl.push(ecken.len() as u32);
        self.indizes.extend_from_slice(ecken);
    }

    /// Alle Flächen als Listen von Punkt-Indizes.
    pub fn flaechen(&self) -> impl Iterator<Item = &[u32]> {
        let mut start = 0usize;
        self.eckenzahl.iter().map(move |&n| {
            let f = &self.indizes[start..start + n as usize];
            start += n as usize;
            f
        })
    }

    /// Alle Flächen als Dreiecke (je 3 Indizes). Vielecke werden als Fächer zerlegt –
    /// das stimmt für konvexe Flächen, und das sind fast alle in der Praxis.
    pub fn dreiecke(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.indizes.len() * 3);
        for f in self.flaechen() {
            for i in 1..f.len() - 1 {
                out.extend_from_slice(&[f[0], f[i], f[i + 1]]);
            }
        }
        out
    }

    /// Prüft, ob alles zusammenpasst.
    pub fn pruefen(&self) -> Result<(), TbrFehler> {
        let n = self.positionen.len();
        if !self.normalen.is_empty() && self.normalen.len() != n {
            return Err(TbrFehler::Kaputt("Anzahl Normalen passt nicht zu den Punkten"));
        }
        if !self.uvs.is_empty() && self.uvs.len() != n {
            return Err(TbrFehler::Kaputt("Anzahl Texturkoordinaten passt nicht zu den Punkten"));
        }
        if self.eckenzahl.iter().any(|&e| e < 3) {
            return Err(TbrFehler::Kaputt("Fläche mit weniger als 3 Ecken"));
        }
        let summe: u64 = self.eckenzahl.iter().map(|&e| e as u64).sum();
        if summe != self.indizes.len() as u64 {
            return Err(TbrFehler::Kaputt("Flächen und Indizes passen nicht zusammen"));
        }
        if self.indizes.iter().any(|&i| i as usize >= n) {
            return Err(TbrFehler::Kaputt("Index zeigt auf einen Punkt, den es nicht gibt"));
        }
        Ok(())
    }

    pub fn zu_bytes(&self) -> Vec<u8> {
        let mut d = DateiSchreiber::new(AssetTyp::Mesh, self.id, &self.name);
        d.block(b"POSI", &f32_bytes(self.positionen.as_flattened()));
        if !self.normalen.is_empty() {
            d.block(b"NORM", &f32_bytes(self.normalen.as_flattened()));
        }
        if !self.uvs.is_empty() {
            d.block(b"UVKO", &f32_bytes(self.uvs.as_flattened()));
        }
        let mut f = Schreiber::default();
        f.u32(self.eckenzahl.len() as u32);
        for &e in &self.eckenzahl {
            f.u32(e);
        }
        for &i in &self.indizes {
            f.u32(i);
        }
        d.block(b"FLAE", &f.daten);
        d.fertig()
    }

    pub fn aus_bytes(daten: &[u8]) -> Result<Self, TbrFehler> {
        let (kopf, bloecke) = bloecke(daten, AssetTyp::Mesh)?;
        let mut m = Mesh {
            id: kopf.id,
            ..Default::default()
        };
        let mut hat_punkte = false;
        let mut hat_flaechen = false;

        for b in &bloecke {
            match &b.name {
                b"NAME" => m.name = text(b)?,
                b"POSI" => {
                    m.positionen = gruppen(&f32_liste(b.daten)?)?;
                    hat_punkte = true;
                }
                b"NORM" => m.normalen = gruppen(&f32_liste(b.daten)?)?,
                b"UVKO" => m.uvs = gruppen(&f32_liste(b.daten)?)?,
                b"FLAE" => {
                    let zahlen: Vec<u32> = u32_liste(b.daten)?;
                    let (&anzahl, rest) = zahlen.split_first().ok_or(TbrFehler::Kaputt("FLAE ist leer"))?;
                    let anzahl = anzahl as usize;
                    if rest.len() < anzahl {
                        return Err(TbrFehler::Kaputt("FLAE ist zu kurz"));
                    }
                    m.eckenzahl = rest[..anzahl].to_vec();
                    m.indizes = rest[anzahl..].to_vec();
                    hat_flaechen = true;
                }
                _ => {}
            }
        }
        if !hat_punkte {
            return Err(TbrFehler::FehlenderBlock("POSI"));
        }
        if !hat_flaechen {
            return Err(TbrFehler::FehlenderBlock("FLAE"));
        }
        m.pruefen()?;
        Ok(m)
    }
}

fn gruppen<const N: usize>(werte: &[f32]) -> Result<Vec<[f32; N]>, TbrFehler> {
    if !werte.len().is_multiple_of(N) {
        return Err(TbrFehler::Kaputt("Zahlenliste passt nicht zur Gruppengröße"));
    }
    Ok(werte.chunks_exact(N).map(|c| c.try_into().unwrap()).collect())
}

fn u32_liste(daten: &[u8]) -> Result<Vec<u32>, TbrFehler> {
    if !daten.len().is_multiple_of(4) {
        return Err(TbrFehler::Kaputt("Zahlenliste hat falsche Länge"));
    }
    Ok(daten
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Würfel aus 6 Vierecken.
    fn wuerfel() -> Mesh {
        let mut m = Mesh::neu("Würfel");
        for z in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for x in [-1.0, 1.0] {
                    m.positionen.push([x, y, z]);
                }
            }
        }
        for f in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            m.flaeche(&f);
        }
        m
    }

    #[test]
    fn hin_und_zurueck() {
        let mut m = wuerfel();
        m.uvs = vec![[0.5, 0.5]; 8];
        let wieder = Mesh::aus_bytes(&m.zu_bytes()).unwrap();
        assert_eq!(wieder, m);
        assert_eq!(wieder.flaechen().count(), 6);
    }

    #[test]
    fn vierecke_werden_zu_zwei_dreiecken() {
        assert_eq!(wuerfel().dreiecke().len(), 6 * 2 * 3);
    }

    #[test]
    fn falscher_index_wird_abgelehnt() {
        let mut m = wuerfel();
        m.flaeche(&[0, 1, 99]);
        assert!(m.pruefen().is_err());
        assert!(Mesh::aus_bytes(&m.zu_bytes()).is_err());
    }
}
