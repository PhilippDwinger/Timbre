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

## Testszene (Meilenstein 2)
| Taste | Wirkung |
|---|---|
| WASD / Pfeiltasten | Kamera bewegen |
| Mausrad / Q, E | Zoomen (zur Maus hin) |
| Linke Maustaste halten | Blätter an der Maus verstreuen |
| Leertaste | 5.000 Objekte dazu |
| R | Zurücksetzen |
| C | Alles löschen |
| Esc | Beenden (nur nativ) |

Die Titelzeile zeigt FPS, Logik-Schritte pro Sekunde, Anzahl Sprites und Draw Calls.

Details zu Architektur und Fahrplan: siehe `CLAUDE.md`.
