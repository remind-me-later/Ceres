// Vertex shader

struct Vertexinput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(model: Vertexinput) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);
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
    let off = vec2(1.0, 1.0) / dims;
    let t1 = in.tex_coords.xyxy + vec4<f32>(0.0, -off.y, -off.x, 0.0);  // a, c
	let t2 = in.tex_coords.xyxy + vec4<f32>(off.x, 0.0, 0.0, off.y);    // b, d	
    /*
        get texels:

		  a		    p0 p1
		c p b		p2 p3
		  d
	*/
	let a = textureSample(t_diffuse, s_diffuse, t1.xy).xyz;
    let c = textureSample(t_diffuse, s_diffuse, t1.zw).xyz;
    let p = textureSample(t_diffuse, s_diffuse, in.tex_coords).xyz;
    let b = textureSample(t_diffuse, s_diffuse, t2.xy).xyz;
    let d = textureSample(t_diffuse, s_diffuse, t2.zw).xyz;

	// rules
	var p0 = p; var p1 = p; var p2 = p; var p3 = p;

    if all(c == a) && any(c != d) && any(a != b) { p0 = a; }
    if all(a == b) && any(a != c) && any(b != d) { p1 = b; }
    if all(d == c) && any(d != b) && any(c != a) { p2 = c; }
    if all(b == d) && any(b != a) && any(d != c) { p3 = d; }

    // subpixel position
	let pp = floor(2.0 * fract(in.tex_coords * dims));
    var ret: vec3<f32>;
    if pp.y == 0.0 { 
        if pp.x == 0.0 { ret = p0; } else { ret = p1; }
    } else { 
        if pp.x == 0.0 { ret = p2; } else { ret = p3; }
    }

    return vec4(ret, 1.0);
}

@fragment
fn fs_scale3x(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(t_diffuse));
    let off = vec2(1.0, 1.0) / dims;
    let t1 = in.tex_coords.xyxy + vec4<f32>(0.0, -off.y, -off.x, 0.0);      // b, d
	let t2 = in.tex_coords.xyxy + vec4<f32>(off.x, 0.0, 0.0, off.y);        // f, h	
    let t3 = in.tex_coords.xyxy + vec4<f32>(off.x, off.y, -off.x, off.y);   // i, g
    let t4 = in.tex_coords.xyxy + vec4<f32>(-off.x, -off.y, off.x, -off.y); // a, c

    /*
        get texels:

		a b c	    p0 p1 p2
		d p f		p3 p4 p5
		g h i       p6 p7 p8
	*/
	let a = textureSample(t_diffuse, s_diffuse, t4.xy).xyz;
    let b = textureSample(t_diffuse, s_diffuse, t1.xy).xyz;
    let c = textureSample(t_diffuse, s_diffuse, t4.zw).xyz;
    let d = textureSample(t_diffuse, s_diffuse, t1.zw).xyz;
    let p = textureSample(t_diffuse, s_diffuse, in.tex_coords).xyz;
    let f = textureSample(t_diffuse, s_diffuse, t2.xy).xyz;
    let g = textureSample(t_diffuse, s_diffuse, t3.zw).xyz;
    let h = textureSample(t_diffuse, s_diffuse, t2.zw).xyz;
    let i = textureSample(t_diffuse, s_diffuse, t3.xy).xyz;

	// rules
	var p0 = p; var p1 = p; var p2 = p;
    var p3 = p; var p4 = p; var p5 = p; 
    var p6 = p; var p7 = p; var p8 = p;

    if all(d == b) && any(d != h) && any(b != f) { p0 = d; }
    if (all(d == b) && any(d != h) && any(b != f) && any(p != c)) 
        || (all(b == f) && any(b != d) && any(f != h) && any(p != a)) { p1=b; }
    if all(b == f) && any(b != d) && any(f != h) { p2=f; }
    if (all(h == d) && any(h != f) && any(d != b) && any(p != a)) 
        || (all(d == b) && any(d != h) && any(b != f) && any(p != g)) { p3=d; }
    if (all(b == f) && any(b != d) && any(f != h) && any(p != i)) 
        || (all(f == h) && any(f != b) && any(h != d) && any(p != c)) { p5=f; }
    if all(h == d) && any(h != f) && any(d != b) { p6=d; }
    if (all(f == h) && any(f != b) && any(h != d) && any(p != g)) 
        || (all(h == d) && any(h != f) && any(d != b) && any(p != i)) { p7=h; }
    if all(f == h) && any(f != b) && any(h != d) { p8=f; }

    // subpixel position
	let pp = floor(3.0 * fract(in.tex_coords * dims));
    var ret: vec3<f32>;
    if pp.y == 0.0 { 
        if pp.x == 0.0 { 
            ret = p0; 
        } else if pp.x == 1.0 { 
            ret = p1; 
        } else {
            ret = p2;
        }
    } else if pp.y == 1.0 {
        if pp.x == 0.0 { 
            ret = p3; 
        } else if pp.x == 1.0 { 
            ret = p4; 
        } else {
            ret = p5;
        }
    } else { 
        if pp.x == 0.0 { 
            ret = p6; 
        } else if pp.x == 1.0 { 
            ret = p7; 
        } else {
            ret = p8;
        }
    }

    return vec4(ret, 1.0);
}
