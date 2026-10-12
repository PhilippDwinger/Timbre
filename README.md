# Timbre

Eigene Engine in Rust – nativ und im Browser, Scripting in Lua, Luau, Python, Java und vielen weiteren Sprachen.
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
| Linke Maus (Szene) | Objekt auswählen und verschieben |
| Rechte / mittlere Maus | Ansicht schieben |
| Mausrad | Zoomen |
| WASD | Ansicht schieben (im Bearbeiten-Modus) |
| Entf | Ausgewähltes Objekt löschen |

Scripts schreiben: siehe `docs/scripting.md`.

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
