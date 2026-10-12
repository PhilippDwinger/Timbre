//! Fester Zeitschritt ("fixed timestep").
//!
//! Die Spiellogik läuft immer in gleich großen Schritten (z. B. 60 pro Sekunde),
//! egal wie schnell gezeichnet wird. Dadurch verhält sich das Spiel auf jedem
//! Rechner gleich. Zwischen zwei Logik-Schritten wird beim Zeichnen
//! interpoliert (`alpha`), damit es auch auf 144-Hz-Monitoren flüssig aussieht.

use web_time::Instant;

/// Längster Frame, der berücksichtigt wird. Verhindert, dass nach einem Hänger
/// hunderte Logik-Schritte auf einmal nachgeholt werden ("spiral of death").
const MAX_FRAME: f64 = 0.25;

pub struct FesterTakt {
    schritt: f64,
    akku: f64,
    letzte: Option<Instant>,
}

impl FesterTakt {
    /// `hz` = Logik-Schritte pro Sekunde.
    pub fn new(hz: u32) -> Self {
        Self {
            schritt: 1.0 / hz as f64,
            akku: 0.0,
            letzte: None,
        }
    }

    /// Einmal pro gezeichnetem Frame aufrufen. Liefert, wie viele Logik-Schritte jetzt fällig sind.
    pub fn frame(&mut self) -> u32 {
        let jetzt = Instant::now();
        let vergangen = match self.letzte {
            Some(l) => jetzt.duration_since(l).as_secs_f64(),
            None => 0.0,
        };
        self.letzte = Some(jetzt);
        self.vorruecken(vergangen)
    }

    /// Wie `frame`, aber mit vorgegebener Zeit (für Tests).
    pub fn vorruecken(&mut self, sekunden: f64) -> u32 {
        self.akku += sekunden.min(MAX_FRAME);
        let mut schritte = 0;
        while self.akku >= self.schritt {
            self.akku -= self.schritt;
            schritte += 1;
        }
        schritte
    }

    /// Dauer eines Logik-Schritts in Sekunden.
    pub fn dt(&self) -> f32 {
        self.schritt as f32
    }

    /// Wie weit wir zwischen letztem und nächstem Logik-Schritt sind (0..1).
    pub fn alpha(&self) -> f32 {
        (self.akku / self.schritt) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaehlt_schritte() {
        let mut t = FesterTakt::new(60);
        assert_eq!(t.vorruecken(1.0 / 60.0 + 1e-9), 1);
        assert_eq!(t.vorruecken(0.1), 6);
    }

    #[test]
    fn begrenzt_haenger() {
        let mut t = FesterTakt::new(60);
        // 10 Sekunden Hänger → höchstens 0,25 s werden nachgeholt.
        assert_eq!(t.vorruecken(10.0), 15);
    }

    #[test]
    fn alpha_zwischen_null_und_eins() {
        let mut t = FesterTakt::new(60);
        t.vorruecken(1.5 / 60.0);
        assert!((t.alpha() - 0.5).abs() < 1e-3);
    }
}
