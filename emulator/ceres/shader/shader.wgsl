// Vertex shader
@group(1) @binding(0)
var<uniform> unif: vec2<f32>;

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
    out.clip_position = vec4(vert_coord * unif, 0.0, 1.0);
    out.tex_coords = saturate(vert_coord);
    return out;
}

// fragment shader
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0)@binding(1)
var s_diffuse: sampler;

@fragment
fn fs_near(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}

@fragment
fn fs_scale2x(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(t_diffuse));
    // offsets
    let off = vec2(1.0, 1.0) / dims;
    /*
		  a		    p0 p1
		c p b		p2 p3
		  d
	*/

    let tc = in.tex_coords;

    let p = textureSample(t_diffuse, s_diffuse, tc).xyz;
	let a = textureSample(t_diffuse, s_diffuse, tc + vec2(0.0, -off.y)).xyz;
    let c = textureSample(t_diffuse, s_diffuse, tc + vec2(-off.x, 0.0)).xyz;
    let b = textureSample(t_diffuse, s_diffuse, tc + vec2(off.x, 0.0)).xyz;
    let d = textureSample(t_diffuse, s_diffuse, tc + vec2(0.0, off.y)).xyz;
    
    // subpixel position
    let pp = floor(2.0 * fract(in.tex_coords * dims));
    let ret = select(
        select(
                select(p, d, all(b == d) && any(b != a) && any(d != c)),
                select(p, c, all(d == c) && any(d != b) && any(c != a)),
                pp.x == 0.0),
        select(
                select(p, b, all(a == b) && any(a != c) && any(b != d)),
                select(p, a, all(c == a) && any(c != d) && any(a != b)),
                pp.x == 0.0),
        pp.y == 0.0);
    return vec4(ret, 1.0);
}

@fragment
fn fs_scale3x(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(t_diffuse));
    // offsets
    let off = vec2(1.0, 1.0) / dims;
    /*
        get texels:

		a b c	    p0 p1 p2
		d p f		p3 p4 p5
		g h i       p6 p7 p8
	*/
    let tc = in.tex_coords;

    let p = textureSample(t_diffuse, s_diffuse, tc).xyz;
	let a = textureSample(t_diffuse, s_diffuse, tc + vec2(-off.x, -off.y)).xyz;
    let b = textureSample(t_diffuse, s_diffuse, tc + vec2(0.0, -off.y)).xyz;
    let c = textureSample(t_diffuse, s_diffuse, tc + vec2(off.x, -off.y)).xyz;
    let d = textureSample(t_diffuse, s_diffuse, tc + vec2(-off.x, 0.0)).xyz;
    let f = textureSample(t_diffuse, s_diffuse, tc + vec2(off.x, 0.0)).xyz;
    let g = textureSample(t_diffuse, s_diffuse, tc + vec2(-off.x, off.y)).xyz;
    let h = textureSample(t_diffuse, s_diffuse, tc + vec2( 0.0, off.y)).xyz;
    let i = textureSample(t_diffuse, s_diffuse, tc + vec2(off.x, off.y)).xyz;

    // subpixel position
    let pp = floor(3.0 * fract(in.tex_coords * dims));
    let ret = select(
        select(
            select(
                select(
                    select(p, f, all(f == h) && any(f != b) && any(h != d)), 
                    select(p, h, (all(f == h) && any(f != b) && any(h != d) && any(p != g)) || (all(h == d) && any(h != f) && any(d != b) && any(p != i))), 
                    pp.x == 1.0), 
                select(p, d, all(h == d) && any(h != f) && any(d != b)), 
                pp.x == 0.0),  
            select(
                select(
                    select(p, f, (all(b == f) && any(b != d) && any(f != h) && any(p != i)) || (all(f == h) && any(f != b) && any(h != d) && any(p != c))),
                    p,
                    pp.x == 1.0),  
                select(p, d, (all(h == d) && any(h != f) && any(d != b) && any(p != a)) || (all(d == b) && any(d != h) && any(b != f) && any(p != g))), 
                pp.x == 0.0), 
            pp.y == 1.0),  
        select(
            select(
                select(p, f, all(b == f) && any(b != d) && any(f != h)), 
                select(p, b, (all(d == b) && any(d != h) && any(b != f) && any(p != c)) || (all(b == f) && any(b != d) && any(f != h) && any(p != a))), 
                pp.x == 1.0),
            select(p, d, all(d == b) && any(d != h) && any(b != f)), 
            pp.x == 0.0), 
        pp.y == 0.0);
    
    return vec4(ret, 1.0);
}
