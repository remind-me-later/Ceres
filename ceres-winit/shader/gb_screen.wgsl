@group(0) @binding(0)
var txt: texture_2d<f32>;
@group(0) @binding(1)
var smpl: sampler;
@group(0) @binding(2)
var prev_frame: texture_2d<f32>;

@group(1) @binding(0)
var<uniform> dims: vec2<f32>;
@group(1) @binding(1)
var<uniform> shader_opt: u32;

struct Vertexinput {
    @builtin(vertex_index) vert_idx: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: Vertexinput) -> VertexOutput {
    let vert_coord = select(select(select(vec2(1.0, 1.0), vec2(1.0, - 1.0), in.vert_idx == 2u), vec2(- 1.0, 1.0), in.vert_idx == 1u), vec2(- 1.0, - 1.0), in.vert_idx == 0u);

    var out: VertexOutput;
    out.clip_position = vec4(vert_coord * dims, 0.0, 1.0);
    // FIXME: ugly hack
    out.tex_coords = saturate(vec2(vert_coord.x, - vert_coord.y));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var ret: vec4<f32>;

    switch shader_opt {
        default : {
            // nearest neighbour
            ret = get_sample(in.tex_coords);
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
        case 4u : {
            // crt
            ret = fs_crt(in.tex_coords);
        }
    }

    return ret;
}

fn get_sample(tex_coords: vec2<f32>) -> vec4<f32> {
    return desaturated(txt, tex_coords);
}

fn get_ghost_sample(tex_coords: vec2<f32>) -> vec4<f32> {
    return desaturated(prev_frame, tex_coords);
}

fn eq(a: vec3<f32>, b: vec3<f32>) -> bool {
    return all(a == b);
}

fn neq(a: vec3<f32>, b: vec3<f32>) -> bool {
    return any(a != b);
}

fn fs_scale2x(tex_coords: vec2<f32>) -> vec4<f32> {
    let dims = vec2<f32>(textureDimensions(txt));
    // offsets
    let off = vec2(1.0, 1.0) / dims;

    //	  a         p0 p1
    //	c p b       p2 p3
    //	  d

    let tc = tex_coords;

    let p = get_sample(tc).xyz;
    let a = get_sample(tc + vec2(0.0, - off.y)).xyz;
    let c = get_sample(tc + vec2(- off.x, 0.0)).xyz;
    let b = get_sample(tc + vec2(off.x, 0.0)).xyz;
    let d = get_sample(tc + vec2(0.0, off.y)).xyz;

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
    let dims = vec2<f32>(textureDimensions(txt));
    // offsets
    let off = vec2(1.0, 1.0) / dims;

    //	a b c	    p0 p1 p2
    //	d p f		p3 p  p5
    //	g h i       p6 p7 p8

    let tc = tex_coords;

    let p = get_sample(tc).xyz;
    let a = get_sample(tc + vec2(- off.x, - off.y)).xyz;
    let b = get_sample(tc + vec2(0.0, - off.y)).xyz;
    let c = get_sample(tc + vec2(off.x, - off.y)).xyz;
    let d = get_sample(tc + vec2(- off.x, 0.0)).xyz;
    let f = get_sample(tc + vec2(off.x, 0.0)).xyz;
    let g = get_sample(tc + vec2(- off.x, off.y)).xyz;
    let h = get_sample(tc + vec2(0.0, off.y)).xyz;
    let i = get_sample(tc + vec2(off.x, off.y)).xyz;

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
    // LCD parameters - can be adjusted for different effects
    const LCD_TINT_RED: f32 = 0.93;
    const LCD_TINT_GREEN: f32 = 0.96;
    const LCD_TINT_BLUE: f32 = 0.9;

    const GRID_GAP_START: f32 = 0.4;
    const GRID_GAP_END: f32 = 0.6;
    const GRID_BASE_BRIGHTNESS: f32 = 0.5;
    const GRID_MAX_DARKENING: f32 = 0.25;
    const GRID_BLEND_FACTOR: f32 = 0.5;

    const VIGNETTE_CENTER_OFFSET: f32 = 0.5;
    const VIGNETTE_SCALE: f32 = 1.0;
    // Add vignette darkness factor - higher value means darker edges
    const VIGNETTE_DARKNESS: f32 = 0.9;
    const REFLECTION_START: f32 = 0.0;
    const REFLECTION_END: f32 = 0.9;
    const REFLECTION_INTENSITY: f32 = 0.1;

    const GHOSTING_FACTOR: f32 = 0.5;

    let dims = vec2<f32>(textureDimensions(txt));
    let pixel_pos = tex_coords * dims;

    // Subpixel offsets
    // Subpixel pattern: R G B
    const RED_SUBPIXEL_OFFSET: f32 = - 0.3;
    const BLUE_SUBPIXEL_OFFSET: f32 = 0.3;

    let red_offset = vec2<f32>(RED_SUBPIXEL_OFFSET / dims.x, 0.0);
    let green_offset = vec2<f32>(0.0, 0.0);
    let blue_offset = vec2<f32>(BLUE_SUBPIXEL_OFFSET / dims.x, 0.0);

    // Sample the texture at subpixel positions
    let red_sample = get_sample(tex_coords + red_offset).r;
    let green_sample = get_sample(tex_coords + green_offset).g;
    let blue_sample = get_sample(tex_coords + blue_offset).b;

    // Combine the subpixel samples
    let pixel = vec3<f32>(red_sample, green_sample, blue_sample);

    // LCD green tint
    let lcd_tint = vec3<f32>(LCD_TINT_RED, LCD_TINT_GREEN, LCD_TINT_BLUE);

    // Create LCD pixel grid effect
    let grid = vec2<f32>(smoothstep(GRID_GAP_START, GRID_GAP_END, fract(pixel_pos.x)), smoothstep(GRID_GAP_START, GRID_GAP_END, fract(pixel_pos.y)));
    let grid_effect = GRID_BASE_BRIGHTNESS + (GRID_MAX_DARKENING * (1.0 - (grid.x + grid.y) * GRID_BLEND_FACTOR));

    // Simulate ambient light reflection
    let vignette = 1.0 - length((tex_coords - VIGNETTE_CENTER_OFFSET) * VIGNETTE_SCALE);
    let reflection = smoothstep(REFLECTION_START, REFLECTION_END, vignette) * REFLECTION_INTENSITY;

    // Combine effects
    let final_color = pixel * lcd_tint * grid_effect;

    // Sample the previous frame
    let prev_color = get_ghost_sample(tex_coords).rgb;

    // Apply ghosting effect
    let ghosted_color = mix(final_color, prev_color, GHOSTING_FACTOR);

    // Apply vignette darkening effect
    let vignette_effect = mix(VIGNETTE_DARKNESS, 1.0, vignette);
    let vignetted_color = ghosted_color * vignette_effect;

    return vec4(vignetted_color + reflection, 1.0);
}

fn fs_crt(tex_coords: vec2<f32>) -> vec4<f32> {
    // CRT parameters - can be adjusted for different effects
    const CURVATURE: f32 = - 0.01;
    const SCANLINE_INTENSITY: f32 = 0.15;
    const SCANLINE_COUNT: f32 = 240.0;
    const VIGNETTE_STRENGTH: f32 = 0.2;
    const COLOR_BLEED: f32 = 0.5;
    const BRIGHTNESS: f32 = 1.2;
    const CONTRAST: f32 = 1.1;
    const FLICKER_INTENSITY: f32 = 0.03;
    const GHOST_INTENSITY: f32 = 0.06;
    const RGB_SHIFT: f32 = 0.003;

    // Center coordinates for distortion
    let centered_coords = vec2(tex_coords - 0.5) * 2.0;

    // Apply barrel distortion (CRT curvature)
    let distort_strength = 1.0 - CURVATURE;
    let dist = length(centered_coords);
    let factor = dist * CURVATURE * (1.0 - dist * dist * 0.5);

    // Calculate distorted texture coordinates
    let distorted = centered_coords * (1.0 + factor * dist);
    let distorted_coords = (distorted * 0.5) + 0.5;

    // Check if we're out of bounds after distortion
    if (any(distorted_coords.xy < vec2(0.0)) || any(distorted_coords.xy > vec2(1.0))) {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }

    // Apply RGB color shift (chromatic aberration)
    let red_shift = vec2(RGB_SHIFT, 0.0);
    let blue_shift = vec2(- RGB_SHIFT, 0.0);

    let r = get_sample(distorted_coords + red_shift).r;
    let g = get_sample(distorted_coords).g;
    let b = get_sample(distorted_coords + blue_shift).b;

    // Create scanlines
    let scanline = 0.5 + 0.5 * sin(distorted_coords.y * SCANLINE_COUNT * 3.14159);
    let scanlines = 1.0 - SCANLINE_INTENSITY * scanline;

    // Create vignette darkening at edges
    let vignette = 1.0 - VIGNETTE_STRENGTH * pow(dist, 2.0);

    // Generate pseudo-random flicker
    let time_seed = f32(textureDimensions(txt).x % 100u) * 0.01;
    let flicker = 1.0 - FLICKER_INTENSITY * (0.5 + 0.5 * sin(time_seed * 10.0));

    // Get ghosting from previous frame
    let ghost = get_ghost_sample(distorted_coords).rgb;

    // Combine color with effects
    var color = vec3(r, g, b);

    // Apply color bleed (soft glow)
    let blur1 = get_sample(distorted_coords + vec2(0.001, 0.001)).rgb;
    let blur2 = get_sample(distorted_coords - vec2(0.001, 0.001)).rgb;
    color = mix(color, (blur1 + blur2) * 0.5, COLOR_BLEED);

    // Apply ghosting
    color = mix(color, ghost, GHOST_INTENSITY);

    // Apply brightness, contrast, scanlines, vignette and flicker
    color = (color * BRIGHTNESS - 0.5) * CONTRAST + 0.5;
    color *= scanlines * vignette * flicker;

    // Add subtle noise for grain effect
    let noise = fract(sin(dot(distorted_coords, vec2(12.9898, 78.233))) * 43758.5453);
    color += (noise * 0.015 - 0.0075);

    return vec4(color, 1.0);
}

fn desaturated(txt: texture_2d<f32>, tex_coords: vec2<f32>) -> vec4<f32> {
    let tex = textureSample(txt, smpl, tex_coords);
    const SATURATION: f32 = 0.7;

    // Calculate luminance (grayscale) using standard coefficients
    let luminance = tex.r * 0.299 + tex.g * 0.587 + tex.b * 0.114;

    // Mix between grayscale (0.0 saturation) and full color (1.0 saturation)
    let desaturated_color = vec3(mix(luminance, tex.r, SATURATION), mix(luminance, tex.g, SATURATION), mix(luminance, tex.b, SATURATION));

    return vec4(desaturated_color, tex.a);
}