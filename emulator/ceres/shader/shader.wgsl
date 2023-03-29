// Globals

@group(0) @binding(0)
var txt: texture_2d<f32>;
@group(0)@binding(1)
var smpl: sampler;

@group(1) @binding(0)
var<uniform> dims: vec2<f32>;
@group(1)@binding(1)
var<uniform> scale_type: u32;

// Vertex shader

struct Vertexinput {
    @builtin(vertex_index) vert_idx: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: Vertexinput) -> VertexOutput {
    let vert_coord = select(
        select(
            select(
                vec2(1.0, 1.0),
                vec2(1.0, -1.0),
                in.vert_idx == 2u
            ),
            vec2(-1.0, 1.0),
            in.vert_idx == 1u
        ),
        vec2(-1.0, -1.0),
        in.vert_idx == 0u
    );

    var out: VertexOutput;
    out.clip_position = vec4(vert_coord * dims, 0.0, 1.0);
    out.tex_coords = saturate(vert_coord);
    return out;
}

// fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var ret: vec4<f32>;

    switch scale_type {
        default: {
            // nearest neighbour
            ret = textureSample(txt, smpl, in.tex_coords);
        }
        case 1u: {
            // scale2x
            ret = fs_scale2x(in.tex_coords);
        }
         case 2u: {
            // scale3x
            ret = fs_scale3x(in.tex_coords);
        }
    };

    return ret;
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

    let p = textureSample(txt, smpl, tc).xyz;
    let a = textureSample(txt, smpl, tc + vec2(0.0, -off.y)).xyz;
    let c = textureSample(txt, smpl, tc + vec2(-off.x, 0.0)).xyz;
    let b = textureSample(txt, smpl, tc + vec2(off.x, 0.0)).xyz;
    let d = textureSample(txt, smpl, tc + vec2(0.0, off.y)).xyz;

    let p0 = select(p, a, eq(c, a) && neq(c, d) && neq(a, b));
    let p1 = select(p, b, eq(a, b) && neq(a, c) && neq(b, d));
    let p2 = select(p, c, eq(d, c) && neq(d, b) && neq(c, a)) ;
    let p3 = select(p, d, eq(b, d) && neq(b, a) && neq(d, c));

    // subpixel position
    let pp = floor(2.0 * fract(tc * dims));
    let ret = select(
        select(p3, p2, pp.x == 0.0),
        select(p1, p0, pp.x == 0.0),
        pp.y == 0.0
    );

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

    let p = textureSample(txt, smpl, tc).xyz;
    let a = textureSample(txt, smpl, tc + vec2(-off.x, -off.y)).xyz;
    let b = textureSample(txt, smpl, tc + vec2(0.0, -off.y)).xyz;
    let c = textureSample(txt, smpl, tc + vec2(off.x, -off.y)).xyz;
    let d = textureSample(txt, smpl, tc + vec2(-off.x, 0.0)).xyz;
    let f = textureSample(txt, smpl, tc + vec2(off.x, 0.0)).xyz;
    let g = textureSample(txt, smpl, tc + vec2(-off.x, off.y)).xyz;
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
    let ret = select(
        select(
            select(select(p8, p7, pp.x == 1.0), p6, pp.x == 0.0),
            select(select(p5, p, pp.x == 1.0), p3, pp.x == 0.0),
            pp.y == 1.0
        ),
        select(select(p2, p1, pp.x == 1.0), p0, pp.x == 0.0),
        pp.y == 0.0
    );

    return vec4(ret, 1.0);
}
