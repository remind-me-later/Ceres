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
uniform highp sampler2D _group_0_binding_0_fs;

uniform type_3_block_0Fragment { uint _group_1_binding_1_fs; };

layout(location = 0) smooth in vec2 _vs2fs_location0;
layout(location = 0) out vec4 _fs2p_location0;

bool neq(vec3 a, vec3 b) {
    return any(notEqual(a, b));
}

bool eq(vec3 a_1, vec3 b_1) {
    return all(equal(a_1, b_1));
}

vec4 fs_scale2x(vec2 tex_coords) {
    vec2 dims_1 = vec2(textureSize(_group_0_binding_0_fs, 0).xy);
    vec2 off = (vec2(1.0, 1.0) / dims_1);
    vec4 _e10 = texture(_group_0_binding_0_fs, vec2(tex_coords));
    vec3 p = _e10.xyz;
    vec4 _e19 = texture(_group_0_binding_0_fs, vec2((tex_coords + vec2(0.0, -(off.y)))));
    vec3 a_2 = _e19.xyz;
    vec4 _e28 = texture(_group_0_binding_0_fs, vec2((tex_coords + vec2(-(off.x), 0.0))));
    vec3 c = _e28.xyz;
    vec4 _e36 = texture(_group_0_binding_0_fs, vec2((tex_coords + vec2(off.x, 0.0))));
    vec3 b_2 = _e36.xyz;
    vec4 _e44 = texture(_group_0_binding_0_fs, vec2((tex_coords + vec2(0.0, off.y))));
    vec3 d = _e44.xyz;
    bool _e46 = eq(c, a_2);
    bool _e47 = neq(c, d);
    bool _e49 = neq(a_2, b_2);
    vec3 p0_ = (((_e46 && _e47) && _e49) ? a_2 : p);
    bool _e52 = eq(a_2, b_2);
    bool _e53 = neq(a_2, c);
    bool _e55 = neq(b_2, d);
    vec3 p1_ = (((_e52 && _e53) && _e55) ? b_2 : p);
    bool _e58 = eq(d, c);
    bool _e59 = neq(d, b_2);
    bool _e61 = neq(c, a_2);
    vec3 p2_ = (((_e58 && _e59) && _e61) ? c : p);
    bool _e64 = eq(b_2, d);
    bool _e65 = neq(b_2, a_2);
    bool _e67 = neq(d, c);
    vec3 p3_ = (((_e64 && _e65) && _e67) ? d : p);
    vec2 pp = floor((2.0 * fract((tex_coords * dims_1))));
    vec3 ret_1 = ((pp.y == 0.0) ? ((pp.x == 0.0) ? p0_ : p1_) : ((pp.x == 0.0) ? p2_ : p3_));
    return vec4(ret_1, 1.0);
}

vec4 fs_scale3x(vec2 tex_coords_1) {
    vec2 dims_2 = vec2(textureSize(_group_0_binding_0_fs, 0).xy);
    vec2 off_1 = (vec2(1.0, 1.0) / dims_2);
    vec4 _e10 = texture(_group_0_binding_0_fs, vec2(tex_coords_1));
    vec3 p_1 = _e10.xyz;
    vec4 _e20 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(-(off_1.x), -(off_1.y)))));
    vec3 a_3 = _e20.xyz;
    vec4 _e29 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(0.0, -(off_1.y)))));
    vec3 b_3 = _e29.xyz;
    vec4 _e38 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(off_1.x, -(off_1.y)))));
    vec3 c_1 = _e38.xyz;
    vec4 _e47 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(-(off_1.x), 0.0))));
    vec3 d_1 = _e47.xyz;
    vec4 _e55 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(off_1.x, 0.0))));
    vec3 f = _e55.xyz;
    vec4 _e64 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(-(off_1.x), off_1.y))));
    vec3 g = _e64.xyz;
    vec4 _e72 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(0.0, off_1.y))));
    vec3 h = _e72.xyz;
    vec4 _e80 = texture(_group_0_binding_0_fs, vec2((tex_coords_1 + vec2(off_1.x, off_1.y))));
    vec3 i = _e80.xyz;
    bool _e82 = eq(d_1, b_3);
    bool _e83 = neq(d_1, h);
    bool _e85 = neq(b_3, f);
    vec3 p0_1 = (((_e82 && _e83) && _e85) ? d_1 : p_1);
    bool _e88 = eq(d_1, b_3);
    bool _e89 = neq(d_1, h);
    bool _e91 = neq(b_3, f);
    bool _e93 = neq(p_1, c_1);
    bool _e95 = eq(b_3, f);
    bool _e96 = neq(b_3, d_1);
    bool _e98 = neq(f, h);
    bool _e100 = neq(p_1, a_3);
    vec3 p1_1 = (((((_e88 && _e89) && _e91) && _e93) || (((_e95 && _e96) && _e98) && _e100)) ? b_3 : p_1);
    bool _e104 = eq(b_3, f);
    bool _e105 = neq(b_3, d_1);
    bool _e107 = neq(f, h);
    vec3 p2_1 = (((_e104 && _e105) && _e107) ? f : p_1);
    bool _e110 = eq(h, d_1);
    bool _e111 = neq(h, f);
    bool _e113 = neq(d_1, b_3);
    bool _e115 = neq(p_1, a_3);
    bool _e117 = eq(d_1, b_3);
    bool _e118 = neq(d_1, h);
    bool _e120 = neq(b_3, f);
    bool _e122 = neq(p_1, g);
    vec3 p3_1 = (((((_e110 && _e111) && _e113) && _e115) || (((_e117 && _e118) && _e120) && _e122)) ? d_1 : p_1);
    bool _e126 = eq(b_3, f);
    bool _e127 = neq(b_3, d_1);
    bool _e129 = neq(f, h);
    bool _e131 = neq(p_1, i);
    bool _e133 = eq(f, h);
    bool _e134 = neq(f, b_3);
    bool _e136 = neq(h, d_1);
    bool _e138 = neq(p_1, c_1);
    vec3 p5_ = (((((_e126 && _e127) && _e129) && _e131) || (((_e133 && _e134) && _e136) && _e138)) ? f : p_1);
    bool _e142 = eq(h, d_1);
    bool _e143 = neq(h, f);
    bool _e145 = neq(d_1, b_3);
    vec3 p6_ = (((_e142 && _e143) && _e145) ? d_1 : p_1);
    bool _e148 = eq(f, h);
    bool _e149 = neq(f, b_3);
    bool _e151 = neq(h, d_1);
    bool _e153 = neq(p_1, g);
    bool _e155 = eq(h, d_1);
    bool _e156 = neq(h, f);
    bool _e158 = neq(d_1, b_3);
    bool _e160 = neq(p_1, i);
    vec3 p7_ = (((((_e148 && _e149) && _e151) && _e153) || (((_e155 && _e156) && _e158) && _e160)) ? h : p_1);
    bool _e164 = eq(f, h);
    bool _e165 = neq(f, b_3);
    bool _e167 = neq(h, d_1);
    vec3 p8_ = (((_e164 && _e165) && _e167) ? f : p_1);
    vec2 pp_1 = floor((3.0 * fract((tex_coords_1 * dims_2))));
    vec3 ret_2 = ((pp_1.y == 0.0) ? ((pp_1.x == 0.0) ? p0_1 : ((pp_1.x == 1.0) ? p1_1 : p2_1)) : ((pp_1.y == 1.0) ? ((pp_1.x == 0.0) ? p3_1 : ((pp_1.x == 1.0) ? p_1 : p5_)) : ((pp_1.x == 0.0) ? p6_ : ((pp_1.x == 1.0) ? p7_ : p8_))));
    return vec4(ret_2, 1.0);
}

void main() {
    VertexOutput in_1 = VertexOutput(gl_FragCoord, _vs2fs_location0);
    vec4 ret = vec4(0.0);
    uint _e3 = _group_1_binding_1_fs;
    switch(_e3) {
        default: {
            vec4 _e7 = texture(_group_0_binding_0_fs, vec2(in_1.tex_coords));
            ret = _e7;
            break;
        }
        case 1u: {
            vec4 _e9 = fs_scale2x(in_1.tex_coords);
            ret = _e9;
            break;
        }
        case 2u: {
            vec4 _e11 = fs_scale3x(in_1.tex_coords);
            ret = _e11;
            break;
        }
    }
    vec4 _e12 = ret;
    _fs2p_location0 = _e12;
    return;
}

