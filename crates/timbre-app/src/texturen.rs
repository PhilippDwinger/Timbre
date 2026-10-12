//! Kleine Texturen, die per Code erzeugt werden – bis es das Asset-Format `.tbr` gibt
//! (Meilenstein 3). Beide sind hell/grau, damit man sie per `Sprite::farbe` einfärben kann.

/// Baumscheibe mit Jahresringen.
pub fn jahresringe(groesse: u32) -> Vec<u8> {
    pixel(groesse, |x, y| {
        let r = (x * x + y * y).sqrt();
        if r > 1.0 {
            return [0.0; 4];
        }
        let ringe = 0.80 + 0.20 * (r * 34.0).cos();
        let rinde = if r > 0.88 { 0.55 } else { 1.0 };
        let hell = ringe * rinde;
        [hell, hell, hell, kante(1.0 - r, groesse)]
    })
}

/// Blatt mit Mittelader.
pub fn blatt(groesse: u32) -> Vec<u8> {
    pixel(groesse, |x, y| {
        // Blattform: in der Mitte breit, oben und unten spitz.
        let breite = 0.55 * (1.0 - y * y).max(0.0);
        let abstand = breite - x.abs();
        if abstand <= 0.0 || y.abs() > 0.97 {
            return [0.0; 4];
        }
        let ader = if x.abs() < 0.04 { 0.65 } else { 1.0 };
        let seitenadern = 0.92 + 0.08 * ((y * 10.0 + x.abs() * 6.0).sin());
        let hell = ader * seitenadern;
        [hell, hell, hell, kante(abstand, groesse)]
    })
}

/// Weiche Kante (Antialiasing): ~1 Pixel breiter Übergang.
fn kante(abstand: f32, groesse: u32) -> f32 {
    (abstand * groesse as f32 * 0.5).clamp(0.0, 1.0)
}

/// Ruft `f` für jedes Pixel mit Koordinaten -1..1 auf und baut RGBA-Bytes.
fn pixel(groesse: u32, f: impl Fn(f32, f32) -> [f32; 4]) -> Vec<u8> {
    let mut daten = Vec::with_capacity((groesse * groesse * 4) as usize);
    for py in 0..groesse {
        for px in 0..groesse {
            let x = (px as f32 + 0.5) / groesse as f32 * 2.0 - 1.0;
            let y = (py as f32 + 0.5) / groesse as f32 * 2.0 - 1.0;
            for kanal in f(x, y) {
                daten.push((kanal.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    daten
}
