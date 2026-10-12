// Timbre Luau-Gast: der Luau-Interpreter als WebAssembly-Modul.
//
// Die Engine (Host) lädt dieses Modul, übergibt Script-Quelltexte und ruft jeden
// Logik-Schritt `timbre_update` auf. Scripts sprechen mit der Engine nur über die
// Funktionen in `timbre.*` – die landen bei den Importen unten (Modul "timbre").
// Schnittstelle: docs/scripting-abi.md
//
// Gebaut wird das Modul mit tools/luau-gast/bauen.sh (zig als C++-Compiler).

#include "lua.h"
#include "lualib.h"
#include "luacode.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>

#define IMPORT(name) __attribute__((import_module("timbre"), import_name(name)))
#define EXPORT(name) __attribute__((export_name(name)))

// ---------------------------------------------------------------------------
// Funktionen der Engine (Importe)
// ---------------------------------------------------------------------------
extern "C" {
IMPORT("log") void h_log(const char* p, int32_t n);
IMPORT("fehler") void h_fehler(const char* p, int32_t n);
IMPORT("zeit") double h_zeit();
IMPORT("spawn") int64_t h_spawn(const char* p, int32_t n);
IMPORT("find") int64_t h_find(const char* p, int32_t n);
IMPORT("destroy") void h_destroy(int64_t id);
IMPORT("set_position") void h_set_position(int64_t id, float x, float y);
IMPORT("get_position") int32_t h_get_position(int64_t id, float* aus);
IMPORT("set_rotation") void h_set_rotation(int64_t id, float r);
IMPORT("get_rotation") float h_get_rotation(int64_t id);
IMPORT("set_scale") void h_set_scale(int64_t id, float x, float y);
IMPORT("set_velocity") void h_set_velocity(int64_t id, float vx, float vy, float dreh);
IMPORT("set_sprite") int32_t h_set_sprite(int64_t id, const char* p, int32_t n, float b, float h, int32_t ebene);
IMPORT("set_color") void h_set_color(int64_t id, float r, float g, float b, float a);
IMPORT("key_down") int32_t h_key_down(const char* p, int32_t n);
IMPORT("key_pressed") int32_t h_key_pressed(const char* p, int32_t n);
IMPORT("mouse_position") void h_mouse_position(float* aus);
IMPORT("set_camera") void h_set_camera(float x, float y, float zoom);
}

static void log_str(const std::string& s) { h_log(s.data(), (int32_t)s.size()); }
static void fehler_str(const std::string& s) { h_fehler(s.data(), (int32_t)s.size()); }

// ---------------------------------------------------------------------------
// Lua-Seite von timbre.*
// ---------------------------------------------------------------------------
static int64_t id_arg(lua_State* L, int i) { return (int64_t)luaL_checknumber(L, i); }

static const char* str_arg(lua_State* L, int i, int32_t* n) {
    size_t len;
    const char* s = luaL_checklstring(L, i, &len);
    *n = (int32_t)len;
    return s;
}

// print / timbre.log: alle Argumente mit Leerzeichen verbinden
static int l_log(lua_State* L) {
    std::string out;
    int n = lua_gettop(L);
    for (int i = 1; i <= n; i++) {
        size_t len;
        const char* s = luaL_tolstring(L, i, &len);
        if (i > 1) out += ' ';
        out.append(s, len);
        lua_pop(L, 1);
    }
    log_str(out);
    return 0;
}

static int l_time(lua_State* L) { lua_pushnumber(L, h_zeit()); return 1; }
static int l_spawn(lua_State* L) {
    int32_t n = 0; const char* s = lua_isnoneornil(L, 1) ? "" : str_arg(L, 1, &n);
    lua_pushnumber(L, (double)h_spawn(s, n)); return 1;
}
static int l_find(lua_State* L) {
    int32_t n; const char* s = str_arg(L, 1, &n);
    int64_t id = h_find(s, n);
    if (id == 0) lua_pushnil(L); else lua_pushnumber(L, (double)id);
    return 1;
}
static int l_destroy(lua_State* L) { h_destroy(id_arg(L, 1)); return 0; }
static int l_set_position(lua_State* L) {
    h_set_position(id_arg(L, 1), (float)luaL_checknumber(L, 2), (float)luaL_checknumber(L, 3)); return 0;
}
static int l_get_position(lua_State* L) {
    float p[2];
    if (!h_get_position(id_arg(L, 1), p)) return 0;
    lua_pushnumber(L, p[0]); lua_pushnumber(L, p[1]); return 2;
}
static int l_set_rotation(lua_State* L) { h_set_rotation(id_arg(L, 1), (float)luaL_checknumber(L, 2)); return 0; }
static int l_get_rotation(lua_State* L) { lua_pushnumber(L, h_get_rotation(id_arg(L, 1))); return 1; }
static int l_set_scale(lua_State* L) {
    float x = (float)luaL_checknumber(L, 2);
    h_set_scale(id_arg(L, 1), x, (float)luaL_optnumber(L, 3, x)); return 0;
}
static int l_set_velocity(lua_State* L) {
    h_set_velocity(id_arg(L, 1), (float)luaL_checknumber(L, 2), (float)luaL_checknumber(L, 3),
                   (float)luaL_optnumber(L, 4, 0)); return 0;
}
static int l_set_sprite(lua_State* L) {
    int32_t n; const char* s = str_arg(L, 2, &n);
    float b = (float)luaL_checknumber(L, 3);
    float h = (float)luaL_optnumber(L, 4, b);
    int32_t ebene = (int32_t)luaL_optinteger(L, 5, 0);
    if (!h_set_sprite(id_arg(L, 1), s, n, b, h, ebene)) luaL_error(L, "unbekannte Textur '%s'", s);
    return 0;
}
static int l_set_color(lua_State* L) {
    h_set_color(id_arg(L, 1), (float)luaL_checknumber(L, 2), (float)luaL_checknumber(L, 3),
                (float)luaL_checknumber(L, 4), (float)luaL_optnumber(L, 5, 1)); return 0;
}
static int l_key_down(lua_State* L) { int32_t n; const char* s = str_arg(L, 1, &n); lua_pushboolean(L, h_key_down(s, n)); return 1; }
static int l_key_pressed(lua_State* L) { int32_t n; const char* s = str_arg(L, 1, &n); lua_pushboolean(L, h_key_pressed(s, n)); return 1; }
static int l_mouse_position(lua_State* L) {
    float p[2]; h_mouse_position(p);
    lua_pushnumber(L, p[0]); lua_pushnumber(L, p[1]); return 2;
}
static int l_set_camera(lua_State* L) {
    h_set_camera((float)luaL_checknumber(L, 1), (float)luaL_checknumber(L, 2), (float)luaL_optnumber(L, 3, 1)); return 0;
}

static const luaL_Reg timbre_funktionen[] = {
    {"log", l_log}, {"time", l_time}, {"spawn", l_spawn}, {"find", l_find}, {"destroy", l_destroy},
    {"set_position", l_set_position}, {"get_position", l_get_position},
    {"set_rotation", l_set_rotation}, {"get_rotation", l_get_rotation}, {"set_scale", l_set_scale},
    {"set_velocity", l_set_velocity}, {"set_sprite", l_set_sprite}, {"set_color", l_set_color},
    {"key_down", l_key_down}, {"key_pressed", l_key_pressed}, {"mouse_position", l_mouse_position},
    {"set_camera", l_set_camera}, {NULL, NULL},
};

// ---------------------------------------------------------------------------
// Scripts verwalten
// ---------------------------------------------------------------------------
struct Script {
    std::string name;
    int umgebung; // Referenz auf die eigene Umgebung (Tabelle) des Scripts
    bool aktiv;   // nach einem Fehler in on_update wird das Script angehalten
};

static lua_State* L = nullptr;
static std::vector<Script> scripts;

static void vm_starten() {
    if (L) return;
    L = luaL_newstate();
    luaL_openlibs(L);
    luaL_register(L, "timbre", timbre_funktionen);
    lua_pop(L, 1);
    lua_pushcfunction(L, l_log, "print");
    lua_setglobal(L, "print");
    // Sandbox: globale Bibliotheken schreibgeschützt; jedes Script bekommt eine eigene Umgebung.
    luaL_sandbox(L);
}

// Ruft name() bzw. name(dt) in der Umgebung eines Scripts auf. false bei Fehler.
static bool aufrufen(Script& s, const char* funktion, bool mit_dt, float dt) {
    lua_getref(L, s.umgebung);
    lua_getfield(L, -1, funktion);
    if (!lua_isfunction(L, -1)) { lua_pop(L, 2); return true; }
    if (mit_dt) lua_pushnumber(L, dt);
    int r = lua_pcall(L, mit_dt ? 1 : 0, 0, 0);
    if (r != 0) {
        fehler_str(std::string(lua_tostring(L, -1) ? lua_tostring(L, -1) : "unbekannter Fehler") + "  (in " + funktion + ")");
        lua_pop(L, 2);
        return false;
    }
    lua_pop(L, 1);
    return true;
}

extern "C" {

EXPORT("timbre_alloc") void* timbre_alloc(int32_t n) { return malloc(n > 0 ? n : 1); }
EXPORT("timbre_free") void timbre_free(void* p) { free(p); }

// Script übersetzen und ausführen (der Code außerhalb von Funktionen läuft sofort).
// 0 = ok, 1 = Syntaxfehler, 2 = Laufzeitfehler
EXPORT("timbre_laden") int32_t timbre_laden(const char* name, int32_t name_n, const char* quelle, int32_t quelle_n) {
    vm_starten();
    std::string chunk = "=" + std::string(name, name_n);

    size_t bc_n = 0;
    char* bc = luau_compile(quelle, quelle_n, nullptr, &bc_n);

    // Eigene Umgebung: liest unbekannte Namen aus den globalen Bibliotheken.
    lua_newtable(L);
    lua_newtable(L);
    lua_pushvalue(L, LUA_GLOBALSINDEX);
    lua_setfield(L, -2, "__index");
    lua_setmetatable(L, -2);
    int umgebung_index = lua_gettop(L);

    int r = luau_load(L, chunk.c_str(), bc, bc_n, umgebung_index);
    free(bc);
    if (r != 0) {
        fehler_str(lua_tostring(L, -1));
        lua_pop(L, 2);
        return 1;
    }
    if (lua_pcall(L, 0, 0, 0) != 0) {
        fehler_str(lua_tostring(L, -1));
        lua_pop(L, 2);
        return 2;
    }
    scripts.push_back(Script{std::string(name, name_n), lua_ref(L, -1), true});
    lua_pop(L, 1);
    return 0;
}

// Ruft on_start() aller Scripts auf.
EXPORT("timbre_start") void timbre_start() {
    for (auto& s : scripts)
        if (s.aktiv && !aufrufen(s, "on_start", false, 0)) s.aktiv = false;
}

// Ruft on_update(dt) aller Scripts auf (einmal pro Logik-Schritt).
EXPORT("timbre_update") void timbre_update(float dt) {
    for (auto& s : scripts) {
        if (s.aktiv && !aufrufen(s, "on_update", true, dt)) {
            s.aktiv = false;
            fehler_str(s.name + " wurde angehalten.");
        }
    }
}

} // extern "C"
