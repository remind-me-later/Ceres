@group(0) @binding(0) var txt: texture_2d<f32>;
@group(0) @binding(1) var smpl: sampler;
@group(0) @binding(2) var prev_frame: texture_2d<f32>;

@group(1) @binding(0) var<uniform> dims: vec2<f32>;
@group(1) @binding(1) var<uniform> shader_opt: u32;

struct Vertexinput {
    @builtin(vertex_index) vert_idx: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>, @location(0) tex_coords: vec2<f32>,
}

@vertex fn vs_main(in: Vertexinput) -> VertexOutput {
    let vert_coord = select(select(select(vec2(1.0, 1.0), vec2(1.0, - 1.0), in.vert_idx == 2u), vec2(- 1.0, 1.0), in.vert_idx == 1u), vec2(- 1.0, - 1.0), in.vert_idx == 0u);

    var out: VertexOutput;
    out.clip_position = vec4(vert_coord * dims, 0.0, 1.0);
    // FIXME: ugly hack
    out.tex_coords = saturate(vec2(vert_coord.x, - vert_coord.y));
    return out;
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var ret: vec4<f32>;

    switch shader_opt {
        default : {
            // nearest neighbour
            ret = textureSample(txt, smpl, in.tex_coords);
        }
        case 1u : {
            // scale2x
            ret = fs_scale2x(in.tex_coords);
        }
        case 2u : {
            // scale3x
            ret = fs_scale3x(in.tex_coords);
        }
        case 3u : {
            // lcd
            ret = fs_lcd(in.tex_coords);
        }
    }

    return ret;
}

fn eq(a: vec3<f32>, b: vec3<f32>) -> bool {
    return all(a == b);
}

fn neq(a: vec3<f32>, b: vec3<f32>) -> bool {
    return any(a != b);
}

fn fs_scale2x(tex_coords: vec2<f32>) -> vec4<f32> {
    let dims = vec2<f32> (textureDimensions(txt));
    // offsets
    let off = vec2(1.0, 1.0) / dims;

    //	  a         p0 p1
    //	c p b       p2 p3
    //	  d

    let tc = tex_coords;

    let p = textureSample(txt, smpl, tc).xyz;
    let a = textureSample(txt, smpl, tc + vec2(0.0, - off.y)).xyz;
    let c = textureSample(txt, smpl, tc + vec2(- off.x, 0.0)).xyz;
    let b = textureSample(txt, smpl, tc + vec2(off.x, 0.0)).xyz;
    let d = textureSample(txt, smpl, tc + vec2(0.0, off.y)).xyz;

    let p0 = select(p, a, eq(c, a) && neq(c, d) && neq(a, b));
    let p1 = select(p, b, eq(a, b) && neq(a, c) && neq(b, d));
    let p2 = select(p, c, eq(d, c) && neq(d, b) && neq(c, a));
    let p3 = select(p, d, eq(b, d) && neq(b, a) && neq(d, c));

    // subpixel position
    let pp = floor(2.0 * fract(tc * dims));
    let ret = select(select(p3, p2, pp.x == 0.0), select(p1, p0, pp.x == 0.0), pp.y == 0.0);

    return vec4(ret, 1.0);
}

fn fs_scale3x(tex_coords: vec2<f32>) -> vec4<f32> {
    let dims = vec2<f32> (textureDimensions(txt));
    // offsets
    let off = vec2(1.0, 1.0) / dims;

    //	a b c	    p0 p1 p2
    //	d p f		p3 p  p5
    //	g h i       p6 p7 p8

    let tc = tex_coords;

    let p = textureSample(txt, smpl, tc).xyz;
    let a = textureSample(txt, smpl, tc + vec2(- off.x, - off.y)).xyz;
    let b = textureSample(txt, smpl, tc + vec2(0.0, - off.y)).xyz;
    let c = textureSample(txt, smpl, tc + vec2(off.x, - off.y)).xyz;
    let d = textureSample(txt, smpl, tc + vec2(- off.x, 0.0)).xyz;
    let f = textureSample(txt, smpl, tc + vec2(off.x, 0.0)).xyz;
    let g = textureSample(txt, smpl, tc + vec2(- off.x, off.y)).xyz;
    let h = textureSample(txt, smpl, tc + vec2(0.0, off.y)).xyz;
    let i = textureSample(txt, smpl, tc + vec2(off.x, off.y)).xyz;

    let p0 = select(p, d, eq(d, b) && neq(d, h) && neq(b, f));
    let p1 = select(p, b, (eq(d, b) && neq(d, h) && neq(b, f) && neq(p, c)) || (eq(b, f) && neq(b, d) && neq(f, h) && neq(p, a)));
    let p2 = select(p, f, eq(b, f) && neq(b, d) && neq(f, h));
    let p3 = select(p, d, (eq(h, d) && neq(h, f) && neq(d, b) && neq(p, a)) || (eq(d, b) && neq(d, h) && neq(b, f) && neq(p, g)));
    let p5 = select(p, f, (eq(b, f) && neq(b, d) && neq(f, h) && neq(p, i)) || (eq(f, h) && neq(f, b) && neq(h, d) && neq(p, c)));
    let p6 = select(p, d, eq(h, d) && neq(h, f) && neq(d, b));
    let p7 = select(p, h, (eq(f, h) && neq(f, b) && neq(h, d) && neq(p, g)) || (eq(h, d) && neq(h, f) && neq(d, b) && neq(p, i)));
    let p8 = select(p, f, eq(f, h) && neq(f, b) && neq(h, d));

    // subpixel position
    let pp = floor(3.0 * fract(tc * dims));
    let ret = select(select(select(select(p8, p7, pp.x == 1.0), p6, pp.x == 0.0), select(select(p5, p, pp.x == 1.0), p3, pp.x == 0.0), pp.y == 1.0), select(select(p2, p1, pp.x == 1.0), p0, pp.x == 0.0), pp.y == 0.0);

    return vec4(ret, 1.0);
}

fn fs_lcd(tex_coords: vec2<f32>) -> vec4<f32> {
    const LCD_TINT_RED: f32 = 0.93;
    const LCD_TINT_GREEN: f32 = 0.96;
    const LCD_TINT_BLUE: f32 = 0.9;

    const GRID_GAP_START: f32 = 0.5;
    const GRID_GAP_END: f32 = 1.0;
    const GRID_BASE_BRIGHTNESS: f32 = 0.7;
    const GRID_MAX_DARKENING: f32 = 0.1;
    const GRID_BLEND_FACTOR: f32 = 0.5;

    const VIGNETTE_CENTER_OFFSET: f32 = 0.5;
    const VIGNETTE_SCALE: f32 = 1.1;
    const REFLECTION_START: f32 = 0.0;
    const REFLECTION_END: f32 = 0.9;
    const REFLECTION_INTENSITY: f32 = 0.05;

    const GHOSTING_FACTOR: f32 = 0.5;

    let dims = vec2<f32> (textureDimensions(txt));
    let pixel_pos = tex_coords * dims;

    // Subpixel offsets
    // Subpixel pattern: R G B
    // We choose 0.165 so that every "band" of 3 pixels is 1/3 of a pixel
    const RED_SUBPIXEL_OFFSET: f32 = - 0.165;
    // const GREEN_SUBPIXEL_OFFSET: f32 = 0.5;
    const BLUE_SUBPIXEL_OFFSET: f32 = 0.165;

    let red_offset = vec2<f32> (RED_SUBPIXEL_OFFSET / dims.x, 0.0);
    let green_offset = vec2<f32> (0.0, 0.0);
    let blue_offset = vec2<f32> (BLUE_SUBPIXEL_OFFSET / dims.x, 0.0);

    // Sample the texture at subpixel positions
    let red_sample = textureSample(txt, smpl, tex_coords + red_offset).r;
    let green_sample = textureSample(txt, smpl, tex_coords + green_offset).g;
    let blue_sample = textureSample(txt, smpl, tex_coords + blue_offset).b;

    // Combine the subpixel samples
    let pixel = vec3<f32> (red_sample, green_sample, blue_sample);

    // LCD green tint
    let lcd_tint = vec3<f32> (LCD_TINT_RED, LCD_TINT_GREEN, LCD_TINT_BLUE);

    // Create LCD pixel grid effect
    let grid = vec2<f32> (smoothstep(GRID_GAP_START, GRID_GAP_END, fract(pixel_pos.x)), smoothstep(GRID_GAP_START, GRID_GAP_END, fract(pixel_pos.y)));
    let grid_effect = GRID_BASE_BRIGHTNESS + (GRID_MAX_DARKENING * (1.0 - (grid.x + grid.y) * GRID_BLEND_FACTOR));

    // Simulate ambient light reflection
    let vignette = 1.0 - length((tex_coords - VIGNETTE_CENTER_OFFSET) * VIGNETTE_SCALE);
    let reflection = smoothstep(REFLECTION_START, REFLECTION_END, vignette) * REFLECTION_INTENSITY;

    // Combine effects
    let final_color = pixel * lcd_tint * grid_effect;

    // Sample the previous frame
    let prev_color = textureSample(prev_frame, smpl, tex_coords).rgb;

    // Apply ghosting effect
    let ghosted_color = mix(final_color, prev_color, GHOSTING_FACTOR);

    return vec4(ghosted_color + reflection, 1.0);
}
