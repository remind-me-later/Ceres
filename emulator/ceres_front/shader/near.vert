#version 320 es

layout(location = 0) out vec2 TexCoord;
layout(location = 1) uniform vec2 transform;

const vec2 verts[4] =
    vec2[4](vec2(-1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(1.0, 1.0));

void main() {
  vec2 aPos = verts[gl_VertexID];
  gl_Position = vec4(aPos.x * transform.x, aPos.y * transform.y, 0.0, 1.0);
  TexCoord = aPos;
}
