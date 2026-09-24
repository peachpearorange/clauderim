#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::{globals, view},
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    let tall = vertex.uv.x;
    let rise = vertex.uv.y;
    let reach = distance(world.xz, view.world_position.xz);
    let shown = 1.0 - smoothstep(50.0, 74.0, reach);
    let time = globals.time;
    let gust = 0.5 + 0.5 * sin(time * 0.7 + world.x * 0.045 + world.z * 0.03);
    let flutter = sin(time * 2.3 + world.x * 0.9 + world.z * 0.7) + 0.5 * sin(time * 3.7 - world.x * 1.3 + world.z * 0.4);
    let bend = rise * rise * tall;
    world.x += bend * (0.08 + 0.2 * gust + 0.06 * flutter);
    world.z += bend * (0.04 * gust + 0.05 * flutter);
    world.y -= rise * tall * (1.0 - shown) * 1.05;
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}
