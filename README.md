# Timbre

Eigene Engine in Rust – nativ und im Browser, Scripting in Lua, Luau, Python, Java und vielen weiteren Sprachen.
Teil der Grove-Welt.

## Starten (nativ, Windows)
```
cargo run -p timbre-app
```

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
Braucht einen Browser mit WebGPU (Chrome oder Edge).

## Testszene
| Taste | Wirkung |
|---|---|
| WASD / Pfeiltasten | Kamera bewegen |
| Mausrad / Q, E | Zoomen (zur Maus hin) |
| Linke Maustaste halten | Blätter an der Maus verstreuen |
| Leertaste | 5.000 Objekte dazu |
| L | Blätter unter / über die Baumscheiben legen (Ebenen) |
| F5 / F9 | Szene speichern / laden (`assets\szenen\gespeichert.tbr`) |
| R | Zurücksetzen |
| C | Alles löschen |
| Esc | Beenden (nur nativ) |

Die Titelzeile zeigt FPS, Logik-Schritte pro Sekunde, Anzahl Sprites und Draw Calls.

## Assets umwandeln (`tbr`)
```
cargo run -p tbr -- textur assets\quellen\blatt.png assets\texturen\blatt.tbr
cargo run -p tbr -- mesh assets\quellen\kiste.glb assets\meshes\kiste.tbr
cargo run -p tbr -- info assets\texturen\blatt.tbr
cargo run -p tbr -- text assets\szenen\gespeichert.tbr
```
Wird erneut in dieselbe `.tbr`-Datei importiert, bleibt ihre Asset-ID erhalten.
Aus Blender: *Datei → Exportieren → glTF 2.0 (.glb)*. Formatbeschreibung: `docs/tbr-format.md`.

Details zu Architektur und Fahrplan: siehe `CLAUDE.md`.
