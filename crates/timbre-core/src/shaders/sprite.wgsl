// Sprites mit Instancing: ein Rechteck (6 Eckpunkte), das pro Sprite
// verschoben, skaliert, gedreht und eingefärbt wird.
// Alle Sprites mit derselben Textur landen in EINEM Zeichenaufruf.

struct Kamera {
    matrix: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> kamera: Kamera;

@group(1) @binding(0) var textur: texture_2d<f32>;
@group(1) @binding(1) var abtaster: sampler;

struct Instanz {
    @location(0) position: vec2<f32>,
    @location(1) groesse: vec2<f32>,
    @location(2) drehung: f32,
    @location(3) farbe: vec4<f32>,
    @location(4) uv: vec4<f32>, // x, y, breite, hoehe im Texturraum (0..1)
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) farbe: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32, inst: Instanz) -> VertexOut {
    var ecken = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, -0.5), vec2<f32>( 0.5, -0.5), vec2<f32>( 0.5,  0.5),
        vec2<f32>(-0.5, -0.5), vec2<f32>( 0.5,  0.5), vec2<f32>(-0.5,  0.5),
    );
    let e = ecken[i];
    let lokal = e * inst.groesse;
    let c = cos(inst.drehung);
    let s = sin(inst.drehung);
    let welt = inst.position + vec2<f32>(lokal.x * c - lokal.y * s, lokal.x * s + lokal.y * c);

    var out: VertexOut;
    out.clip = kamera.matrix * vec4<f32>(welt, 0.0, 1.0);
    // Texturkoordinaten: (0,0) = links oben
    out.uv = inst.uv.xy + vec2<f32>(e.x + 0.5, 0.5 - e.y) * inst.uv.zw;
    out.farbe = inst.farbe;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(textur, abtaster, in.uv) * in.farbe;
}
