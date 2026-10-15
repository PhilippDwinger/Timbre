"""Erzeugt Logo und Bilder von Timbre (Eis/Arktis-Stil) nach assets/marke/.

Aufruf (Linux/WSL):  python3 tools/marke_erzeugen.py
Braucht Pillow und die Schrift Inter (für die Wortmarke). Alles ist mit Code gezeichnet – keine fremden Bilder.
"""
import math
import os
import random

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ZIEL = os.path.join(os.path.dirname(__file__), "..", "assets", "marke")
os.makedirs(ZIEL, exist_ok=True)

NACHT = (11, 22, 36)
PANEL = (16, 31, 49)
EIS = (191, 232, 255)
AKZENT = (92, 200, 240)
GLETSCHER = (30, 127, 184)
SCHNEE = (228, 241, 250)
NORDLICHT = (47, 184, 142)
BERNSTEIN = (232, 176, 98)  # das Holz im Herzen des Kristalls (timber)

SCHRIFT = None
for kandidat in ("/usr/share/fonts/opentype/inter/InterDisplay-SemiBold.otf",
                 "/usr/share/fonts/opentype/inter/Inter-SemiBold.otf",
                 "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"):
    if os.path.exists(kandidat):
        SCHRIFT = kandidat
        break


def schrift(groesse):
    return ImageFont.truetype(SCHRIFT, groesse) if SCHRIFT else ImageFont.load_default()


def mischen(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def sechseck(m, r, drehung=0.0):
    return [(m[0] + r * math.cos(drehung + i * math.pi / 3), m[1] + r * math.sin(drehung + i * math.pi / 3))
            for i in range(6)]


def kristall(groesse, mit_platte=True):
    """Das Logo: ein Eiskristall (Sechseck mit sechs Armen), im Herzen Jahresringe aus Holz."""
    s = groesse * 4  # Überabtastung für glatte Kanten
    bild = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    m = (s / 2, s / 2)
    if mit_platte:
        # Platte: Sechseck mit Verlauf von Gletscher (unten) zu Eis (oben)
        platte = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        verlauf = Image.new("RGBA", (s, s))
        vd = ImageDraw.Draw(verlauf)
        for y in range(s):
            t = y / s
            vd.line([(0, y), (s, y)], fill=mischen((40, 150, 205), (10, 42, 74), t) + (255,))
        maske = Image.new("L", (s, s), 0)
        ImageDraw.Draw(maske).polygon(sechseck(m, s * 0.47, math.pi / 6), fill=255)
        platte.paste(verlauf, (0, 0), maske)
        # heller Rand
        ImageDraw.Draw(platte).polygon(sechseck(m, s * 0.47, math.pi / 6), outline=EIS + (200,), width=int(s * 0.012))
        # Lichtkante oben links
        glanz = Image.new("L", (s, s), 0)
        ImageDraw.Draw(glanz).polygon([m, sechseck(m, s * 0.47, math.pi / 6)[3], sechseck(m, s * 0.47, math.pi / 6)[4],
                                       sechseck(m, s * 0.47, math.pi / 6)[5]], fill=40)
        weiss = Image.new("RGBA", (s, s), (255, 255, 255, 255))
        platte.paste(weiss, (0, 0), Image.composite(glanz, Image.new("L", (s, s), 0), maske))
        bild.alpha_composite(platte)

    d = ImageDraw.Draw(bild)
    breite = int(s * 0.03)
    arm = s * 0.36
    for i in range(6):
        w = -math.pi / 2 + i * math.pi / 3
        ende = (m[0] + arm * math.cos(w), m[1] + arm * math.sin(w))
        d.line([m, ende], fill=SCHNEE + (255,), width=breite)
        # Seitenzweige
        for pos, laenge in ((0.55, 0.16), (0.8, 0.1)):
            p = (m[0] + arm * pos * math.cos(w), m[1] + arm * pos * math.sin(w))
            for seite in (-1, 1):
                ww = w + seite * math.pi / 3.2
                q = (p[0] + s * laenge * math.cos(ww), p[1] + s * laenge * math.sin(ww))
                d.line([p, q], fill=SCHNEE + (235,), width=int(breite * 0.75))
        # Spitze
        d.ellipse([ende[0] - breite * 0.8, ende[1] - breite * 0.8, ende[0] + breite * 0.8, ende[1] + breite * 0.8],
                  fill=EIS + (255,))
    # Herz: Jahresringe (Holz) in einem Eis-Sechseck
    d.polygon(sechseck(m, s * 0.135, 0), fill=(14, 40, 66, 255), outline=EIS + (255,), width=int(breite * 0.7))
    for k, r in enumerate((0.105, 0.078, 0.052)):
        farbe = mischen(BERNSTEIN, (255, 236, 205), k / 3)
        d.ellipse([m[0] - s * r, m[1] - s * r, m[0] + s * r, m[1] + s * r], outline=farbe + (255,),
                  width=int(s * 0.011))
    d.ellipse([m[0] - s * 0.022, m[1] - s * 0.022, m[0] + s * 0.022, m[1] + s * 0.022], fill=BERNSTEIN + (255,))
    return bild.resize((groesse, groesse), Image.LANCZOS)


def schneeflocken(bild, anzahl, rnd, maxr=3):
    d = ImageDraw.Draw(bild)
    w, h = bild.size
    for _ in range(anzahl):
        x, y, r = rnd.uniform(0, w), rnd.uniform(0, h), rnd.uniform(0.6, maxr)
        a = int(rnd.uniform(60, 200))
        d.ellipse([x - r, y - r, x + r, y + r], fill=SCHNEE + (a,))


def nordlicht(bild, rnd):
    """Weiche Nordlicht-Bänder in einer eigenen Ebene, stark weichgezeichnet."""
    w, h = bild.size
    ebene = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(ebene)
    for band, farbe in enumerate((NORDLICHT, AKZENT, (120, 90, 220))):
        punkte = []
        phase = rnd.uniform(0, 6.28)
        for x in range(0, w + 20, 20):
            y = h * (0.25 + band * 0.08) + math.sin(x / w * 5 + phase) * h * 0.07
            punkte.append((x, y))
        for dy in range(0, int(h * 0.12), 3):
            a = int(70 * (1 - dy / (h * 0.12)))
            d.line([(x, y + dy) for x, y in punkte], fill=farbe + (a,), width=3)
    ebene = ebene.filter(ImageFilter.GaussianBlur(radius=h * 0.03))
    bild.alpha_composite(ebene)


def hintergrund(w, h, rnd):
    bild = Image.new("RGBA", (w, h))
    d = ImageDraw.Draw(bild)
    for y in range(h):
        d.line([(0, y), (w, y)], fill=mischen((18, 40, 66), NACHT, y / h) + (255,))
    nordlicht(bild, rnd)
    # Eisberge / Gletscher am unteren Rand
    berge = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    bd = ImageDraw.Draw(berge)
    for schicht, farbe in enumerate(((70, 120, 160), (40, 82, 118), (24, 52, 80))):
        punkte = [(0, h)]
        x = 0
        hoehe = 0.30 - 0.08 * schicht  # hintere Schicht höher, vordere niedriger
        while x < w:
            punkte.append((x, h - h * hoehe * rnd.uniform(0.45, 1.0)))
            x += rnd.uniform(w * 0.05, w * 0.12)
        punkte += [(w, h)]
        bd.polygon(punkte, fill=farbe + (255,))
    bild.alpha_composite(berge)
    schneeflocken(bild, int(w * h / 3000), rnd)
    return bild


def wortmarke(text_hoehe, mit_studio=False):
    f = schrift(text_hoehe)
    text = "Timbre"
    box = f.getbbox(text)
    tw, th = box[2] - box[0], box[3] - box[1]
    extra = 0
    f2 = schrift(int(text_hoehe * 0.42))
    if mit_studio:
        b2 = f2.getbbox("STUDIO")
        extra = int(text_hoehe * 0.55)
    bild = Image.new("RGBA", (tw + 20, th + 30 + extra), (0, 0, 0, 0))
    # Text mit Verlauf: Maske zeichnen, Verlauf hineinfüllen
    maske = Image.new("L", bild.size, 0)
    ImageDraw.Draw(maske).text((10 - box[0], 10 - box[1]), text, font=f, fill=255)
    verlauf = Image.new("RGBA", bild.size)
    vd = ImageDraw.Draw(verlauf)
    for y in range(bild.size[1]):
        vd.line([(0, y), (bild.size[0], y)], fill=mischen(SCHNEE, AKZENT, y / max(1, th + 20)) + (255,))
    bild.paste(verlauf, (0, 0), maske)
    if mit_studio:
        d = ImageDraw.Draw(bild)
        sb = f2.getbbox("STUDIO")
        d.text((bild.size[0] - (sb[2] - sb[0]) - 14, th + 22), "STUDIO", font=f2, fill=AKZENT + (255,))
    return bild


def speichern(bild, name):
    pfad = os.path.join(ZIEL, name)
    bild.save(pfad, optimize=True)
    print(pfad, bild.size)


rnd = random.Random(7)

# 1. Logo in mehreren Größen (Fenster-Symbol, Leiste, Splash)
for g in (1024, 512, 256, 128, 64, 32):
    speichern(kristall(g), f"logo_{g}.png")
speichern(kristall(512, mit_platte=False), "kristall_512.png")

# 2. Wortmarke
speichern(wortmarke(160), "wortmarke.png")
speichern(wortmarke(160, mit_studio=True), "wortmarke_studio.png")

# 3. Startbild (Splash) 1200×640
splash = hintergrund(1200, 640, rnd)
logo = kristall(300)
glow = logo.filter(ImageFilter.GaussianBlur(18))
splash.alpha_composite(glow, (150, 150))
splash.alpha_composite(logo, (150, 150))
wm = wortmarke(150, mit_studio=True)
splash.alpha_composite(wm, (480, 220))
speichern(splash, "startbild.png")

# 4. Banner für README / GitHub 1600×400
banner = hintergrund(1600, 400, random.Random(11))
logo = kristall(260)
banner.alpha_composite(logo.filter(ImageFilter.GaussianBlur(14)), (120, 70))
banner.alpha_composite(logo, (120, 70))
banner.alpha_composite(wortmarke(140), (420, 110))
d = ImageDraw.Draw(banner)
d.text((432, 285), "Engine · Studio · Luau · Lua · Python", font=schrift(34), fill=EIS + (230,))
speichern(banner, "banner.png")

# 5. Hintergrund für die leere Szene / Projekte 1920×1080
speichern(hintergrund(1920, 1080, random.Random(3)).convert("RGB"), "hintergrund.png")

# 6. Kleine Bilder für Projekte: Eisscholle, Schneeflocke, Eiskristall (als Texturen nutzbar)
def eisscholle(g):
    s = g * 4
    b = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(b)
    r2 = random.Random(5)
    punkte = [(s / 2 + math.cos(w) * s * r2.uniform(0.36, 0.47), s / 2 + math.sin(w) * s * r2.uniform(0.36, 0.47))
              for w in [i * math.tau / 11 for i in range(11)]]
    d.polygon(punkte, fill=(214, 238, 252, 255), outline=(150, 205, 235, 255), width=int(s * 0.02))
    innen = [(s / 2 + (x - s / 2) * 0.75, s / 2 + (y - s / 2) * 0.75 - s * 0.03) for x, y in punkte]
    d.polygon(innen, fill=(236, 248, 255, 255))
    for _ in range(5):
        x, y = s / 2 + r2.uniform(-s * 0.2, s * 0.2), s / 2 + r2.uniform(-s * 0.2, s * 0.2)
        d.line([(x, y), (x + r2.uniform(-s * 0.15, s * 0.15), y + r2.uniform(-s * 0.15, s * 0.15))],
               fill=(170, 215, 240, 255), width=int(s * 0.008))
    return b.resize((g, g), Image.LANCZOS)


speichern(eisscholle(256), "eisscholle.png")
speichern(kristall(128, mit_platte=False), "schneeflocke.png")

# 7. Symbole für das Studio (32 px): Objekt, Script, Bild, Sound, Ordner – als Eis-Rauten/Formen
def symbol(form):
    s = 128
    b = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(b)
    m = (s / 2, s / 2)
    if form == "objekt":
        d.polygon([(64, 14), (114, 64), (64, 114), (14, 64)], fill=GLETSCHER + (120,), outline=EIS + (255,), width=8)
    elif form == "script":
        d.rounded_rectangle([22, 14, 106, 114], 14, fill=PANEL + (255,), outline=AKZENT + (255,), width=8)
        for y in (40, 62, 84):
            d.line([(40, y), (88 - (y % 3) * 8, y)], fill=EIS + (255,), width=8)
    elif form == "live":
        d.polygon([(72, 8), (30, 72), (60, 72), (50, 120), (98, 50), (66, 50)], fill=AKZENT + (255,))
    return b.resize((32, 32), Image.LANCZOS)


for f in ("objekt", "script", "live"):
    speichern(symbol(f), f"symbol_{f}.png")
