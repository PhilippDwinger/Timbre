// 3D-Meshes mit Instancing: Würfel, Kugeln, Ebenen … in einer Farbe, mit weichem Licht.
// Stil passend zu Timbre: kühles Himmelslicht von oben, warmes Gegenlicht, leichter Nebel in der Ferne.

struct Szene {
    ansicht_projektion: mat4x4<f32>,
    kamera: vec4<f32>,  // xyz = Kameraposition
    licht: vec4<f32>,   // xyz = Richtung zum Licht (normiert)
    nebel: vec4<f32>,   // rgb = Nebelfarbe, w = Dichte
};
@group(0) @binding(0) var<uniform> szene: Szene;

struct Ecke {
    @location(0) position: vec3<f32>,
    @location(1) normale: vec3<f32>,
};

struct Instanz {
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    @location(6) farbe: vec4<f32>,
    @location(7) extra: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) welt: vec3<f32>,
    @location(1) normale: vec3<f32>,
    @location(2) farbe: vec4<f32>,
    @location(3) unbeleuchtet: f32,
};

@vertex
fn vs_main(e: Ecke, i: Instanz) -> VertexOut {
    let modell = mat4x4<f32>(i.m0, i.m1, i.m2, i.m3);
    let welt = modell * vec4<f32>(e.position, 1.0);
    var aus: VertexOut;
    aus.clip = szene.ansicht_projektion * welt;
    aus.welt = welt.xyz;
    // Normale mit der Modellmatrix drehen (für gleichmäßige Skalierung genau genug)
    aus.normale = normalize((modell * vec4<f32>(e.normale, 0.0)).xyz);
    aus.farbe = i.farbe;
    aus.unbeleuchtet = i.extra.x;
    return aus;
}

@fragment
fn fs_main(v: VertexOut) -> @location(0) vec4<f32> {
    if (v.unbeleuchtet > 0.5) {
        return v.farbe;
    }
    let n = normalize(v.normale);
    let l = normalize(szene.licht.xyz);
    let zur_kamera = normalize(szene.kamera.xyz - v.welt);
    // Himmel (kühl) und Boden (dunkler) als Grundlicht, dazu die Sonne
    let himmel = mix(vec3<f32>(0.10, 0.13, 0.18), vec3<f32>(0.42, 0.55, 0.68), n.y * 0.5 + 0.5);
    let sonne = max(dot(n, l), 0.0) * vec3<f32>(1.0, 0.96, 0.9);
    // Glanzlicht (Eis glänzt) und heller Rand
    let halb = normalize(l + zur_kamera);
    let glanz = pow(max(dot(n, halb), 0.0), 48.0) * 0.35;
    let rand = pow(1.0 - max(dot(n, zur_kamera), 0.0), 3.0) * 0.25;
    var farbe = v.farbe.rgb * (himmel * 0.6 + sonne * 0.62) + vec3<f32>(glanz + rand * 0.6, glanz + rand * 0.8, glanz + rand);
    // Nebel mit der Entfernung
    let abstand = length(szene.kamera.xyz - v.welt);
    let nebel = 1.0 - exp(-abstand * szene.nebel.w);
    farbe = mix(farbe, szene.nebel.rgb, clamp(nebel, 0.0, 1.0));
    return vec4<f32>(farbe, v.farbe.a);
}
