#!/usr/bin/env bash
# Baut assets/scripting/luau.wasm aus den Luau-Quellen.
#
# Nur nötig, wenn sich gast.cpp oder die Luau-Version ändert – das fertige Modul liegt im Repo.
# Voraussetzungen (Linux/WSL): python3 mit `pip install ziglang`, cargo (lädt die Luau-Quellen).
#
# Warum so: Luau ist C++. zig bringt einen C++-Compiler für WebAssembly samt Standardbibliothek mit.
# Luau meldet Fehler per C++-Ausnahme bzw. longjmp → wir nutzen WebAssembly-Exceptions
# (neue "exnref"-Variante, die wasmtime und aktuelle Browser verstehen).
set -euo pipefail
cd "$(dirname "$0")"
ZIEL=../../assets/scripting/luau.wasm
LUAU_VERSION="0.22.0"   # Crate luau0-src (enthält Luau-Quellen)

# Luau-Quellen über cargo holen
TMP=$(mktemp -d)
mkdir -p "$TMP/src"
printf '[package]\nname="hole"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nluau0-src="=%s"\n' "$LUAU_VERSION" > "$TMP/Cargo.toml"
echo 'fn main(){}' > "$TMP/src/main.rs"
(cd "$TMP" && cargo fetch -q)
L=$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/luau0-src-${LUAU_VERSION}*/luau | head -1)
Z=$(python3 -c "import ziglang,os;print(os.path.dirname(ziglang.__file__))")/lib
CXX="python3 -m ziglang c++"
CC="python3 -m ziglang cc"

FL="-target wasm32-wasi -mcpu=generic+exception_handling+reference_types+bulk_memory -O2 \
    -fwasm-exceptions -mllvm -wasm-use-legacy-eh=false -DLUA_USE_LONGJMP=1 -mllvm -wasm-enable-sjlj"
INC="-I$L/VM/include -I$L/VM/src -I$L/Compiler/include -I$L/Ast/include -I$L/Common/include -I$L/Bytecode/include"

OBJ="$TMP/obj"; mkdir -p "$OBJ"
echo "Übersetze Luau …"
ls "$L"/{VM,Compiler,Ast,Bytecode,Common}/src/*.cpp | xargs -P "$(nproc)" -I{} sh -c "$CXX $FL $INC -c {} -o $OBJ/\$(basename {} .cpp).o"

echo "Übersetze Ausnahme-Unterstützung …"
for f in cxa_exception cxa_personality; do
  $CXX $FL -c -std=c++20 -D_LIBCPP_BUILDING_LIBRARY -D_LIBCXXABI_BUILDING_LIBRARY -DLIBCXXABI_USE_LLVM_UNWINDER \
    -D_LIBCPP_ENABLE_CXX17_REMOVED_UNEXPECTED_FUNCTIONS -I"$Z/libcxxabi/include" -I"$Z/libunwind/include" -I"$Z/libcxx/src" \
    "$Z/libcxxabi/src/$f.cpp" -o "$OBJ/eh_$f.o"
done
$CC -target wasm32-wasi -mcpu=generic+exception_handling+reference_types -O2 -fwasm-exceptions -mllvm -wasm-use-legacy-eh=false \
  -c -D_LIBUNWIND_IS_NATIVE_ONLY -D_LIBUNWIND_HIDE_SYMBOLS -I"$Z/libunwind/include" "$Z/libunwind/src/Unwind-wasm.c" -o "$OBJ/eh_unwind.o"
$CC -target wasm32-wasi -mcpu=generic+exception_handling -c tags.s -o "$OBJ/eh_tags.o"

echo "Linke …"
mkdir -p "$(dirname "$ZIEL")"
$CXX $FL -mexec-model=reactor $INC gast.cpp "$OBJ"/*.o -Wl,--strip-all -o "$ZIEL"
rm -rf "$TMP"
echo "Fertig: $ZIEL ($(stat -c %s "$ZIEL") Bytes)"
