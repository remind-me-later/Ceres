#version 140

in vec2 aPos;
out vec2 TexCoord;

uniform vec2 transform;

void main() {
    gl_Position = vec4(aPos.x * transform.x, aPos.y * transform.y, 0.0, 1.0);
    TexCoord = aPos;
}
