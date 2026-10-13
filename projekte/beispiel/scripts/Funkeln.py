# Funkeln: ein Script in Python 3 (die anderen sind Luau und Lua).
# Wird ein Goldstück eingesammelt (Nachricht "gesammelt" aus Steuerung.luau),
# sprühen ein paar goldene Funken um den Spieler.
#
# Warten geht in Python mit async/await:  async def ...  und  await task.wait(sekunden)

import math
import random

ANZAHL = 10


async def on_message(nachricht, daten=None):
    if nachricht != "gesammelt":
        return
    spieler = timbre.find("Spieler")
    if spieler is None:
        return
    x, y = timbre.get_position(spieler)

    funken = []
    for i in range(ANZAHL):
        f = timbre.spawn("Funke")
        timbre.set_sprite(f, "blatt", 14, 14, 5)
        timbre.set_color(f, 1.0, random.uniform(0.75, 0.95), 0.3)
        timbre.set_position(f, x, y)
        winkel = i / ANZAHL * math.tau + random.uniform(-0.2, 0.2)
        tempo = random.uniform(140, 240)
        timbre.set_velocity(f, math.cos(winkel) * tempo, math.sin(winkel) * tempo, 8)
        funken.append(f)

    await task.wait(0.35)
    for f in funken:
        timbre.destroy(f)
