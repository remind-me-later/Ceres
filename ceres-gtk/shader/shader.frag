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

uniform highp sampler2D _group_0_binding_2_fs;

uniform type_3_block_0Fragment { uint _group_1_binding_1_fs; };

layout(location = 0) smooth in vec2 _vs2fs_location0;
layout(location = 0) out vec4 _fs2p_location0;

vec4 desaturated(highp sampler2D txt, vec2 tex_coords) {
    vec4 tex = texture(txt, vec2(tex_coords));
    float luminance = (((tex.x * 0.299) + (tex.y * 0.587)) + (tex.z * 0.114));
    vec3 desaturated_color = vec3(mix(luminance, tex.x, 0.7), mix(luminance, tex.y, 0.7), mix(luminance, tex.z, 0.7));
    return vec4(desaturated_color, tex.w);
}

vec4 get_sample(vec2 tex_coords_1) {
    vec4 _e2 = desaturated(_group_0_binding_0_fs, tex_coords_1);
    return _e2;
}

bool eq(vec3 a, vec3 b) {
    return all(equal(a, b));
}

bool neq(vec3 a_1, vec3 b_1) {
    return any(notEqual(a_1, b_1));
}

vec4 fs_scale2x(vec2 tex_coords_2) {
    vec2 dims_1 = vec2(uvec2(textureSize(_group_0_binding_0_fs, 0).xy));
    vec2 off = (vec2(1.0, 1.0) / dims_1);
    vec4 _e8 = get_sample(tex_coords_2);
    vec3 p = _e8.xyz;
    vec4 _e15 = get_sample((tex_coords_2 + vec2(0.0, -(off.y))));
    vec3 a_2 = _e15.xyz;
    vec4 _e22 = get_sample((tex_coords_2 + vec2(-(off.x), 0.0)));
    vec3 c = _e22.xyz;
    vec4 _e28 = get_sample((tex_coords_2 + vec2(off.x, 0.0)));
    vec3 b_2 = _e28.xyz;
    vec4 _e34 = get_sample((tex_coords_2 + vec2(0.0, off.y)));
    vec3 d = _e34.xyz;
    bool _e36 = eq(c, a_2);
    bool _e37 = neq(c, d);
    bool _e39 = neq(a_2, b_2);
    vec3 p0_ = (((_e36 && _e37) && _e39) ? a_2 : p);
    bool _e42 = eq(a_2, b_2);
    bool _e43 = neq(a_2, c);
    bool _e45 = neq(b_2, d);
    vec3 p1_ = (((_e42 && _e43) && _e45) ? b_2 : p);
    bool _e48 = eq(d, c);
    bool _e49 = neq(d, b_2);
    bool _e51 = neq(c, a_2);
    vec3 p2_ = (((_e48 && _e49) && _e51) ? c : p);
    bool _e54 = eq(b_2, d);
    bool _e55 = neq(b_2, a_2);
    bool _e57 = neq(d, c);
    vec3 p3_ = (((_e54 && _e55) && _e57) ? d : p);
    vec2 pp = floor((2.0 * fract((tex_coords_2 * dims_1))));
    vec3 ret_1 = ((pp.y == 0.0) ? ((pp.x == 0.0) ? p0_ : p1_) : ((pp.x == 0.0) ? p2_ : p3_));
    return vec4(ret_1, 1.0);
}

vec4 fs_scale3x(vec2 tex_coords_3) {
    vec2 dims_2 = vec2(uvec2(textureSize(_group_0_binding_0_fs, 0).xy));
    vec2 off_1 = (vec2(1.0, 1.0) / dims_2);
    vec4 _e8 = get_sample(tex_coords_3);
    vec3 p_1 = _e8.xyz;
    vec4 _e16 = get_sample((tex_coords_3 + vec2(-(off_1.x), -(off_1.y))));
    vec3 a_3 = _e16.xyz;
    vec4 _e23 = get_sample((tex_coords_3 + vec2(0.0, -(off_1.y))));
    vec3 b_3 = _e23.xyz;
    vec4 _e30 = get_sample((tex_coords_3 + vec2(off_1.x, -(off_1.y))));
    vec3 c_1 = _e30.xyz;
    vec4 _e37 = get_sample((tex_coords_3 + vec2(-(off_1.x), 0.0)));
    vec3 d_1 = _e37.xyz;
    vec4 _e43 = get_sample((tex_coords_3 + vec2(off_1.x, 0.0)));
    vec3 f = _e43.xyz;
    vec4 _e50 = get_sample((tex_coords_3 + vec2(-(off_1.x), off_1.y)));
    vec3 g = _e50.xyz;
    vec4 _e56 = get_sample((tex_coords_3 + vec2(0.0, off_1.y)));
    vec3 h = _e56.xyz;
    vec4 _e62 = get_sample((tex_coords_3 + vec2(off_1.x, off_1.y)));
    vec3 i = _e62.xyz;
    bool _e64 = eq(d_1, b_3);
    bool _e65 = neq(d_1, h);
    bool _e67 = neq(b_3, f);
    vec3 p0_1 = (((_e64 && _e65) && _e67) ? d_1 : p_1);
    bool _e70 = eq(d_1, b_3);
    bool _e71 = neq(d_1, h);
    bool _e73 = neq(b_3, f);
    bool _e75 = neq(p_1, c_1);
    bool _e77 = eq(b_3, f);
    bool _e78 = neq(b_3, d_1);
    bool _e80 = neq(f, h);
    bool _e82 = neq(p_1, a_3);
    vec3 p1_1 = (((((_e70 && _e71) && _e73) && _e75) || (((_e77 && _e78) && _e80) && _e82)) ? b_3 : p_1);
    bool _e86 = eq(b_3, f);
    bool _e87 = neq(b_3, d_1);
    bool _e89 = neq(f, h);
    vec3 p2_1 = (((_e86 && _e87) && _e89) ? f : p_1);
    bool _e92 = eq(h, d_1);
    bool _e93 = neq(h, f);
    bool _e95 = neq(d_1, b_3);
    bool _e97 = neq(p_1, a_3);
    bool _e99 = eq(d_1, b_3);
    bool _e100 = neq(d_1, h);
    bool _e102 = neq(b_3, f);
    bool _e104 = neq(p_1, g);
    vec3 p3_1 = (((((_e92 && _e93) && _e95) && _e97) || (((_e99 && _e100) && _e102) && _e104)) ? d_1 : p_1);
    bool _e108 = eq(b_3, f);
    bool _e109 = neq(b_3, d_1);
    bool _e111 = neq(f, h);
    bool _e113 = neq(p_1, i);
    bool _e115 = eq(f, h);
    bool _e116 = neq(f, b_3);
    bool _e118 = neq(h, d_1);
    bool _e120 = neq(p_1, c_1);
    vec3 p5_ = (((((_e108 && _e109) && _e111) && _e113) || (((_e115 && _e116) && _e118) && _e120)) ? f : p_1);
    bool _e124 = eq(h, d_1);
    bool _e125 = neq(h, f);
    bool _e127 = neq(d_1, b_3);
    vec3 p6_ = (((_e124 && _e125) && _e127) ? d_1 : p_1);
    bool _e130 = eq(f, h);
    bool _e131 = neq(f, b_3);
    bool _e133 = neq(h, d_1);
    bool _e135 = neq(p_1, g);
    bool _e137 = eq(h, d_1);
    bool _e138 = neq(h, f);
    bool _e140 = neq(d_1, b_3);
    bool _e142 = neq(p_1, i);
    vec3 p7_ = (((((_e130 && _e131) && _e133) && _e135) || (((_e137 && _e138) && _e140) && _e142)) ? h : p_1);
    bool _e146 = eq(f, h);
    bool _e147 = neq(f, b_3);
    bool _e149 = neq(h, d_1);
    vec3 p8_ = (((_e146 && _e147) && _e149) ? f : p_1);
    vec2 pp_1 = floor((3.0 * fract((tex_coords_3 * dims_2))));
    vec3 ret_2 = ((pp_1.y == 0.0) ? ((pp_1.x == 0.0) ? p0_1 : ((pp_1.x == 1.0) ? p1_1 : p2_1)) : ((pp_1.y == 1.0) ? ((pp_1.x == 0.0) ? p3_1 : ((pp_1.x == 1.0) ? p_1 : p5_)) : ((pp_1.x == 0.0) ? p6_ : ((pp_1.x == 1.0) ? p7_ : p8_))));
    return vec4(ret_2, 1.0);
}

vec4 get_ghost_sample(vec2 tex_coords_4) {
    vec4 _e2 = desaturated(_group_0_binding_2_fs, tex_coords_4);
    return _e2;
}

vec4 fs_lcd(vec2 tex_coords_5) {
    vec2 dims_3 = vec2(uvec2(textureSize(_group_0_binding_0_fs, 0).xy));
    vec2 pixel_pos = (tex_coords_5 * dims_3);
    vec2 red_offset = vec2((-0.3 / dims_3.x), 0.0);
    vec2 green_offset = vec2(0.0, 0.0);
    vec2 blue_offset = vec2((0.3 / dims_3.x), 0.0);
    vec4 _e34 = get_sample((tex_coords_5 + red_offset));
    float red_sample = _e34.x;
    vec4 _e37 = get_sample((tex_coords_5 + green_offset));
    float green_sample = _e37.y;
    vec4 _e40 = get_sample((tex_coords_5 + blue_offset));
    float blue_sample = _e40.z;
    vec3 pixel = vec3(red_sample, green_sample, blue_sample);
    vec3 lcd_tint = vec3(0.93, 0.96, 0.9);
    vec2 grid = vec2(smoothstep(0.4, 0.6, fract(pixel_pos.x)), smoothstep(0.4, 0.6, fract(pixel_pos.y)));
    float grid_effect = (0.5 + (0.25 * (1.0 - ((grid.x + grid.y) * 0.5))));
    float vignette = (1.0 - length(((tex_coords_5 - vec2(0.5)) * 1.0)));
    float reflection = (smoothstep(0.0, 0.9, vignette) * 0.1);
    vec3 final_color = ((pixel * lcd_tint) * grid_effect);
    vec4 _e69 = get_ghost_sample(tex_coords_5);
    vec3 prev_color = _e69.xyz;
    vec3 ghosted_color = mix(final_color, prev_color, 0.5);
    float vignette_effect = mix(0.9, 1.0, vignette);
    vec3 vignetted_color = (ghosted_color * vignette_effect);
    return vec4((vignetted_color + vec3(reflection)), 1.0);
}

vec4 fs_crt(vec2 tex_coords_6) {
    vec3 color = vec3(0.0);
    vec2 centered_coords = ((tex_coords_6 - vec2(0.5)) * 2.0);
    float dist = length(centered_coords);
    float factor = ((dist * -0.01) * (1.0 - ((dist * dist) * 0.5)));
    vec2 distorted = (centered_coords * (1.0 + (factor * dist)));
    vec2 distorted_coords = ((distorted * 0.5) + vec2(0.5));
    if ((any(lessThan(distorted_coords.xy, vec2(0.0))) || any(greaterThan(distorted_coords.xy, vec2(1.0))))) {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }
    vec2 red_shift = vec2(0.003, 0.0);
    vec2 blue_shift = vec2(-0.003, 0.0);
    vec4 _e56 = get_sample((distorted_coords + red_shift));
    float r = _e56.x;
    vec4 _e58 = get_sample(distorted_coords);
    float g_1 = _e58.y;
    vec4 _e61 = get_sample((distorted_coords + blue_shift));
    float b_4 = _e61.z;
    float scanline = (0.5 + (0.5 * sin(((distorted_coords.y * 240.0) * 3.14159))));
    float scanlines = (1.0 - (0.15 * scanline));
    float vignette_1 = (1.0 - (0.2 * pow(dist, 2.0)));
    float time_seed = (float((uvec2(textureSize(_group_0_binding_0_fs, 0).xy).x % 100u)) * 0.01);
    float flicker = (1.0 - (0.03 * (0.5 + (0.5 * sin((time_seed * 10.0))))));
    vec4 _e98 = get_ghost_sample(distorted_coords);
    vec3 ghost = _e98.xyz;
    color = vec3(r, g_1, b_4);
    vec4 _e106 = get_sample((distorted_coords + vec2(0.001, 0.001)));
    vec3 blur1_ = _e106.xyz;
    vec4 _e112 = get_sample((distorted_coords - vec2(0.001, 0.001)));
    vec3 blur2_ = _e112.xyz;
    vec3 _e114 = color;
    color = mix(_e114, ((blur1_ + blur2_) * 0.5), 0.5);
    vec3 _e119 = color;
    color = mix(_e119, ghost, 0.06);
    vec3 _e121 = color;
    color = ((((_e121 * 1.2) - vec3(0.5)) * 1.1) + vec3(0.5));
    vec3 _e132 = color;
    color = (_e132 * ((scanlines * vignette_1) * flicker));
    float noise = fract((sin(dot(distorted_coords, vec2(12.9898, 78.233))) * 43758.547));
    vec3 _e146 = color;
    color = (_e146 + vec3(((noise * 0.015) - 0.0075)));
    vec3 _e149 = color;
    return vec4(_e149, 1.0);
}

void main() {
    VertexOutput in_1 = VertexOutput(gl_FragCoord, _vs2fs_location0);
    vec4 ret = vec4(0.0);
    uint _e3 = _group_1_binding_1_fs;
    switch(_e3) {
        default: {
            vec4 _e5 = get_sample(in_1.tex_coords);
            ret = _e5;
            break;
        }
        case 1u: {
            vec4 _e7 = fs_scale2x(in_1.tex_coords);
            ret = _e7;
            break;
        }
        case 2u: {
            vec4 _e9 = fs_scale3x(in_1.tex_coords);
            ret = _e9;
            break;
        }
        case 3u: {
            vec4 _e11 = fs_lcd(in_1.tex_coords);
            ret = _e11;
            break;
        }
        case 4u: {
            vec4 _e13 = fs_crt(in_1.tex_coords);
            ret = _e13;
            break;
        }
    }
    vec4 _e14 = ret;
    _fs2p_location0 = _e14;
    return;
}

