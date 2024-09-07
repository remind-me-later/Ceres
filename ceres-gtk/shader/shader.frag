#version 320 es

precision mediump float;

layout(location = 0) in vec2 TexCoord;
layout(location = 0) out vec4 FragColor;

layout(location = 2) uniform uint scale_mode;

uniform sampler2D img;

bool neq(vec3 a, vec3 b) {
    return any(notEqual(a, b));
}

bool eq(vec3 a, vec3 b) {
    return all(equal(a, b));
}

vec4 fs_scale2x(vec2 tex_coords) {
    vec2 dims = vec2(textureSize(img, 0));
    // offsets
    vec2 off = vec2(1.0, 1.0) / dims;

    //	  a         p0 p1
    //	c p b       p2 p3
    //	  d

    vec2 tc = tex_coords;

    vec3 p = texture(img, tc).xyz;
    vec3 a = texture(img, tc + vec2(0.0, -off.y)).xyz;
    vec3 c = texture(img, tc + vec2(-off.x, 0.0)).xyz;
    vec3 b = texture(img, tc + vec2(off.x, 0.0)).xyz;
    vec3 d = texture(img, tc + vec2(0.0, off.y)).xyz;

    vec3 p0 = eq(c, a) && neq(c, d) && neq(a, b) ? a : p;
    vec3 p1 = eq(a, b) && neq(a, c) && neq(b, d) ? b : p;
    vec3 p2 = eq(d, c) && neq(d, b) && neq(c, a) ? c : p;
    vec3 p3 = eq(b, d) && neq(b, a) && neq(d, c) ? d : p;

    // subpixel position
    vec2 pp = floor((2.0 * fract((tex_coords * dims))));

    vec3 ret = pp.y == 0.0 ? (pp.x == 0.0 ? p0 : p1) : (pp.x == 0.0 ? p2 : p3);

    return vec4(ret, 1.0);
}

vec4 fs_scale3x(vec2 tex_coords) {
    vec2 dims = vec2(textureSize(img, 0));
    // offsets
    vec2 off = vec2(1.0, 1.0) / dims;

    //	a b c	    p0 p1 p2
    //	d p f		  p3 p  p5
    //	g h i     p6 p7 p8

    vec2 tc = tex_coords;

    vec3 p = texture(img, tc).xyz;
    vec3 a = texture(img, tc + vec2(-off.x, -off.y)).xyz;
    vec3 b = texture(img, tc + vec2(0.0, -off.y)).xyz;
    vec3 c = texture(img, tc + vec2(off.x, -off.y)).xyz;
    vec3 d = texture(img, tc + vec2(-off.x, 0.0)).xyz;
    vec3 f = texture(img, tc + vec2(off.x, 0.0)).xyz;
    vec3 g = texture(img, tc + vec2(-off.x, off.y)).xyz;
    vec3 h = texture(img, tc + vec2(0.0, off.y)).xyz;
    vec3 i = texture(img, tc + vec2(off.x, off.y)).xyz;

    vec3 p0 = (eq(d, b) && neq(d, h) && neq(b, f)) ? d : p;
    vec3 p1 = ((eq(d, b) && neq(d, h) && neq(b, f) && neq(p, c)) ||
            (eq(b, f) && neq(b, d) && neq(f, h) && neq(p, a)))
        ? b : p;
    vec3 p2 = (eq(b, f) && neq(b, d) && neq(f, h)) ? f : p;
    vec3 p3 = ((eq(h, d) && neq(h, f) && neq(d, b) && neq(p, a)) ||
            (eq(d, b) && neq(d, h) && neq(b, f) && neq(p, g)))
        ? d : p;
    vec3 p5 = ((eq(b, f) && neq(b, d) && neq(f, h) && neq(p, i)) ||
            (eq(f, h) && neq(f, b) && neq(h, d) && neq(p, c)))
        ? f : p;
    vec3 p6 = (eq(h, d) && neq(h, f) && neq(d, b)) ? d : p;
    vec3 p7 = ((eq(f, h) && neq(f, b) && neq(h, d) && neq(p, g)) ||
            (eq(h, d) && neq(h, f) && neq(d, b) && neq(p, i)))
        ? h : p;
    vec3 p8 = (eq(f, h) && neq(f, b) && neq(h, d)) ? f : p;

    //	a b c    p0 p1 p2
    //	d p f    p3 p  p5
    //	g h i    p6 p7 p8

    // subpixel position
    vec2 pp = floor((3.0 * fract((tex_coords * dims))));
    vec3 ret = pp.y == 0.0
        ? (pp.x == 0.0 ? p0 : (pp.x == 1.0 ? p1 : p2)) : (pp.y == 1.0 ? (pp.x == 0.0 ? p3 : (pp.x == 1.0 ? p : p5)) : (pp.x == 0.0 ? p6 : (pp.x == 1.0 ? p7 : p8)));

    return vec4(ret, 1.0);
}

void main() {
    vec2 coor;
    coor.x = (TexCoord.x + 1.0) / 2.0;
    coor.y = (1.0 - TexCoord.y) / 2.0;

    if (scale_mode == 0u) {
        FragColor = texture(img, coor);
    } else if (scale_mode == 1u) {
        FragColor = fs_scale2x(coor);
    } else {
        FragColor = fs_scale3x(coor);
    }
}
