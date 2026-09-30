struct Placement {
    origin: vec2<f32>,
    size: vec2<f32>,
    viewport: vec2<f32>,
    padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> placement: Placement;
@group(0) @binding(1) var glyphs: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let pixel = placement.origin + corners[index] * placement.size;
    return vec4(pixel / placement.viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(glyphs, vec2<i32>(position.xy - placement.origin), 0);
}
