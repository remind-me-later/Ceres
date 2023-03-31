#version 310 es

layout(location = 0) out vec2 TexCoord;
uniform vec2 vp_dims;

void main() {
  vec2 v_coor = gl_VertexID == 0
                    ? vec2(-1.0, -1.0)
                    : (gl_VertexID == 1 ? vec2(-1.0, 1.0)
                                        : (gl_VertexID == 2 ? vec2(1.0, -1.0)
                                                            : vec2(1.0, 1.0)));

  gl_Position = vec4(v_coor.x * vp_dims.x, v_coor.y * vp_dims.y, 0.0, 1.0);
  TexCoord = v_coor;
}
