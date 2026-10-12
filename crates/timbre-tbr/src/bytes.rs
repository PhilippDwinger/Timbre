//! Kleine Helfer zum Lesen und Schreiben von Zahlen (Little Endian).

use crate::TbrFehler;

pub(crate) struct Leser<'a> {
    daten: &'a [u8],
    pos: usize,
}

impl<'a> Leser<'a> {
    pub fn new(daten: &'a [u8]) -> Self {
        Self { daten, pos: 0 }
    }

    pub fn leer(&self) -> bool {
        self.pos >= self.daten.len()
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], TbrFehler> {
        let ende = self.pos.checked_add(n).ok_or(TbrFehler::Abgeschnitten)?;
        let teil = self.daten.get(self.pos..ende).ok_or(TbrFehler::Abgeschnitten)?;
        self.pos = ende;
        Ok(teil)
    }

    pub fn u8(&mut self) -> Result<u8, TbrFehler> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, TbrFehler> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32, TbrFehler> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn f32(&mut self) -> Result<f32, TbrFehler> {
        Ok(f32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    /// Text mit vorangestellter Länge (u32).
    pub fn text(&mut self) -> Result<String, TbrFehler> {
        let n = self.u32()? as usize;
        String::from_utf8(self.bytes(n)?.to_vec()).map_err(|_| TbrFehler::Kaputt("Text ist kein gültiges UTF-8"))
    }

    /// Restliche Bytes.
    pub fn rest(&mut self) -> &'a [u8] {
        let r = &self.daten[self.pos.min(self.daten.len())..];
        self.pos = self.daten.len();
        r
    }
}

#[derive(Default)]
pub(crate) struct Schreiber {
    pub daten: Vec<u8>,
}

impl Schreiber {
    pub fn bytes(&mut self, b: &[u8]) {
        self.daten.extend_from_slice(b);
    }

    pub fn u8(&mut self, v: u8) {
        self.daten.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn text(&mut self, t: &str) {
        self.u32(t.len() as u32);
        self.bytes(t.as_bytes());
    }
}
