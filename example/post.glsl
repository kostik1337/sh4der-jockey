#version 440

out vec4 color;

#pragma include "common.glsl"

uniform vec3 tint;

void main() {
    vec2 uv = gl_FragCoord.xy / resolution.xy;

    vec4 acc = vec4(0);
    const int iter = 40;

    for(int i = 0; i < iter; i++) {
        float x = float(i) / float(iter - 1);
        float s = 1.0 + 0.3 * (sliders[0]) * (x - 0.5);
        vec2 tuv = 0.5 + s * (uv - 0.5);
        acc += vec4(rainbow(x), 1) * texture2D(tex, tuv);
    }

    acc /= float(iter);

    acc.rgb *= tint;

    color = pow(acc, vec4(1) / 2.2);
}
