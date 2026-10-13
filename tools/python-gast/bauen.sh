#!/usr/bin/env bash
# Baut assets/scripting/python.wasm (CPython 3.12) und assets/scripting/python312.zip (Standardbibliothek).
#
# Nur nötig, wenn sich gast.c / laufzeit.py oder die Python-Version ändert – das Ergebnis liegt im Repo.
# Voraussetzungen (Linux/WSL): curl, tar, python3. Lädt beim ersten Mal (in tools/python-gast/.cache):
#   - wasi-sdk 20 (C-Compiler für WebAssembly)
#   - libpython 3.12 für WASI aus den "WebAssembly Language Runtimes" von VMware Labs
#     (fertig übersetzte CPython-Bibliothek + Standardbibliothek als python312.zip)
set -euo pipefail
cd "$(dirname "$0")"
ZIEL=../../assets/scripting/python.wasm
ZIP_ZIEL=../../assets/scripting/python312.zip
CACHE=.cache
mkdir -p "$CACHE"

WASI_SDK="$CACHE/wasi-sdk-20.0"
if [ ! -d "$WASI_SDK" ]; then
  echo "Lade wasi-sdk 20 …"
  curl -sSL "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-20/wasi-sdk-20.0-linux.tar.gz" | tar -xz -C "$CACHE"
fi
PY="$CACHE/libpython"
if [ ! -d "$PY" ]; then
  echo "Lade libpython 3.12 (WASI) …"
  mkdir -p "$PY"
  curl -sSL "https://github.com/vmware-labs/webassembly-language-runtimes/releases/download/python%2F3.12.0%2B20231211-040d5a6/libpython-3.12.0-wasi-sdk-20.0.tar.gz" | tar -xz -C "$PY"
fi

# laufzeit.py als C-Text einbauen
python3 - <<'EOF'
import json
text = open("laufzeit.py", encoding="utf-8").read()
with open(".cache/laufzeit_py.h", "w", encoding="utf-8") as f:
    f.write("// Erzeugt von bauen.sh aus laufzeit.py – nicht von Hand ändern.\n")
    f.write("static const char LAUFZEIT_PY[] = " + json.dumps(text, ensure_ascii=False) + ";\n")
EOF

echo "Übersetze und linke …"
CC="$WASI_SDK/bin/clang --sysroot=$WASI_SDK/share/wasi-sysroot"
$CC --target=wasm32-wasi -O2 -D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_GETPID \
  -I"$PY/include/python3.12" -I"$CACHE" -mexec-model=reactor \
  gast.c "$PY/lib/wasm32-wasi/libpython3.12.a" \
  -lwasi-emulated-signal -lwasi-emulated-process-clocks -lwasi-emulated-getpid \
  -Wl,-z,stack-size=1048576 -Wl,--stack-first -Wl,--initial-memory=16777216 -Wl,--strip-all \
  -o "$ZIEL"
# Standardbibliothek ausdünnen: alles weg, was Spiele-Scripts nicht brauchen (Netz, Dateien, GUI, Tests …)
python3 - "$PY/usr/local/lib/python312.zip" "$ZIP_ZIEL" <<'EOF'
import sys, zipfile
WEG = ("asyncio/", "email/", "http/", "xml/", "xmlrpc/", "urllib/", "sqlite3/", "unittest/", "test/", "idlelib/",
       "tkinter/", "turtledemo/", "lib2to3/", "ensurepip/", "venv/", "multiprocessing/", "concurrent/", "pydoc_data/",
       "wsgiref/", "curses/", "dbm/", "ctypes/", "html/", "zoneinfo/", "tomllib/", "__phello__/", "site-packages/")
CJK = ("big5", "cp932", "cp949", "cp950", "euc_", "gb", "hz", "iso2022", "johab", "shift_jis")
EINZELN = ("pydoc.pyc", "turtle.pyc", "smtplib.pyc", "imaplib.pyc", "ftplib.pyc", "poplib.pyc", "nntplib.pyc",
           "telnetlib.pyc", "mailbox.pyc", "doctest.pyc", "pdb.pyc", "tarfile.pyc", "_pydecimal.pyc", "decimal.pyc",
           "ssl.pyc", "socketserver.pyc", "subprocess.pyc", "_pydatetime.pyc")
with zipfile.ZipFile(sys.argv[1]) as ein, zipfile.ZipFile(sys.argv[2], "w", zipfile.ZIP_DEFLATED) as aus:
    for e in ein.infolist():
        n = e.filename
        if n.startswith(WEG) or n in EINZELN:
            continue
        if n.startswith("encodings/") and n[len("encodings/"):].startswith(CJK):
            continue
        aus.writestr(e, ein.read(n))
EOF
echo "Fertig: $ZIEL ($(stat -c %s "$ZIEL") Bytes), $ZIP_ZIEL ($(stat -c %s "$ZIP_ZIEL") Bytes)"
