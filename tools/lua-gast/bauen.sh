#!/usr/bin/env bash
# Baut assets/scripting/lua.wasm (Lua 5.4) – Gegenstück zu tools/luau-gast/bauen.sh.
#
# Nur nötig, wenn sich gast.cpp oder die Lua-Version ändert – das fertige Modul liegt im Repo.
# Voraussetzungen (Linux/WSL): python3 mit `pip install ziglang`, cargo (lädt die Lua-Quellen).
#
# Lua meldet Fehler per setjmp/longjmp → WebAssembly-Exceptions (exnref) wie beim Luau-Gast.
set -euo pipefail
cd "$(dirname "$0")"
ZIEL=../../assets/scripting/lua.wasm
LUA_SRC_VERSION="548.1.1"   # Crate lua-src (enthält Lua 5.4.8)

TMP=$(mktemp -d)
mkdir -p "$TMP/src"
printf '[package]\nname="hole"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nlua-src="=%s"\n' "$LUA_SRC_VERSION" > "$TMP/Cargo.toml"
echo 'fn main(){}' > "$TMP/src/main.rs"
(cd "$TMP" && cargo fetch -q)
L=$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/lua-src-${LUA_SRC_VERSION}/lua-5.4* | head -1)
CXX="python3 -m ziglang c++"
CC="python3 -m ziglang cc"

FL="-target wasm32-wasi -mcpu=generic+exception_handling+reference_types+bulk_memory -O2 \
    -mllvm -wasm-use-legacy-eh=false -mllvm -wasm-enable-sjlj"

OBJ="$TMP/obj"; mkdir -p "$OBJ"
echo "Übersetze Lua …"
# Ohne: lua.c/luac.c (Programme), linit.c (eigene Auswahl in gast.cpp),
#       liolib/loslib/loadlib/ldblib (Dateien, Betriebssystem, Module, Debug – für Scripts gesperrt)
for f in "$L"/*.c; do
  case "$(basename "$f")" in lua.c|luac.c|linit.c|liolib.c|loslib.c|loadlib.c|ldblib.c) continue;; esac
  echo "$f"
done | xargs -P "$(nproc)" -I{} sh -c "$CC $FL -std=gnu99 -D_WASI_EMULATED_SIGNAL -c {} -o $OBJ/\$(basename {} .c).o"
$CC -target wasm32-wasi -mcpu=generic+exception_handling -c tags.s -o "$OBJ/eh_tags.o"

echo "Linke …"
mkdir -p "$(dirname "$ZIEL")"
$CXX $FL -fno-exceptions -mexec-model=reactor -I"$L" gast.cpp "$OBJ"/*.o -Wl,--strip-all -o "$ZIEL"
rm -rf "$TMP"
echo "Fertig: $ZIEL ($(stat -c %s "$ZIEL") Bytes)"
