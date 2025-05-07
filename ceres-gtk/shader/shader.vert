#version 310 es

precision highp float;
precision highp int;

struct Vertexinput {
    uint vert_idx;
};
struct VertexOutput {
    vec4 clip_position;
    vec2 tex_coords;
};
uniform vec2 _group_1_binding_0_vs;

layout(location = 0) smooth out vec2 _vs2fs_location0;

bool eq(vec3 a, vec3 b) {
    return all(equal(a, b));
}

bool neq(vec3 a_1, vec3 b_1) {
    return any(notEqual(a_1, b_1));
}

void main() {
    Vertexinput in_ = Vertexinput(uint(gl_VertexID));
    VertexOutput out_ = VertexOutput(vec4(0.0), vec2(0.0));
    vec2 vert_coord = ((in_.vert_idx == 0u) ? vec2(-1.0, -1.0) : ((in_.vert_idx == 1u) ? vec2(-1.0, 1.0) : ((in_.vert_idx == 2u) ? vec2(1.0, -1.0) : vec2(1.0, 1.0))));
    vec2 _e28 = _group_1_binding_0_vs;
    out_.clip_position = vec4((vert_coord * _e28), 0.0, 1.0);
    out_.tex_coords = clamp(vec2(vert_coord.x, -(vert_coord.y)), vec2(0.0), vec2(1.0));
    VertexOutput _e39 = out_;
    gl_Position = _e39.clip_position;
    _vs2fs_location0 = _e39.tex_coords;
    gl_Position.yz = vec2(gl_Position.y, gl_Position.z * 2.0 - gl_Position.w);
    return;
}

