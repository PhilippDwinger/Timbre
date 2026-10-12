use std::fmt;

/// Eindeutige, feste ID eines Assets (128 Bit).
///
/// Szenen verweisen über diese ID auf Meshes und Texturen – nicht über den Dateinamen.
/// Dateien dürfen also umbenannt und verschoben werden (später auch auf den Homeserver).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId([u8; 16]);

impl AssetId {
    pub const fn aus_bytes(b: [u8; 16]) -> Self {
        Self(b)
    }

    pub fn zu_bytes(&self) -> [u8; 16] {
        self.0
    }

    /// Erzeugt eine neue, zufällige ID.
    ///
    /// Nur nativ verfügbar – im Browser werden (noch) keine Assets erzeugt, nur geladen.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn neu() -> Self {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static ZAEHLER: AtomicU64 = AtomicU64::new(0);
        let zeit = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let nr = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        // RandomState wird vom Betriebssystem zufällig initialisiert.
        let haelfte = |salz: u64| {
            let mut h = RandomState::new().build_hasher();
            h.write_u128(zeit);
            h.write_u64(nr);
            h.write_u64(salz);
            h.finish()
        };
        let mut b = [0u8; 16];
        b[..8].copy_from_slice(&haelfte(1).to_le_bytes());
        b[8..].copy_from_slice(&haelfte(2).to_le_bytes());
        Self(b)
    }

    /// Als 32 Hex-Zeichen, z. B. `3f2a…`.
    pub fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Gegenstück zu [`AssetId::hex`].
    pub fn aus_hex(text: &str) -> Option<Self> {
        if text.len() != 32 {
            return None;
        }
        let mut b = [0u8; 16];
        for (i, ziel) in b.iter_mut().enumerate() {
            *ziel = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
        }
        Some(Self(b))
    }
}

/// Die "leere" ID (alles 0) – nur als Platzhalter, echte Assets bekommen [`AssetId::neu`].
impl Default for AssetId {
    fn default() -> Self {
        AssetId::aus_bytes([0; 16])
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({})", self.hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_sind_verschieden() {
        assert_ne!(AssetId::neu(), AssetId::neu());
    }

    #[test]
    fn hex_hin_und_zurueck() {
        let id = AssetId::neu();
        assert_eq!(AssetId::aus_hex(&id.hex()), Some(id));
        assert_eq!(AssetId::aus_hex("kaputt"), None);
    }
}
