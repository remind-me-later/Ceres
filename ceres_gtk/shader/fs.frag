#version 310 es

precision mediump float;

layout(location = 0) in vec2 TexCoord;
layout(location = 0) out vec4 FragColor;

uniform uint scale_mode;

uniform sampler2D img;

bool neq(vec3 a, vec3 b) { return any(notEqual(a, b)); }

bool eq(vec3 a, vec3 b) { return all(equal(a, b)); }

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

void main() {
  vec2 coor;
  coor.x = (TexCoord.x + 1.0) / 2.0;
  coor.y = (1.0 - TexCoord.y) / 2.0;

  if (scale_mode == 0u) {
    FragColor = texture(img, coor);
  } else {
    FragColor = fs_scale2x(coor);
  }
}
