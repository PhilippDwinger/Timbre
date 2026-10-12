// Meilenstein 1: farbiges Dreieck. Die Eckpunkte stehen direkt im Shader,
// es gibt also noch keinen Vertex-Buffer.

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) farbe: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    var positionen = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.6),
        vec2<f32>(-0.6, -0.6),
        vec2<f32>( 0.6, -0.6),
    );
    var farben = array<vec3<f32>, 3>(
        vec3<f32>(0.90, 0.35, 0.25), // rot
        vec3<f32>(0.30, 0.75, 0.40), // gruen
        vec3<f32>(0.25, 0.45, 0.90), // blau
    );
    var out: VertexOut;
    out.position = vec4<f32>(positionen[i], 0.0, 1.0);
    out.farbe = farben[i];
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.farbe, 1.0);
}
