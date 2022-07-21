#version 310 es

precision mediump float;

layout(location = 0) in vec2 TexCoord;
layout(location = 0) out vec4 FragColor;

uniform sampler2D img;

void main() {
  vec2 coor;
  coor.x = (TexCoord.x + 1.0) / 2.0;
  coor.y = (1.0 - TexCoord.y) / 2.0;
  FragColor = texture(img, coor);
}
