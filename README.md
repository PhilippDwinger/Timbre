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

Details zu Architektur und Fahrplan: siehe `CLAUDE.md`.
