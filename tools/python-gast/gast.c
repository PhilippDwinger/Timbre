// Timbre Python-Gast: CPython 3.12 als WebAssembly-Modul.
//
// Gleiche Schnittstelle (ABI) wie tools/luau-gast/gast.cpp: dieselben Importe (Modul "timbre")
// und Exporte (timbre_laden, timbre_update …). Die Script-Verwaltung (eigene Globals pro Script,
// Aufgaben mit async/await, Fehlermeldungen) steckt in laufzeit.py – dieser C-Teil
//   1. stellt das Python-Modul `timbre` bereit (ruft die Engine-Importe auf),
//   2. startet den Interpreter (Standardbibliothek aus /lib/python312.zip, vom Host bereitgestellt),
//   3. leitet die Exporte an laufzeit.py weiter.
//
// Gebaut wird das Modul mit tools/python-gast/bauen.sh (wasi-sdk 20 + libpython aus den
// WebAssembly Language Runtimes von VMware Labs).

#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "laufzeit_py.h" // LAUFZEIT_PY: Inhalt von laufzeit.py als Text (erzeugt von bauen.sh)

#define IMPORT(name) __attribute__((import_module("timbre"), import_name(name)))
#define EXPORT(name) __attribute__((export_name(name)))

// ---------------------------------------------------------------------------
// Eingebaute C-Module: nur, was Spiele-Scripts brauchen. Ersetzt die Liste aus libpython
// (config.c), die auch sqlite, Tests, ostasiatische Zeichensätze … enthält – spart viel Größe.
// Fehlende Module kann man hier ergänzen (PyInit_… gibt es alle in libpython3.12.a).
// ---------------------------------------------------------------------------
#define MODUL(name) extern PyObject* PyInit_##name(void);
MODUL(posix) MODUL(errno) MODUL(_io) MODUL(_codecs) MODUL(_weakref) MODUL(_abc) MODUL(_functools)
MODUL(_collections) MODUL(_operator) MODUL(_stat) MODUL(time) MODUL(itertools) MODUL(_sre) MODUL(_thread)
MODUL(_locale) MODUL(math) MODUL(cmath) MODUL(_random) MODUL(_bisect) MODUL(_heapq) MODUL(_struct)
MODUL(binascii) MODUL(zlib) MODUL(_json) MODUL(_datetime) MODUL(array) MODUL(_typing) MODUL(atexit)
MODUL(_contextvars) MODUL(_statistics) MODUL(_sha2) MODUL(_imp) MODUL(gc) MODUL(_string) MODUL(_signal)
MODUL(_tokenize) MODUL(_ast) MODUL(_opcode) MODUL(_md5) MODUL(_sha1) MODUL(_sha3) MODUL(_blake2)
extern PyObject* PyMarshal_Init(void);
extern PyObject* _PyWarnings_Init(void);

struct _inittab _PyImport_Inittab[] = {
    {"posix", PyInit_posix}, {"errno", PyInit_errno}, {"_io", PyInit__io}, {"_codecs", PyInit__codecs},
    {"_weakref", PyInit__weakref}, {"_abc", PyInit__abc}, {"_functools", PyInit__functools},
    {"_collections", PyInit__collections}, {"_operator", PyInit__operator}, {"_stat", PyInit__stat},
    {"time", PyInit_time}, {"itertools", PyInit_itertools}, {"_sre", PyInit__sre}, {"_thread", PyInit__thread},
    {"_locale", PyInit__locale}, {"math", PyInit_math}, {"cmath", PyInit_cmath}, {"_random", PyInit__random},
    {"_bisect", PyInit__bisect}, {"_heapq", PyInit__heapq}, {"_struct", PyInit__struct},
    {"binascii", PyInit_binascii}, {"zlib", PyInit_zlib}, {"_json", PyInit__json}, {"_datetime", PyInit__datetime},
    {"array", PyInit_array}, {"_typing", PyInit__typing}, {"atexit", PyInit_atexit},
    {"_contextvars", PyInit__contextvars}, {"_statistics", PyInit__statistics}, {"_sha2", PyInit__sha2},
    {"_md5", PyInit__md5}, {"_sha1", PyInit__sha1}, {"_sha3", PyInit__sha3}, {"_blake2", PyInit__blake2}, // für hashlib/random
    {"_signal", PyInit__signal}, {"_tokenize", PyInit__tokenize}, {"_ast", PyInit__ast}, {"_opcode", PyInit__opcode},
    {"marshal", PyMarshal_Init}, {"_imp", PyInit__imp}, {"builtins", NULL}, {"sys", NULL}, {"gc", PyInit_gc},
    {"_warnings", _PyWarnings_Init}, {"_string", PyInit__string}, {0, 0},
};

// ---------------------------------------------------------------------------
// Funktionen der Engine (Importe) – wie im Luau-Gast
// ---------------------------------------------------------------------------
IMPORT("log") void h_log(const char* p, int32_t n);
IMPORT("fehler") void h_fehler(const char* p, int32_t n);
IMPORT("zeit") double h_zeit(void);
IMPORT("spawn") int64_t h_spawn(const char* p, int32_t n);
IMPORT("find") int64_t h_find(const char* p, int32_t n);
IMPORT("destroy") void h_destroy(int64_t id);
IMPORT("set_position") void h_set_position(int64_t id, float x, float y);
IMPORT("get_position") int32_t h_get_position(int64_t id, float* aus);
IMPORT("set_rotation") void h_set_rotation(int64_t id, float r);
IMPORT("get_rotation") float h_get_rotation(int64_t id);
IMPORT("set_scale") void h_set_scale(int64_t id, float x, float y);
IMPORT("set_velocity") void h_set_velocity(int64_t id, float vx, float vy, float dreh);
IMPORT("set3d") void h_set3d(int64_t id, int32_t was, float x, float y, float z);
IMPORT("get3d") int32_t h_get3d(int64_t id, int32_t was, float* aus);
IMPORT("set_mesh") void h_set_mesh(int64_t id, const char* p, int32_t n, float r, float g, float b);
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
IMPORT("play_sound") int32_t h_play_sound(const char* p, int32_t n, float lautstaerke, int32_t schleife);
IMPORT("stop_sounds") void h_stop_sounds(const char* p, int32_t n);
IMPORT("nachricht") void h_nachricht(const char* p, int32_t n, int32_t art, double zahl, const char* t, int32_t tn);

static void fehler_c(const char* s) { h_fehler(s, (int32_t)strlen(s)); }

// ---------------------------------------------------------------------------
// Python-Modul `timbre`
// ---------------------------------------------------------------------------

// Objekt-ID aus Python: None = 0
static int64_t id_von(PyObject* o) {
    if (o == NULL || o == Py_None) return 0;
    return (int64_t)PyLong_AsLongLong(o);
}

static PyObject* id_nach(int64_t id) {
    if (id == 0) Py_RETURN_NONE;
    return PyLong_FromLongLong(id);
}

static PyObject* t_log(PyObject* self, PyObject* args) {
    Py_ssize_t n = PyTuple_Size(args);
    PyObject* teile = PyList_New(0);
    for (Py_ssize_t i = 0; i < n; i++) {
        PyObject* s = PyObject_Str(PyTuple_GetItem(args, i));
        if (!s) { Py_DECREF(teile); return NULL; }
        PyList_Append(teile, s);
        Py_DECREF(s);
    }
    PyObject* leer = PyUnicode_FromString(" ");
    PyObject* text = PyUnicode_Join(leer, teile);
    Py_DECREF(leer);
    Py_DECREF(teile);
    if (!text) return NULL;
    Py_ssize_t len;
    const char* c = PyUnicode_AsUTF8AndSize(text, &len);
    h_log(c, (int32_t)len);
    Py_DECREF(text);
    Py_RETURN_NONE;
}

static PyObject* t_fehler(PyObject* self, PyObject* args) {
    const char* s; Py_ssize_t n;
    if (!PyArg_ParseTuple(args, "s#", &s, &n)) return NULL;
    h_fehler(s, (int32_t)n);
    Py_RETURN_NONE;
}

static PyObject* t_time(PyObject* self, PyObject* args) { return PyFloat_FromDouble(h_zeit()); }

static PyObject* t_spawn(PyObject* self, PyObject* args) {
    const char* s = ""; Py_ssize_t n = 0;
    if (!PyArg_ParseTuple(args, "|s#", &s, &n)) return NULL;
    return id_nach(h_spawn(s, (int32_t)n));
}

static PyObject* t_find(PyObject* self, PyObject* args) {
    const char* s; Py_ssize_t n;
    if (!PyArg_ParseTuple(args, "s#", &s, &n)) return NULL;
    return id_nach(h_find(s, (int32_t)n));
}

static PyObject* t_destroy(PyObject* self, PyObject* args) {
    PyObject* o;
    if (!PyArg_ParseTuple(args, "O", &o)) return NULL;
    h_destroy(id_von(o));
    Py_RETURN_NONE;
}

static PyObject* t_set_position(PyObject* self, PyObject* args) {
    PyObject* o; float x, y;
    if (!PyArg_ParseTuple(args, "Off", &o, &x, &y)) return NULL;
    h_set_position(id_von(o), x, y);
    Py_RETURN_NONE;
}

static PyObject* t_get_position(PyObject* self, PyObject* args) {
    PyObject* o; float p[2];
    if (!PyArg_ParseTuple(args, "O", &o)) return NULL;
    if (!h_get_position(id_von(o), p)) Py_RETURN_NONE;
    return Py_BuildValue("(dd)", (double)p[0], (double)p[1]);
}

static PyObject* t_set_rotation(PyObject* self, PyObject* args) {
    PyObject* o; float r;
    if (!PyArg_ParseTuple(args, "Of", &o, &r)) return NULL;
    h_set_rotation(id_von(o), r);
    Py_RETURN_NONE;
}

static PyObject* t_get_rotation(PyObject* self, PyObject* args) {
    PyObject* o;
    if (!PyArg_ParseTuple(args, "O", &o)) return NULL;
    return PyFloat_FromDouble(h_get_rotation(id_von(o)));
}

static PyObject* t_set_scale(PyObject* self, PyObject* args) {
    PyObject* o; float x; PyObject* y = NULL;
    if (!PyArg_ParseTuple(args, "Of|O", &o, &x, &y)) return NULL;
    h_set_scale(id_von(o), x, (y && y != Py_None) ? (float)PyFloat_AsDouble(y) : x);
    Py_RETURN_NONE;
}

static PyObject* t_set_velocity(PyObject* self, PyObject* args) {
    PyObject* o; float vx, vy, d = 0;
    if (!PyArg_ParseTuple(args, "Off|f", &o, &vx, &vy, &d)) return NULL;
    h_set_velocity(id_von(o), vx, vy, d);
    Py_RETURN_NONE;
}

static PyObject* t_set_sprite(PyObject* self, PyObject* args) {
    PyObject* o; const char* s; Py_ssize_t n; float b; PyObject* h = NULL; int ebene = 0;
    if (!PyArg_ParseTuple(args, "Os#f|Oi", &o, &s, &n, &b, &h, &ebene)) return NULL;
    float hoehe = (h && h != Py_None) ? (float)PyFloat_AsDouble(h) : b;
    if (!h_set_sprite(id_von(o), s, (int32_t)n, b, hoehe, ebene))
        return PyErr_Format(PyExc_ValueError, "unbekannte Textur '%s'", s);
    Py_RETURN_NONE;
}

static PyObject* t_set_color(PyObject* self, PyObject* args) {
    PyObject* o; float r, g, b, a = 1;
    if (!PyArg_ParseTuple(args, "Offf|f", &o, &r, &g, &b, &a)) return NULL;
    h_set_color(id_von(o), r, g, b, a);
    Py_RETURN_NONE;
}

static PyObject* t_key_down(PyObject* self, PyObject* args) {
    const char* s; Py_ssize_t n;
    if (!PyArg_ParseTuple(args, "s#", &s, &n)) return NULL;
    return PyBool_FromLong(h_key_down(s, (int32_t)n));
}

static PyObject* t_key_pressed(PyObject* self, PyObject* args) {
    const char* s; Py_ssize_t n;
    if (!PyArg_ParseTuple(args, "s#", &s, &n)) return NULL;
    return PyBool_FromLong(h_key_pressed(s, (int32_t)n));
}

static PyObject* t_mouse_position(PyObject* self, PyObject* args) {
    float p[2];
    h_mouse_position(p);
    return Py_BuildValue("(dd)", (double)p[0], (double)p[1]);
}

static PyObject* t_set_camera(PyObject* self, PyObject* args) {
    float x, y, zoom = 1;
    if (!PyArg_ParseTuple(args, "ff|f", &x, &y, &zoom)) return NULL;
    h_set_camera(x, y, zoom);
    Py_RETURN_NONE;
}

static PyObject* t_set_parent(PyObject* self, PyObject* args) {
    PyObject* o; PyObject* e = Py_None;
    if (!PyArg_ParseTuple(args, "O|O", &o, &e)) return NULL;
    h_set_parent(id_von(o), id_von(e));
    Py_RETURN_NONE;
}

static PyObject* t_get_parent(PyObject* self, PyObject* args) {
    PyObject* o;
    if (!PyArg_ParseTuple(args, "O", &o)) return NULL;
    return id_nach(h_get_parent(id_von(o)));
}

static PyObject* t_get_name(PyObject* self, PyObject* args) {
    PyObject* o;
    if (!PyArg_ParseTuple(args, "O", &o)) return NULL;
    int64_t id = id_von(o);
    char puffer[256];
    int32_t n = h_get_name(id, puffer, sizeof(puffer));
    if (n < 0) Py_RETURN_NONE;
    if (n <= (int32_t)sizeof(puffer)) return PyUnicode_DecodeUTF8(puffer, n, "replace");
    char* gross = malloc(n);
    h_get_name(id, gross, n);
    PyObject* r = PyUnicode_DecodeUTF8(gross, n, "replace");
    free(gross);
    return r;
}

// timbre.set_collision(id, "circle" | "box" | None, fest=True, verankert=True)
static PyObject* t_set_collision(PyObject* self, PyObject* args) {
    PyObject* o; PyObject* form = Py_None; int fest = 1, verankert = 1;
    if (!PyArg_ParseTuple(args, "O|Opp", &o, &form, &fest, &verankert)) return NULL;
    int32_t f = 0;
    if (form != Py_None) {
        const char* s = PyUnicode_AsUTF8(form);
        if (!s) return NULL;
        if (!strcmp(s, "circle") || !strcmp(s, "kreis")) f = 1;
        else if (!strcmp(s, "box") || !strcmp(s, "rechteck")) f = 2;
        else return PyErr_Format(PyExc_ValueError, "Form '%s' unbekannt – \"circle\" oder \"box\"", s);
    }
    h_set_collision(id_von(o), f, fest, verankert);
    Py_RETURN_NONE;
}

static PyObject* t_get_touching(PyObject* self, PyObject* args) {
    PyObject* o;
    if (!PyArg_ParseTuple(args, "O", &o)) return NULL;
    int64_t ids[256];
    int32_t n = h_get_touching(id_von(o), ids, 256);
    if (n > 256) n = 256;
    PyObject* liste = PyList_New(n);
    for (int i = 0; i < n; i++) PyList_SET_ITEM(liste, i, PyLong_FromLongLong(ids[i]));
    return liste;
}

static int maustaste(PyObject* args, int32_t* t) {
    const char* s = "left";
    if (!PyArg_ParseTuple(args, "|s", &s)) return 0;
    if (!strcmp(s, "left") || !strcmp(s, "links")) *t = 0;
    else if (!strcmp(s, "right") || !strcmp(s, "rechts")) *t = 1;
    else if (!strcmp(s, "middle") || !strcmp(s, "mitte")) *t = 2;
    else { PyErr_Format(PyExc_ValueError, "Maustaste '%s' unbekannt – \"left\", \"right\" oder \"middle\"", s); return 0; }
    return 1;
}

static PyObject* t_mouse_down(PyObject* self, PyObject* args) {
    int32_t t; if (!maustaste(args, &t)) return NULL;
    return PyBool_FromLong(h_mouse_down(t));
}

static PyObject* t_mouse_pressed(PyObject* self, PyObject* args) {
    int32_t t; if (!maustaste(args, &t)) return NULL;
    return PyBool_FromLong(h_mouse_pressed(t));
}

// timbre.ui_text(schluessel, text, x, y, groesse=22, r=1, g=1, b=1, a=1)
static PyObject* t_ui_text(PyObject* self, PyObject* args) {
    const char* k; Py_ssize_t kn; PyObject* text; float x, y, groesse = 22, r = 1, g = 1, b = 1, a = 1;
    if (!PyArg_ParseTuple(args, "s#Off|fffff", &k, &kn, &text, &x, &y, &groesse, &r, &g, &b, &a)) return NULL;
    PyObject* s = PyObject_Str(text);
    if (!s) return NULL;
    Py_ssize_t tn;
    const char* t = PyUnicode_AsUTF8AndSize(s, &tn);
    h_ui_text(k, (int32_t)kn, t, (int32_t)tn, x, y, groesse, r, g, b, a);
    Py_DECREF(s);
    Py_RETURN_NONE;
}

static PyObject* t_ui_clear(PyObject* self, PyObject* args) {
    const char* k = ""; Py_ssize_t kn = 0;
    if (!PyArg_ParseTuple(args, "|z#", &k, &kn)) return NULL;
    if (!k) { k = ""; kn = 0; }
    h_ui_clear(k, (int32_t)kn);
    Py_RETURN_NONE;
}

static PyObject* t_play_sound(PyObject* self, PyObject* args) {
    const char* s; Py_ssize_t n; float lautstaerke = 1; int schleife = 0;
    if (!PyArg_ParseTuple(args, "s#|fp", &s, &n, &lautstaerke, &schleife)) return NULL;
    if (!h_play_sound(s, (int32_t)n, lautstaerke, schleife))
        return PyErr_Format(PyExc_ValueError, "unbekannter Sound '%s'", s);
    Py_RETURN_NONE;
}

static PyObject* t_stop_sounds(PyObject* self, PyObject* args) {
    const char* s = ""; Py_ssize_t n = 0;
    if (!PyArg_ParseTuple(args, "|z#", &s, &n)) return NULL;
    if (!s) { s = ""; n = 0; }
    h_stop_sounds(s, (int32_t)n);
    Py_RETURN_NONE;
}

// Intern (laufzeit.py): broadcast an andere Sprachen weiterreichen
static PyObject* t_nachricht(PyObject* self, PyObject* args) {
    const char* m; Py_ssize_t mn; PyObject* d;
    if (!PyArg_ParseTuple(args, "s#O", &m, &mn, &d)) return NULL;
    int32_t art = 0; double zahl = 0; const char* t = ""; Py_ssize_t tn = 0;
    if (PyBool_Check(d)) { art = 1; zahl = (d == Py_True); }
    else if (PyLong_Check(d)) { art = 4; zahl = (double)PyLong_AsLongLong(d); if (PyErr_Occurred()) { PyErr_Clear(); art = 2; zahl = PyLong_AsDouble(d); } }
    else if (PyFloat_Check(d)) { art = 2; zahl = PyFloat_AsDouble(d); }
    else if (PyUnicode_Check(d)) { art = 3; t = PyUnicode_AsUTF8AndSize(d, &tn); }
    h_nachricht(m, (int32_t)mn, art, zahl, t, (int32_t)tn);
    Py_RETURN_NONE;
}

static PyObject* t_broadcast(PyObject* self, PyObject* args); // weiter unten (ruft laufzeit.broadcast)

// --- 3D -------------------------------------------------------------------
static PyObject* setze3d(PyObject* args, int32_t was) {
    PyObject* o; float x, y = NAN, z = NAN;
    if (!PyArg_ParseTuple(args, "Of|ff", &o, &x, &y, &z)) return NULL;
    if (isnan(y)) y = was == 2 ? x : 0;
    if (isnan(z)) z = was == 2 ? x : 0;
    h_set3d(id_von(o), was, x, y, z);
    Py_RETURN_NONE;
}
static PyObject* lies3d(PyObject* args, int32_t was) {
    PyObject* o; float v[3];
    if (!PyArg_ParseTuple(args, "O", &o)) return NULL;
    if (!h_get3d(id_von(o), was, v)) Py_RETURN_NONE;
    return Py_BuildValue("(ddd)", (double)v[0], (double)v[1], (double)v[2]);
}
static PyObject* t_set_position3d(PyObject* self, PyObject* args) { return setze3d(args, 0); }
static PyObject* t_set_rotation3d(PyObject* self, PyObject* args) { return setze3d(args, 1); }
static PyObject* t_set_scale3d(PyObject* self, PyObject* args) { return setze3d(args, 2); }
static PyObject* t_get_position3d(PyObject* self, PyObject* args) { return lies3d(args, 0); }
static PyObject* t_get_rotation3d(PyObject* self, PyObject* args) { return lies3d(args, 1); }
static PyObject* t_get_scale3d(PyObject* self, PyObject* args) { return lies3d(args, 2); }
static PyObject* t_set_mesh(PyObject* self, PyObject* args) {
    PyObject* o; const char* s = NULL; Py_ssize_t n = 0; float r = 0.55f, g = 0.78f, b = 0.95f;
    if (!PyArg_ParseTuple(args, "Oz#|fff", &o, &s, &n, &r, &g, &b)) return NULL;
    h_set_mesh(id_von(o), s ? s : "", (int32_t)n, r, g, b);
    Py_RETURN_NONE;
}

static PyMethodDef timbre_funktionen[] = {
    {"log", t_log, METH_VARARGS, "log(...) – Text in die Konsole"},
    {"time", t_time, METH_NOARGS, "time() → Sekunden seit ▶"},
    {"spawn", t_spawn, METH_VARARGS, "spawn(name) → id – neues Objekt"},
    {"find", t_find, METH_VARARGS, "find(name) → id oder None"},
    {"destroy", t_destroy, METH_VARARGS, "destroy(id) – löscht auch die Kinder"},
    {"set_position", t_set_position, METH_VARARGS, "set_position(id, x, y) – Weltkoordinaten"},
    {"get_position", t_get_position, METH_VARARGS, "get_position(id) → (x, y)"},
    {"set_rotation", t_set_rotation, METH_VARARGS, "set_rotation(id, winkel) – Bogenmaß"},
    {"get_rotation", t_get_rotation, METH_VARARGS, "get_rotation(id) → winkel"},
    {"set_scale", t_set_scale, METH_VARARGS, "set_scale(id, x, y=x)"},
    {"set_velocity", t_set_velocity, METH_VARARGS, "set_velocity(id, vx, vy, drehen=0)"},
    {"set_sprite", t_set_sprite, METH_VARARGS, "set_sprite(id, textur, breite, hoehe=breite, ebene=0)"},
    {"set_color", t_set_color, METH_VARARGS, "set_color(id, r, g, b, a=1)"},
    {"key_down", t_key_down, METH_VARARGS, "key_down(taste) → bool"},
    {"key_pressed", t_key_pressed, METH_VARARGS, "key_pressed(taste) → bool"},
    {"mouse_position", t_mouse_position, METH_NOARGS, "mouse_position() → (x, y)"},
    {"set_camera", t_set_camera, METH_VARARGS, "set_camera(x, y, zoom=1)"},
    {"set_parent", t_set_parent, METH_VARARGS, "set_parent(id, eltern=None)"},
    {"get_parent", t_get_parent, METH_VARARGS, "get_parent(id) → id oder None"},
    {"get_name", t_get_name, METH_VARARGS, "get_name(id) → name oder None"},
    {"set_collision", t_set_collision, METH_VARARGS, "set_collision(id, \"circle\" | \"box\" | None, fest=True, verankert=True)"},
    {"get_touching", t_get_touching, METH_VARARGS, "get_touching(id) → [ids]"},
    {"mouse_down", t_mouse_down, METH_VARARGS, "mouse_down(\"left\" | \"right\" | \"middle\") → bool"},
    {"mouse_pressed", t_mouse_pressed, METH_VARARGS, "mouse_pressed(taste) → bool"},
    {"ui_text", t_ui_text, METH_VARARGS, "ui_text(schluessel, text, x, y, groesse=22, r=1, g=1, b=1, a=1)"},
    {"ui_clear", t_ui_clear, METH_VARARGS, "ui_clear(schluessel=None) – ohne: alle"},
    {"play_sound", t_play_sound, METH_VARARGS, "play_sound(name, lautstaerke=1, schleife=False)"},
    {"stop_sounds", t_stop_sounds, METH_VARARGS, "stop_sounds(name=None) – ohne: alle"},
    {"set_position3d", t_set_position3d, METH_VARARGS, "set_position3d(id, x, y, z) – Weltkoordinaten"},
    {"get_position3d", t_get_position3d, METH_VARARGS, "get_position3d(id) → (x, y, z)"},
    {"set_rotation3d", t_set_rotation3d, METH_VARARGS, "set_rotation3d(id, x, y, z) – Grad"},
    {"get_rotation3d", t_get_rotation3d, METH_VARARGS, "get_rotation3d(id) → (x, y, z)"},
    {"set_scale3d", t_set_scale3d, METH_VARARGS, "set_scale3d(id, x, y=x, z=x)"},
    {"get_scale3d", t_get_scale3d, METH_VARARGS, "get_scale3d(id) → (x, y, z)"},
    {"set_mesh", t_set_mesh, METH_VARARGS, "set_mesh(id, \"cube\" | \"sphere\" | \"plane\" | \"cylinder\" | None, r, g, b)"},
    {"broadcast", t_broadcast, METH_VARARGS, "broadcast(nachricht, daten=None) → on_message in allen Scripts"},
    {"_fehler", t_fehler, METH_VARARGS, NULL},
    {"_nachricht", t_nachricht, METH_VARARGS, NULL},
    {NULL, NULL, 0, NULL},
};

static struct PyModuleDef timbre_modul = {PyModuleDef_HEAD_INIT, "timbre", "Timbre-Engine", -1, timbre_funktionen};

static PyObject* PyInit_timbre(void) { return PyModule_Create(&timbre_modul); }

// ---------------------------------------------------------------------------
// Interpreter starten, laufzeit.py laden
// ---------------------------------------------------------------------------
static PyObject* laufzeit = NULL; // Modul `_laufzeit`
static int kaputt = 0;

static void python_fehler_melden(const char* wo) {
    PyObject *typ, *wert, *tb;
    PyErr_Fetch(&typ, &wert, &tb);
    PyObject* s = wert ? PyObject_Str(wert) : NULL;
    const char* text = s ? PyUnicode_AsUTF8(s) : "?";
    char puffer[1024];
    snprintf(puffer, sizeof(puffer), "Python-Gast (%s): %s", wo, text ? text : "?");
    fehler_c(puffer);
    Py_XDECREF(s);
    Py_XDECREF(typ);
    Py_XDECREF(wert);
    Py_XDECREF(tb);
}

static int starten(void) {
    if (laufzeit) return 1;
    if (kaputt) return 0;
    PyImport_AppendInittab("timbre", PyInit_timbre);

    PyConfig config;
    PyConfig_InitIsolatedConfig(&config);
    config.site_import = 0;
    config.write_bytecode = 0;
    config.install_signal_handlers = 0;
    config.module_search_paths_set = 1;
    config.use_frozen_modules = 1;
    config.buffered_stdio = 0;
    PyWideStringList_Append(&config.module_search_paths, L"/lib/python312.zip");
    PyConfig_SetString(&config, &config.home, L"/lib");
    PyConfig_SetString(&config, &config.program_name, L"timbre");
    PyStatus st = Py_InitializeFromConfig(&config);
    PyConfig_Clear(&config);
    if (PyStatus_Exception(st)) {
        char puffer[512];
        snprintf(puffer, sizeof(puffer), "Python startet nicht: %s %s", st.func ? st.func : "",
                 st.err_msg ? st.err_msg : "");
        fehler_c(puffer);
        kaputt = 1;
        return 0;
    }

    PyObject* code = Py_CompileString(LAUFZEIT_PY, "laufzeit.py", Py_file_input);
    if (code) {
        laufzeit = PyImport_ExecCodeModule("_laufzeit", code);
        Py_DECREF(code);
    }
    if (!laufzeit) {
        python_fehler_melden("laufzeit.py");
        kaputt = 1;
        return 0;
    }
    return 1;
}

// Funktion aus laufzeit.py aufrufen. Fehler in der Laufzeit selbst werden gemeldet.
static PyObject* aufrufen(const char* name, PyObject* args) {
    if (!args) { python_fehler_melden(name); return NULL; }
    PyObject* f = PyObject_GetAttrString(laufzeit, name);
    PyObject* r = f ? PyObject_CallObject(f, args) : NULL;
    Py_XDECREF(f);
    Py_DECREF(args);
    if (!r) python_fehler_melden(name);
    return r;
}

static PyObject* t_broadcast(PyObject* self, PyObject* args) {
    PyObject* f = PyObject_GetAttrString(laufzeit, "broadcast");
    if (!f) return NULL;
    PyObject* r = PyObject_CallObject(f, args);
    Py_DECREF(f);
    return r;
}

// ---------------------------------------------------------------------------
// Exporte (gleiche ABI wie der Luau-Gast)
// ---------------------------------------------------------------------------
EXPORT("timbre_alloc") void* timbre_alloc(int32_t n) { return malloc(n > 0 ? n : 1); }
EXPORT("timbre_free") void timbre_free(void* p) { free(p); }

static int32_t laden(const char* name, int32_t name_n, const char* quelle, int32_t quelle_n, int64_t script_id,
                     int64_t eltern_id, int ersetzen) {
    if (!starten()) return 2;
    PyObject* r = aufrufen("laden", Py_BuildValue("(s#s#LLi)", name, (Py_ssize_t)name_n, quelle, (Py_ssize_t)quelle_n,
                                                  (long long)script_id, (long long)eltern_id, ersetzen));
    if (!r) return 2;
    int32_t ergebnis = (int32_t)PyLong_AsLong(r);
    Py_DECREF(r);
    return ergebnis;
}

EXPORT("timbre_laden") int32_t timbre_laden(const char* name, int32_t name_n, const char* quelle, int32_t quelle_n,
                                            int64_t script_id, int64_t eltern_id) {
    return laden(name, name_n, quelle, quelle_n, script_id, eltern_id, 0);
}

EXPORT("timbre_neu_laden") int32_t timbre_neu_laden(const char* name, int32_t name_n, const char* quelle,
                                                    int32_t quelle_n, int64_t script_id, int64_t eltern_id) {
    return laden(name, name_n, quelle, quelle_n, script_id, eltern_id, 1);
}

EXPORT("timbre_start") void timbre_start(void) {
    if (!starten()) return;
    Py_XDECREF(aufrufen("start", PyTuple_New(0)));
}

EXPORT("timbre_update") void timbre_update(float dt) {
    if (!laufzeit) return;
    Py_XDECREF(aufrufen("update", Py_BuildValue("(d)", (double)dt)));
}

EXPORT("timbre_beruehrung") void timbre_beruehrung(int64_t script_id, int64_t anderes) {
    if (!laufzeit) return;
    Py_XDECREF(aufrufen("beruehrung", Py_BuildValue("(LL)", (long long)script_id, (long long)anderes)));
}

EXPORT("timbre_script_stoppen") void timbre_script_stoppen(int64_t script_id) {
    if (!laufzeit) return;
    Py_XDECREF(aufrufen("stoppen", Py_BuildValue("(L)", (long long)script_id)));
}

EXPORT("timbre_nachricht") void timbre_nachricht(const char* p, int32_t n, int32_t art, double zahl, const char* t,
                                                 int32_t tn) {
    if (!laufzeit) return;
    PyObject* daten;
    switch (art) {
        case 1: daten = PyBool_FromLong(zahl != 0); break;
        case 2: daten = PyFloat_FromDouble(zahl); break;
        case 3: daten = PyUnicode_DecodeUTF8(t, tn, "replace"); break;
        case 4: daten = PyLong_FromLongLong((long long)zahl); break;
        default: daten = Py_NewRef(Py_None);
    }
    PyObject* args = Py_BuildValue("(s#N)", p, (Py_ssize_t)n, daten);
    Py_XDECREF(aufrufen("nachricht", args));
}
