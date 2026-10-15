# Timbre

![Timbre](assets/marke/banner.png)

Eigene Engine in Rust – 2D und 3D, nativ und im Browser, Scripting in Lua, Luau, Python, Java und vielen weiteren Sprachen.
Teil der Grove-Welt.

## Timbre Studio starten (Windows)
```
cargo run -p timbre-app
```
Öffnet das Beispielprojekt `projekte\beispiel`. Anderes Projekt: `cargo run -p timbre-app -- projekte\meinprojekt`.
Der allererste Build dauert einige Minuten (Grafik, Scripting und Oberfläche werden optimiert übersetzt).

| Taste / Knopf | Wirkung |
|---|---|
| **▶ Ausführen** / **F5** | Scripts starten |
| **■ Stopp** / **F5** | Anhalten, Szene zurücksetzen |
| **💾 Speichern** / **Strg+S** | Scripts und Szene speichern |
| **↩ / ↪** / **Strg+Z**, **Strg+Y** | Rückgängig / Wiederholen (im Script-Editor: Text-Rückgängig) |
| **Strg+D** | Ausgewähltes Objekt duplizieren (samt Kindern) |
| Explorer: ziehen | Objekt unter ein anderes hängen (auf freie Fläche = oberste Ebene) |
| Explorer: Doppelklick | Script öffnen / Kamera zum Objekt |
| Explorer: Rechtsklick | Script hinzufügen, Duplizieren, Löschen |
| Konsole: Fehler anklicken | Springt im Script zur Fehlerzeile |
| Script-Editor: tippen | **Autovervollständigung** – z. B. nach `timbre.` alle Engine-Funktionen. Pfeiltasten wählen, **Tab**/**Enter** übernehmen, **Esc** schließt, **Strg+Leertaste** öffnet die Liste von Hand |
| **Strg+S** während das Spiel läuft | **Live-Neuladen**: geänderte Scripts übernehmen sofort ihre neue Fassung (ihr `on_start` läuft neu), ohne ■ Stopp |
| Script in VS Code ändern und speichern | Studio übernimmt die Änderung automatisch (läuft das Spiel: sofort neu geladen) |
| **Gitter** (oben) | Gitter anzeigen; Verschieben, Drehen (15°) und Skalieren rasten ein – **Alt** gedrückt = frei |
| Griffe am ausgewählten Objekt | Ecken = skalieren, Kreis oben = drehen |
| Linke Maus (Szene) | Objekt auswählen und verschieben |
| Rechte / mittlere Maus | Ansicht schieben |
| Mausrad | Zoomen |
| WASD | Ansicht schieben (im Bearbeiten-Modus) |
| Entf | Ausgewähltes Objekt löschen |
| Explorer: **+ Objekt** | Neues **leeres** Objekt (nur Name, Position, Eltern) |
| Eigenschaften: **+ Property hinzufügen** | Bild, HitBox, Masse, Bewegung, Scripts, Transform 3D, Mesh (3D) – jedes als eigener Block, 🗙 entfernt es |
| Script-Eigenschaften: **⚑ Ereignisse** | Eigene Funktionsnamen statt `on_start`, `on_update` … (mit Vorschlägen) |
| Script-Eigenschaften: **⚡ Live-Script** | `live function …` / `live def …`: Argumente werden Felder, **▶ Run** ruft die Funktion ohne Spielstart auf |
| Leiste: **2D / 3D** | Ansicht umschalten |
| Explorer: **+ Körper** | Würfel, Kugel, Ebene oder Zylinder (3D) |
| 3D: rechte Maus / Mitte / Rad | Kamera drehen / schieben / zoomen (Umschalt+rechts = schieben) |
| 3D: Körper ziehen | Waagerecht verschieben, mit **Umschalt** hoch/runter; Gitter an = auf 0,5 einrasten |
| **PNG ins Fenster ziehen** | Wird zum Bild (`texturen/<name>.tbr` im Projekt) und landet als Objekt mitten in der Szene |
| **WAV / OGG / MP3 / FLAC ins Fenster ziehen** | Wird zum Sound (`sounds/` im Projekt), im Script: `timbre.play_sound("Name")` |
| Explorer: **🖼 Bilder** / **🔊 Sounds** | Alle Bilder (klicken = in die Szene setzen) und Sounds (▶ = anhören) des Projekts |

Scripts schreiben (Luau, Lua 5.4 oder Python 3): siehe `docs/scripting.md`.

## Starten (Browser)
Einmalig:
```
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```
Dann:
```
cd crates\timbre-app
trunk serve --open
```
Braucht einen Browser mit WebGPU (Chrome oder Edge). Die Browser-Version des Studios ist noch ungeprüft und wird in Meilenstein 8 fertig gemacht.

## Assets umwandeln (`tbr`)
```
cargo run -p tbr -- textur assets\quellen\blatt.png assets\texturen\blatt.tbr
cargo run -p tbr -- mesh assets\quellen\kiste.glb assets\meshes\kiste.tbr
cargo run -p tbr -- info assets\texturen\blatt.tbr
cargo run -p tbr -- text projekte\beispiel\szene.tbr
cargo run -p tbr -- szene meine_szene.json projekte\beispiel\szene.tbr
```
Wird erneut in dieselbe `.tbr`-Datei importiert, bleibt ihre Asset-ID erhalten.
Aus Blender: *Datei → Exportieren → glTF 2.0 (.glb)*. Formatbeschreibung: `docs/tbr-format.md`.

Details zu Architektur und Fahrplan: siehe `CLAUDE.md`.
