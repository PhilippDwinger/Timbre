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
IMPORT("set_parent") void h_set_parent(int64_t id, int64_t eltern);
IMPORT("get_parent") int64_t h_get_parent(int64_t id);
IMPORT("get_name") int32_t h_get_name(int64_t id, char* aus, int32_t platz);
IMPORT("set_collision") void h_set_collision(int64_t id, int32_t form, int32_t fest, int32_t verankert);
IMPORT("get_touching") int32_t h_get_touching(int64_t id, int64_t* aus, int32_t platz);
IMPORT("mouse_down") int32_t h_mouse_down(int32_t taste);
IMPORT("mouse_pressed") int32_t h_mouse_pressed(int32_t taste);
IMPORT("ui_text") void h_ui_text(const char* k, int32_t kn, const char* t, int32_t tn, float x, float y, float groesse,
                                 float r, float g, float b, float a);
IMPORT("ui_clear") void h_ui_clear(const char* k, int32_t kn);
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

static int l_set_parent(lua_State* L) {
    h_set_parent(id_arg(L, 1), lua_isnoneornil(L, 2) ? 0 : id_arg(L, 2)); return 0;
}
static int l_get_parent(lua_State* L) {
    int64_t e = h_get_parent(id_arg(L, 1));
    if (e == 0) lua_pushnil(L); else lua_pushnumber(L, (double)e);
    return 1;
}

static int l_get_name(lua_State* L) {
    char puffer[256];
    int64_t id = id_arg(L, 1);
    int32_t n = h_get_name(id, puffer, sizeof(puffer));
    if (n < 0) { lua_pushnil(L); return 1; }
    if (n <= (int32_t)sizeof(puffer)) { lua_pushlstring(L, puffer, n); return 1; }
    std::string gross(n, '\0');
    h_get_name(id, gross.data(), n);
    lua_pushlstring(L, gross.data(), n);
    return 1;
}
// timbre.set_collision(id, "circle"|"box"|nil [, fest=true [, verankert=true]])
static int l_set_collision(lua_State* L) {
    int32_t form = 0;
    if (!lua_isnoneornil(L, 2)) {
        const char* f = luaL_checkstring(L, 2);
        if (!strcmp(f, "circle") || !strcmp(f, "kreis")) form = 1;
        else if (!strcmp(f, "box") || !strcmp(f, "rechteck")) form = 2;
        else luaL_error(L, "Form '%s' unbekannt – \"circle\" oder \"box\"", f);
    }
    int fest = lua_isnoneornil(L, 3) ? 1 : lua_toboolean(L, 3);
    int verankert = lua_isnoneornil(L, 4) ? 1 : lua_toboolean(L, 4);
    h_set_collision(id_arg(L, 1), form, fest, verankert);
    return 0;
}
static int l_get_touching(lua_State* L) {
    int64_t ids[256];
    int32_t n = h_get_touching(id_arg(L, 1), ids, 256);
    if (n > 256) n = 256;
    lua_createtable(L, n, 0);
    for (int i = 0; i < n; i++) { lua_pushnumber(L, (double)ids[i]); lua_rawseti(L, -2, i + 1); }
    return 1;
}
static int maustaste(lua_State* L) {
    const char* t = luaL_optstring(L, 1, "left");
    if (!strcmp(t, "left") || !strcmp(t, "links")) return 0;
    if (!strcmp(t, "right") || !strcmp(t, "rechts")) return 1;
    if (!strcmp(t, "middle") || !strcmp(t, "mitte")) return 2;
    luaL_error(L, "Maustaste '%s' unbekannt – \"left\", \"right\" oder \"middle\"", t);
    return 0;
}
static int l_mouse_down(lua_State* L) { lua_pushboolean(L, h_mouse_down(maustaste(L))); return 1; }
static int l_mouse_pressed(lua_State* L) { lua_pushboolean(L, h_mouse_pressed(maustaste(L))); return 1; }
// timbre.ui_text(schluessel, text, x, y [, groesse, r, g, b, a])
static int l_ui_text(lua_State* L) {
    // Zahlen zuerst lesen: luaL_tolstring legt den Text oben auf den Stack und würde
    // sonst fehlende optionale Argumente "auffüllen".
    float x = (float)luaL_checknumber(L, 3), y = (float)luaL_checknumber(L, 4);
    float groesse = (float)luaL_optnumber(L, 5, 22);
    float r = (float)luaL_optnumber(L, 6, 1), g = (float)luaL_optnumber(L, 7, 1);
    float b = (float)luaL_optnumber(L, 8, 1), a = (float)luaL_optnumber(L, 9, 1);
    int32_t kn; const char* k = str_arg(L, 1, &kn);
    size_t tn; const char* t = luaL_tolstring(L, 2, &tn);
    h_ui_text(k, kn, t, (int32_t)tn, x, y, groesse, r, g, b, a);
    return 0;
}
static int l_ui_clear(lua_State* L) {
    int32_t kn = 0; const char* k = lua_isnoneornil(L, 1) ? "" : str_arg(L, 1, &kn);
    h_ui_clear(k, kn);
    return 0;
}
static int l_broadcast(lua_State* L); // weiter unten

static const luaL_Reg timbre_funktionen[] = {
    {"log", l_log}, {"time", l_time}, {"spawn", l_spawn}, {"find", l_find}, {"destroy", l_destroy},
    {"set_position", l_set_position}, {"get_position", l_get_position},
    {"set_rotation", l_set_rotation}, {"get_rotation", l_get_rotation}, {"set_scale", l_set_scale},
    {"set_velocity", l_set_velocity}, {"set_sprite", l_set_sprite}, {"set_color", l_set_color},
    {"key_down", l_key_down}, {"key_pressed", l_key_pressed}, {"mouse_position", l_mouse_position},
    {"set_camera", l_set_camera}, {"set_parent", l_set_parent}, {"get_parent", l_get_parent},
    {"get_name", l_get_name}, {"set_collision", l_set_collision}, {"get_touching", l_get_touching},
    {"mouse_down", l_mouse_down}, {"mouse_pressed", l_mouse_pressed}, {"ui_text", l_ui_text}, {"ui_clear", l_ui_clear},
    {"broadcast", l_broadcast}, {NULL, NULL},
};

// ---------------------------------------------------------------------------
// Scripts verwalten
// ---------------------------------------------------------------------------
struct Script {
    std::string name;
    int64_t id;   // Objekt-ID des Script-Objekts (0 = keins)
    int umgebung; // Referenz auf die eigene Umgebung (Tabelle) des Scripts
    bool aktiv;   // nach einem Fehler in on_update wird das Script angehalten
};

static lua_State* L = nullptr;
static std::vector<Script> scripts;
static int geteilt_ref = 0; // Tabelle `shared` – für alle Scripts dieselbe

// ---------------------------------------------------------------------------
// Aufgaben: task.wait / task.delay / task.spawn (wie in Roblox)
//
// Wartende Coroutinen merken wir uns mit Weckzeit; jeder Logik-Schritt weckt die fälligen.
// ---------------------------------------------------------------------------
struct Warten {
    int ref;         // hält die Coroutine am Leben
    lua_State* co;
    double wann;     // Weckzeit (timbre.time())
    int nargs;       // 1 = Rückkehr aus task.wait (vergangene Zeit), 0 = Start (task.delay)
    double seit;     // für den Rückgabewert von task.wait
};
static std::vector<Warten> wartende;

// Coroutine fortsetzen; meldet Fehler. Liefert false bei Fehler.
static bool fortsetzen(lua_State* co, int nargs) {
    int st = lua_resume(co, nullptr, nargs);
    if (st == LUA_OK || st == LUA_YIELD) return true;
    const char* m = lua_tostring(co, -1);
    fehler_str(std::string(m ? m : "unbekannter Fehler"));
    return false;
}

static int l_task_wait(lua_State* co) {
    double s = luaL_optnumber(co, 1, 0);
    if (!lua_isyieldable(co))
        luaL_error(co, "task.wait geht nur in on_start, on_touch, on_message, task.spawn und task.delay – nicht in on_update");
    lua_pushthread(co);
    int ref = lua_ref(co, -1);
    lua_pop(co, 1);
    double jetzt = h_zeit();
    wartende.push_back(Warten{ref, co, jetzt + s, 1, jetzt});
    return lua_yield(co, 0);
}

// Funktion (+ Argumente) an Stelle `ab` auf dem Stack von L als neue Coroutine starten.
static bool als_aufgabe_starten(lua_State* von, int ab, int nargs) {
    lua_State* co = lua_newthread(von);
    lua_pushvalue(von, ab);
    for (int i = 1; i <= nargs; i++) lua_pushvalue(von, ab + i);
    lua_xmove(von, co, nargs + 1);
    bool ok = fortsetzen(co, nargs);
    lua_pop(von, 1); // Coroutine vom Stack; wartet sie, hält `wartende` sie fest
    return ok;
}

static int l_task_spawn(lua_State* L) {
    luaL_checktype(L, 1, LUA_TFUNCTION);
    als_aufgabe_starten(L, 1, lua_gettop(L) - 1);
    return 0;
}

static int l_task_delay(lua_State* L) {
    double s = luaL_checknumber(L, 1);
    luaL_checktype(L, 2, LUA_TFUNCTION);
    lua_State* co = lua_newthread(L);
    int ref = lua_ref(L, -1);
    lua_pop(L, 1);
    lua_pushvalue(L, 2);
    lua_xmove(L, co, 1);
    wartende.push_back(Warten{ref, co, h_zeit() + s, 0, 0});
    return 0;
}

static const luaL_Reg task_funktionen[] = {
    {"wait", l_task_wait}, {"spawn", l_task_spawn}, {"delay", l_task_delay}, {NULL, NULL},
};

// Fällige Aufgaben wecken (einmal pro Logik-Schritt).
static void aufgaben_wecken() {
    double jetzt = h_zeit();
    std::vector<Warten> faellig;
    for (size_t i = 0; i < wartende.size();) {
        if (wartende[i].wann <= jetzt) {
            faellig.push_back(wartende[i]);
            wartende.erase(wartende.begin() + i);
        } else {
            i++;
        }
    }
    for (auto& w : faellig) {
        if (w.nargs == 1) lua_pushnumber(w.co, jetzt - w.seit);
        fortsetzen(w.co, w.nargs);
        lua_unref(L, w.ref);
    }
}

static void vm_starten() {
    if (L) return;
    L = luaL_newstate();
    luaL_openlibs(L);
    luaL_register(L, "timbre", timbre_funktionen);
    lua_pop(L, 1);
    luaL_register(L, "task", task_funktionen);
    lua_pop(L, 1);
    lua_pushcfunction(L, l_log, "print");
    lua_setglobal(L, "print");
    // Sandbox: globale Bibliotheken schreibgeschützt; jedes Script bekommt eine eigene Umgebung.
    luaL_sandbox(L);
    // `shared`: eine beschreibbare Tabelle, die alle Scripts gemeinsam sehen
    lua_newtable(L);
    geteilt_ref = lua_ref(L, -1);
    lua_pop(L, 1);
}

// Ruft funktion(...) eines Scripts als eigene Coroutine auf (darf task.wait benutzen).
// Die nargs Argumente liegen oben auf dem Stack von S und werden verbraucht. false bei Fehler.
// S = der Lua-Stack, auf dem gerade gearbeitet wird (Haupt-State oder eine Coroutine, z. B. bei broadcast).
static bool ereignis(lua_State* S, Script& s, const char* funktion, int nargs) {
    lua_getref(S, s.umgebung);
    lua_getfield(S, -1, funktion);
    lua_remove(S, -2);
    if (!lua_isfunction(S, -1)) { lua_pop(S, 1 + nargs); return true; }
    lua_insert(S, -(nargs + 1)); // Funktion vor die Argumente
    int ab = lua_gettop(S) - nargs;
    bool ok = als_aufgabe_starten(S, ab, nargs);
    lua_settop(S, ab - 1);
    return ok;
}

// timbre.broadcast(nachricht [, daten]) → on_message(nachricht, daten) in allen Scripts
static int l_broadcast(lua_State* S) {
    luaL_checkstring(S, 1);
    int daten = lua_gettop(S) >= 2 ? 2 : 0;
    for (auto& s : scripts) {
        if (!s.aktiv) continue;
        lua_pushvalue(S, 1);
        if (daten) lua_pushvalue(S, daten); else lua_pushnil(S);
        ereignis(S, s, "on_message", 2);
    }
    return 0;
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
// script_id = Objekt des Scripts, eltern_id = Objekt, unter dem es hängt (0 = keins).
// 0 = ok, 1 = Syntaxfehler, 2 = Laufzeitfehler
EXPORT("timbre_laden") int32_t timbre_laden(const char* name, int32_t name_n, const char* quelle, int32_t quelle_n,
                                            int64_t script_id, int64_t eltern_id) {
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

    // `script` – wie in Roblox: script.name, script.id, script.parent
    lua_newtable(L);
    lua_pushlstring(L, name, name_n);
    lua_setfield(L, -2, "name");
    if (script_id != 0) { lua_pushnumber(L, (double)script_id); lua_setfield(L, -2, "id"); }
    if (eltern_id != 0) { lua_pushnumber(L, (double)eltern_id); lua_setfield(L, -2, "parent"); }
    lua_setfield(L, umgebung_index, "script");
    lua_getref(L, geteilt_ref);
    lua_setfield(L, umgebung_index, "shared");

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
    scripts.push_back(Script{std::string(name, name_n), script_id, lua_ref(L, -1), true});
    lua_pop(L, 1);
    return 0;
}

// Ruft on_start() aller Scripts auf – jeweils als Coroutine, damit task.wait darin geht.
EXPORT("timbre_start") void timbre_start() {
    for (auto& s : scripts)
        if (s.aktiv && !ereignis(L, s, "on_start", 0)) s.aktiv = false;
}

// Ruft on_update(dt) aller Scripts auf (einmal pro Logik-Schritt).
EXPORT("timbre_update") void timbre_update(float dt) {
    for (auto& s : scripts) {
        if (s.aktiv && !aufrufen(s, "on_update", true, dt)) {
            s.aktiv = false;
            fehler_str(s.name + " wurde angehalten.");
        }
    }
    aufgaben_wecken();
}

// Das Eltern-Objekt dieses Scripts hat `anderes` berührt → on_touch(anderes)
EXPORT("timbre_beruehrung") void timbre_beruehrung(int64_t script_id, int64_t anderes) {
    for (auto& s : scripts) {
        if (s.id == script_id && s.aktiv) {
            lua_pushnumber(L, (double)anderes);
            ereignis(L, s, "on_touch", 1);
        }
    }
}

// Script anhalten (z. B. weil sein Objekt gelöscht wurde).
EXPORT("timbre_script_stoppen") void timbre_script_stoppen(int64_t script_id) {
    for (auto& s : scripts)
        if (s.id == script_id) s.aktiv = false;
}

} // extern "C"
