//! `tbr` – Werkzeug für Timbre-Asset-Dateien.
//!
//!   tbr info  datei.tbr                  Zeigt, was in einer .tbr-Datei steckt
//!   tbr textur bild.png  [ziel.tbr]      PNG → Textur
//!   tbr mesh  modell.glb [ziel.tbr]      glTF/GLB (z. B. aus Blender) → Mesh
//!   tbr text  szene.tbr                  Inhalt einer Szene als Text ausgeben
//!   tbr szene szene.json [ziel.tbr]      Szenen-Text (JSON) → Szene
//!
//! Wird in eine bestehende .tbr-Datei gleichen Typs importiert, bleibt deren Asset-ID
//! erhalten – Szenen, die darauf verweisen, funktionieren also weiter.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use glam::{Mat3, Mat4, Vec3};
use timbre_tbr::{Asset, AssetId, AssetTyp, Mesh, Textur};

type Ergebnis<T> = Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ergebnis = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["info", datei] => info(Path::new(datei)),
        ["text", datei] => szenen_text(Path::new(datei)),
        ["szene", quelle] => szene(Path::new(quelle), &ziel_neben(quelle)),
        ["szene", quelle, ziel] => szene(Path::new(quelle), Path::new(ziel)),
        ["textur", quelle] => textur(Path::new(quelle), &ziel_neben(quelle)),
        ["textur", quelle, ziel] => textur(Path::new(quelle), Path::new(ziel)),
        ["mesh", quelle] => mesh(Path::new(quelle), &ziel_neben(quelle)),
        ["mesh", quelle, ziel] => mesh(Path::new(quelle), Path::new(ziel)),
        _ => {
            hilfe();
            return ExitCode::from(2);
        }
    };
    match ergebnis {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fehler: {e}");
            ExitCode::FAILURE
        }
    }
}

fn hilfe() {
    println!(
        "tbr {} – Werkzeug für Timbre-Assets\n\n\
         Benutzung:\n  \
         tbr info   datei.tbr              Inhalt anzeigen\n  \
         tbr textur bild.png  [ziel.tbr]   PNG in Textur umwandeln\n  \
         tbr mesh   modell.glb [ziel.tbr]  glTF/GLB in Mesh umwandeln\n  \
         tbr text   szene.tbr              Inhalt einer Szene anzeigen\n  \
         tbr szene  szene.json [ziel.tbr]  Szenen-Text (JSON) in Szene umwandeln\n\n\
         Ohne Ziel wird neben der Quelle eine .tbr-Datei mit gleichem Namen angelegt.",
        env!("CARGO_PKG_VERSION")
    );
}

/// `bilder/blatt.png` → `bilder/blatt.tbr`
fn ziel_neben(quelle: &str) -> PathBuf {
    Path::new(quelle).with_extension("tbr")
}

fn name_aus(pfad: &Path) -> String {
    pfad.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn lesen(pfad: &Path) -> Ergebnis<Vec<u8>> {
    std::fs::read(pfad).map_err(|e| format!("{} kann nicht gelesen werden: {e}", pfad.display()))
}

fn schreiben(pfad: &Path, daten: &[u8]) -> Ergebnis<()> {
    std::fs::write(pfad, daten).map_err(|e| format!("{} kann nicht geschrieben werden: {e}", pfad.display()))
}

/// ID einer bestehenden Zieldatei gleichen Typs übernehmen, sonst neue ID.
fn id_fuer(ziel: &Path, typ: AssetTyp) -> AssetId {
    if let Ok(daten) = std::fs::read(ziel) {
        if let Ok(kopf) = timbre_tbr::kopf_lesen(&daten) {
            if kopf.typ == typ {
                println!("  bestehende Datei: Asset-ID {} bleibt erhalten", kopf.id);
                return kopf.id;
            }
        }
    }
    AssetId::neu()
}

// ---------------------------------------------------------------------------

fn info(pfad: &Path) -> Ergebnis<()> {
    let daten = lesen(pfad)?;
    let kopf = timbre_tbr::kopf_lesen(&daten).map_err(|e| e.to_string())?;
    let asset = timbre_tbr::lesen(&daten).map_err(|e| e.to_string())?;
    println!("{}", pfad.display());
    println!("  Typ:      {:?}", kopf.typ);
    println!("  Name:     {}", asset.name());
    println!("  ID:       {}", kopf.id);
    println!("  Version:  {}", kopf.version);
    println!("  Größe:    {} Bytes", daten.len());
    match asset {
        Asset::Textur(t) => {
            println!("  Pixel:    {} × {}", t.breite, t.hoehe);
            println!("  Ebenen:   {}", t.ebenen.len());
        }
        Asset::Mesh(m) => {
            println!("  Punkte:   {}", m.positionen.len());
            println!(
                "  Flächen:  {} ({} Dreiecke)",
                m.eckenzahl.len(),
                m.dreiecke().len() / 3
            );
            println!("  Normalen: {}", if m.normalen.is_empty() { "nein" } else { "ja" });
            println!("  UVs:      {}", if m.uvs.is_empty() { "nein" } else { "ja" });
        }
        Asset::Szene(s) => println!("  Zeilen:   {}", s.text.lines().count()),
    }
    Ok(())
}

fn szenen_text(pfad: &Path) -> Ergebnis<()> {
    let szene = timbre_tbr::Szene::aus_bytes(&lesen(pfad)?).map_err(|e| e.to_string())?;
    // Fehler beim Ausgeben ignorieren (z. B. wenn die Ausgabe vorzeitig abgebrochen wird).
    use std::io::Write;
    let _ = writeln!(std::io::stdout().lock(), "{}", szene.text);
    Ok(())
}

fn szene(quelle: &Path, ziel: &Path) -> Ergebnis<()> {
    println!("{} → {}", quelle.display(), ziel.display());
    let text = String::from_utf8(lesen(quelle)?).map_err(|_| "Datei ist kein UTF-8-Text")?;
    let s = timbre_tbr::Szene {
        id: id_fuer(ziel, AssetTyp::Szene),
        name: name_aus(ziel),
        text,
    };
    schreiben(ziel, &s.zu_bytes())?;
    println!("  fertig: {} Zeilen, ID {}", s.text.lines().count(), s.id);
    Ok(())
}

// ---------------------------------------------------------------------------

fn textur(quelle: &Path, ziel: &Path) -> Ergebnis<()> {
    println!("{} → {}", quelle.display(), ziel.display());
    let (breite, hoehe, pixel) = png_lesen(quelle)?;
    let mut t = Textur::neu(name_aus(quelle), breite, hoehe, pixel).map_err(|e| e.to_string())?;
    t.id = id_fuer(ziel, AssetTyp::Textur);
    schreiben(ziel, &t.zu_bytes())?;
    println!("  fertig: {breite} × {hoehe} Pixel, ID {}", t.id);
    Ok(())
}

/// Liest ein PNG beliebiger Art (Graustufen, Palette, 16 Bit …) als RGBA mit 8 Bit.
fn png_lesen(pfad: &Path) -> Ergebnis<(u32, u32, Vec<u8>)> {
    let datei = std::fs::File::open(pfad).map_err(|e| format!("{} kann nicht geöffnet werden: {e}", pfad.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(datei));
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

// ---------------------------------------------------------------------------

fn mesh(quelle: &Path, ziel: &Path) -> Ergebnis<()> {
    println!("{} → {}", quelle.display(), ziel.display());
    let (dokument, puffer, _bilder) =
        gltf::import(quelle).map_err(|e| format!("glTF kann nicht gelesen werden: {e}"))?;

    let mut m = Mesh::neu(name_aus(quelle));
    m.id = id_fuer(ziel, AssetTyp::Mesh);
    let mut hat_normalen = true;
    let mut hat_uvs = true;
    let mut normalen = Vec::new();
    let mut uvs = Vec::new();

    // Alle Knoten der Szene durchgehen und ihre Meshes mit Position/Drehung/Größe übernehmen.
    let szene = dokument
        .default_scene()
        .or_else(|| dokument.scenes().next())
        .ok_or("glTF enthält keine Szene")?;
    let mut stapel: Vec<(gltf::Node, Mat4)> = szene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
    while let Some((knoten, eltern)) = stapel.pop() {
        let welt = eltern * Mat4::from_cols_array_2d(&knoten.transform().matrix());
        stapel.extend(knoten.children().map(|k| (k, welt)));
        let Some(gltf_mesh) = knoten.mesh() else {
            continue;
        };
        let normal_matrix = Mat3::from_mat4(welt).inverse().transpose();

        for primitiv in gltf_mesh.primitives() {
            if primitiv.mode() != gltf::mesh::Mode::Triangles {
                println!(
                    "  übersprungen: Teil mit Modus {:?} (nur Dreiecke werden übernommen)",
                    primitiv.mode()
                );
                continue;
            }
            let leser = primitiv.reader(|b| Some(&puffer[b.index()]));
            let Some(pos) = leser.read_positions() else {
                continue;
            };
            let start = m.positionen.len() as u32;
            m.positionen
                .extend(pos.map(|p| welt.transform_point3(Vec3::from(p)).to_array()));
            let anzahl = m.positionen.len() - start as usize;

            match leser.read_normals() {
                Some(n) => normalen.extend(n.map(|n| (normal_matrix * Vec3::from(n)).normalize_or_zero().to_array())),
                None => hat_normalen = false,
            }
            match leser.read_tex_coords(0) {
                Some(t) => uvs.extend(t.into_f32()),
                None => hat_uvs = false,
            }

            let indizes: Vec<u32> = match leser.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..anzahl as u32).collect(),
            };
            for d in indizes.chunks_exact(3) {
                m.flaeche(&[start + d[0], start + d[1], start + d[2]]);
            }
        }
    }

    if m.positionen.is_empty() {
        return Err("glTF enthält keine Dreiecks-Meshes".into());
    }
    // Nur übernehmen, wenn ALLE Teile Normalen bzw. UVs hatten.
    if hat_normalen {
        m.normalen = normalen;
    }
    if hat_uvs {
        m.uvs = uvs;
    }
    m.pruefen().map_err(|e| e.to_string())?;
    schreiben(ziel, &m.zu_bytes())?;
    println!(
        "  fertig: {} Punkte, {} Dreiecke, Normalen {}, UVs {}, ID {}",
        m.positionen.len(),
        m.eckenzahl.len(),
        if m.normalen.is_empty() { "nein" } else { "ja" },
        if m.uvs.is_empty() { "nein" } else { "ja" },
        m.id
    );
    println!("  Hinweis: glTF kennt nur Dreiecke – Vierecke aus Blender kommen als je zwei Dreiecke an.");
    Ok(())
}
