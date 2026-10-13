# Timbre Python-Gast: Laufzeit für Scripts (wird in den Gast eingebaut, siehe gast.c).
#
# Gegenstück zur Script-Verwaltung in tools/luau-gast/gast.cpp – gleiche Ideen:
# - jedes Script hat eigene Globals (Umgebung) mit timbre, task, script, shared, print
# - on_start / on_touch / on_message dürfen `async def` sein und mit `await task.wait(s)` warten
# - on_update läuft 60x pro Sekunde und darf nicht warten
# - wartende Aufgaben gehören zu ihrem Script und enden, wenn es angehalten oder neu geladen wird
# - Fehler werden als "Script:Zeile: Fehler" gemeldet und halten nur das betroffene Script an

import heapq
import timbre

_scripts = []      # Liste von _Script
_wartende = []     # Heap: (wann, nummer, script_index, coroutine, start)
_nummer = 0
_gestartet = False
_aktuell = -1
shared = {}        # für alle Python-Scripts dieselbe Tabelle


class _Script:
    __slots__ = ("name", "id", "umgebung", "aktiv")

    def __init__(self, name, sid, umgebung):
        self.name = name
        self.id = sid
        self.umgebung = umgebung
        self.aktiv = False


class _ScriptInfo:
    """`script` in jedem Script: script.name, script.id, script.parent"""

    __slots__ = ("name", "id", "parent")

    def __init__(self, name, sid, parent):
        self.name = name
        self.id = sid
        self.parent = parent

    def __repr__(self):
        return f"<Script {self.name}>"


# ---------------------------------------------------------------------------
# Fehler melden
# ---------------------------------------------------------------------------

def _fehlertext(e, name):
    if isinstance(e, SyntaxError):
        zeile = e.lineno or 0
        return f"{e.filename or name}:{zeile}: Syntaxfehler: {e.msg}"
    # Letzte Stelle im Traceback, die in einem Script liegt (nicht in der Laufzeit selbst)
    datei, zeile = name, 0
    tb = e.__traceback__
    namen = {s.name for s in _scripts}
    while tb is not None:
        f = tb.tb_frame.f_code.co_filename
        if f in namen or f == name:
            datei, zeile = f, tb.tb_lineno
        tb = tb.tb_next
    text = str(e)
    art = type(e).__name__
    return f"{datei}:{zeile}: {art}: {text}" if text else f"{datei}:{zeile}: {art}"


def _melde(e, index):
    name = _scripts[index].name if 0 <= index < len(_scripts) else "?"
    timbre._fehler(_fehlertext(e, name))


# ---------------------------------------------------------------------------
# Aufgaben: task.wait / task.delay / task.spawn
# ---------------------------------------------------------------------------

class _Warten:
    __slots__ = ("sekunden",)

    def __init__(self, sekunden):
        self.sekunden = sekunden

    def __await__(self):
        vergangen = yield self
        return vergangen


def _planen(sekunden, index, coro, start):
    global _nummer
    _nummer += 1
    heapq.heappush(_wartende, (timbre.time() + max(0.0, sekunden), _nummer, index, coro, start))


def _weiter(index, coro, wert):
    """Coroutine bis zum nächsten `await task.wait(...)` laufen lassen."""
    global _aktuell
    vorher = _aktuell
    _aktuell = index
    try:
        w = coro.send(wert)
    except StopIteration:
        return True
    except BaseException as e:
        _melde(e, index)
        return False
    finally:
        _aktuell = vorher
    if isinstance(w, _Warten):
        _planen(w.sekunden, index, coro, timbre.time())
        return True
    coro.close()
    timbre._fehler(_scripts[index].name + ": In Timbre-Scripts geht nur `await task.wait(sekunden)`.")
    return False


def _ausfuehren(index, f, args):
    """Funktion aufrufen; ist sie `async`, als Aufgabe starten. False bei Fehler."""
    global _aktuell
    vorher = _aktuell
    _aktuell = index
    try:
        ergebnis = f(*args)
    except BaseException as e:
        _melde(e, index)
        return False
    finally:
        _aktuell = vorher
    if hasattr(ergebnis, "send") and hasattr(ergebnis, "throw"):
        return _weiter(index, ergebnis, None)
    return True


class _Task:
    """`task` in Scripts – wie in Luau."""

    @staticmethod
    def wait(sekunden=0):
        """await task.wait(sekunden) – nur in `async def`-Funktionen."""
        return _Warten(float(sekunden))

    @staticmethod
    def delay(sekunden, f, *args):
        """f(*args) nach `sekunden` aufrufen."""
        index = _aktuell

        async def spaeter():
            await _Warten(float(sekunden))
            r = f(*args)
            if hasattr(r, "__await__"):
                await r

        _weiter(index, spaeter(), None)

    @staticmethod
    def spawn(f, *args):
        """f(*args) sofort als eigene Aufgabe (darf warten)."""
        _ausfuehren(_aktuell, f, args)


task = _Task()


def _aufgaben_wecken():
    jetzt = timbre.time()
    while _wartende and _wartende[0][0] <= jetzt:
        _, _, index, coro, start = heapq.heappop(_wartende)
        if 0 <= index < len(_scripts) and not _scripts[index].aktiv:
            coro.close()  # Script angehalten oder neu geladen → Aufgabe endet
            continue
        _weiter(index, coro, jetzt - start)


# ---------------------------------------------------------------------------
# Nachrichten
# ---------------------------------------------------------------------------

def broadcast(nachricht, daten=None):
    """timbre.broadcast(nachricht, daten) → on_message(nachricht, daten) in allen Scripts."""
    nachricht = str(nachricht)
    # An andere Sprachen über die Engine (nur einfache Daten)
    if daten is None or isinstance(daten, (bool, int, float, str)):
        timbre._nachricht(nachricht, daten)
    else:
        timbre._nachricht(nachricht, None)
    _zustellen(nachricht, daten)


def _zustellen(nachricht, daten):
    for i, s in enumerate(_scripts):
        if s.aktiv:
            f = s.umgebung.get("on_message")
            if callable(f):
                _ausfuehren(i, f, _argumente(f, (nachricht, daten)))


def _argumente(f, args):
    """Nur so viele Argumente übergeben, wie die Funktion annimmt (wie in Lua: on_message(m) reicht)."""
    try:
        code = f.__code__
        if code.co_flags & 0x04:  # *args
            return args
        return args[: code.co_argcount]
    except AttributeError:
        return args


# ---------------------------------------------------------------------------
# Von der Engine aufgerufen (über gast.c)
# ---------------------------------------------------------------------------

def _drucken(*werte, sep=" ", end="", **_):
    timbre.log(sep.join(str(w) for w in werte))


def laden(name, quelle, script_id, eltern_id, alte_ersetzen):
    """0 = ok, 1 = Syntaxfehler, 2 = Laufzeitfehler"""
    try:
        code = compile(quelle, name, "exec")
    except SyntaxError as e:
        text = _fehlertext(e, name)
        timbre._fehler(text + "  (alte Fassung läuft weiter)" if alte_ersetzen else text)
        return 1
    if alte_ersetzen:
        for s in _scripts:
            if s.id == script_id:
                s.aktiv = False
    umgebung = {
        "__name__": "__main__",
        "__builtins__": __builtins__,
        "timbre": timbre,
        "task": task,
        "shared": shared,
        "print": _drucken,
        "script": _ScriptInfo(name, script_id or None, eltern_id or None),
    }
    index = len(_scripts)
    s = _Script(name, script_id, umgebung)
    _scripts.append(s)
    global _aktuell
    vorher = _aktuell
    _aktuell = index
    try:
        exec(code, umgebung)
    except BaseException as e:
        _melde(e, index)
        return 2
    finally:
        _aktuell = vorher
    s.aktiv = True
    if alte_ersetzen and _gestartet:
        f = umgebung.get("on_start")
        if callable(f) and not _ausfuehren(index, f, ()):
            s.aktiv = False
    return 0


def start():
    global _gestartet
    _gestartet = True
    for i, s in enumerate(_scripts):
        if s.aktiv:
            f = s.umgebung.get("on_start")
            if callable(f) and not _ausfuehren(i, f, ()):
                s.aktiv = False


def update(dt):
    global _aktuell
    for i, s in enumerate(_scripts):
        if not s.aktiv:
            continue
        f = s.umgebung.get("on_update")
        if not callable(f):
            continue
        _aktuell = i
        try:
            r = f(dt)
            if hasattr(r, "close") and hasattr(r, "send"):
                r.close()
                raise RuntimeError("on_update darf nicht `async` sein – warten geht nur in on_start, on_touch, on_message")
        except BaseException as e:
            _melde(e, i)
            timbre._fehler(s.name + " wurde angehalten.")
            s.aktiv = False
        finally:
            _aktuell = -1
    _aufgaben_wecken()


def beruehrung(script_id, anderes):
    for i, s in enumerate(_scripts):
        if s.id == script_id and s.aktiv:
            f = s.umgebung.get("on_touch")
            if callable(f):
                _ausfuehren(i, f, _argumente(f, (anderes,)))


def stoppen(script_id):
    for s in _scripts:
        if s.id == script_id:
            s.aktiv = False


def nachricht(name, daten):
    """Nachricht aus einer anderen Sprache – nicht noch einmal weiterreichen."""
    _zustellen(name, daten)
