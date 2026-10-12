# Das `.tbr`-Format (Formatversion 0)

Eine `.tbr`-Datei enthält genau **ein Asset**: ein Mesh, eine Textur oder eine Szene.
Alle Zahlen sind **Little Endian**. Referenz-Implementierung: `crates/timbre-tbr`.

## Kopf (32 Bytes)

| Bytes | Inhalt |
|---|---|
| 0–7 | Kennung `89 54 42 52 0D 0A 1A 0A` (`\x89TBR\r\n\x1a\n`) |
| 8–9 | Formatversion, `u16` (derzeit 0) |
| 10 | Asset-Typ, `u8`: 1 = Mesh, 2 = Textur, 3 = Szene |
| 11 | Flags, `u8` (derzeit 0; reserviert z. B. für Kompression) |
| 12–15 | reserviert, `u32` = 0 |
| 16–31 | Asset-ID, 16 Bytes |

Die Kennung funktioniert wie bei PNG: Wird die Datei versehentlich als Text übertragen
(Zeilenenden umgewandelt), stimmt sie nicht mehr und der Fehler fällt sofort auf.

## Blöcke

Nach dem Kopf folgen beliebig viele Blöcke bis zum Dateiende:

| Bytes | Inhalt |
|---|---|
| 4 | Blockname, 4 ASCII-Zeichen (z. B. `PIXL`) |
| 4 | Länge der Daten in Bytes, `u32` |
| n | Daten |

**Regel:** Unbekannte Blöcke werden übersprungen. So können neue Versionen Blöcke ergänzen,
ohne dass ältere Timbre-Versionen an der Datei scheitern.

Gemeinsam für alle Typen:

| Block | Pflicht | Inhalt |
|---|---|---|
| `NAME` | nein | Anzeigename, UTF-8 |

## Textur (Typ 2)

| Block | Pflicht | Inhalt |
|---|---|---|
| `TINF` | ja | Breite `u32`, Höhe `u32`, Pixelformat `u8` (0 = RGBA 8 Bit sRGB), 3 Bytes reserviert |
| `PIXL` | ja | Fertiges Bild: Breite × Höhe × 4 Bytes, Zeile für Zeile ab links oben |
| `EBNE` | nein, mehrfach | Eine Ebene (von unten nach oben): Name (`u32` Länge + UTF-8), sichtbar `u8`, Deckkraft `f32` (0–1), Mischmodus `u8` (0 Normal, 1 Multiplizieren, 2 Addieren), danach Pixel wie `PIXL` |

Die Engine lädt nur `PIXL`. Ebenen braucht nur der Mal-Editor (Timbre Stain).

## Mesh (Typ 1)

| Block | Pflicht | Inhalt |
|---|---|---|
| `POSI` | ja | Punkte: je 3 × `f32` (x, y, z) |
| `NORM` | nein | Normalen: je 3 × `f32`, gleiche Anzahl wie Punkte |
| `UVKO` | nein | Texturkoordinaten: je 2 × `f32`, gleiche Anzahl wie Punkte; (0, 0) = links oben |
| `FLAE` | ja | Anzahl Flächen `u32`, dann je Fläche ihre Eckenzahl `u32`, dann alle Punkt-Indizes `u32` hintereinander |

Flächen sind **Vielecke** (≥ 3 Ecken), damit der Mesh-Editor (Timbre Carve) Vierecke und
größere Flächen behält. Zum Zeichnen werden sie als Fächer in Dreiecke zerlegt.
Koordinatensystem wie glTF: rechtshändig, Y nach oben.

## Szene (Typ 3)

| Block | Pflicht | Inhalt |
|---|---|---|
| `TEXT` | ja | Szenenbeschreibung als UTF-8-Text |

Der Aufbau des Textes wird mit dem ECS (Meilenstein 4) festgelegt. Verweise auf andere
Assets erfolgen über deren **Asset-ID** (32 Hex-Zeichen), nie über Dateinamen.

## Asset-IDs

- 128 Bit, zufällig erzeugt beim ersten Import.
- Wird mit `tbr` erneut in dieselbe Zieldatei importiert, bleibt die ID erhalten.
- Dateien dürfen umbenannt und verschoben werden – Verweise bleiben gültig.

## Versionierung

Version 0 ist der Entwurf aus Meilenstein 3 und darf sich noch ändern.
Ab Version 1 gilt: Jede neuere Timbre-Version kann alle älteren Dateien lesen.
Dateien mit höherer Version als bekannt werden mit einer klaren Fehlermeldung abgelehnt.
