#version 450

out vec4 FragColor;
in vec2 TexCoord;

uniform sampler2D img;

void main() {
  vec2 coor;
  coor.x = (TexCoord.x + 1) / 2;
  coor.y = (1 - TexCoord.y) / 2;
  FragColor = texture(img, coor);
}
