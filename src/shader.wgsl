@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32>{
    let positions = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.7),
        vec2<f32>(-0.7, -0.7),
        vec2<f32>( 0.7, -0.7),
    );
    let pos = positions[vertex_index];
    return vec4<f32>(pos, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32>{
    return vec4<f32>(0.2, 0.8, 1.0, 1.0);
}
