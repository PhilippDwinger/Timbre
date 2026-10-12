# Beispiel: dasselbe Script in Python (läuft ab Meilenstein 6)
import math
import timbre

kiste = None

def on_start():
    global kiste
    kiste = timbre.spawn("Kiste")
    timbre.log(f"Kiste erzeugt: {kiste}")

def on_update(dt):
    t = timbre.time()
    timbre.set_position(kiste, math.sin(t) * 2, 0, 0)
