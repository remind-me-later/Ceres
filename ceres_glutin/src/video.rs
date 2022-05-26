use {
    glium::{
        implement_vertex, texture::SrgbTexture2d, uniform, Display, IndexBuffer, Program, Surface,
        VertexBuffer,
    },
    std::time::Instant,
};

const INDICES: [u8; 6] = [
    0, 1, 3, // first triangle
    1, 2, 3, // second triangle
];

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

const TOP_RIGHT: Vertex = Vertex {
    position: [1.0, 1.0],
    tex_coords: [1.0, 0.0],
};
const BOTTOM_RIGHT: Vertex = Vertex {
    position: [1.0, -1.0],
    tex_coords: [1.0, 1.0],
};
const BOTTOM_LEFT: Vertex = Vertex {
    position: [-1.0, -1.0],
    tex_coords: [0.0, 1.0],
};
const TOP_LEFT: Vertex = Vertex {
    position: [-1.0, 1.0],
    tex_coords: [0.0, 0.0],
};

const SQUARE: [Vertex; 4] = [TOP_RIGHT, BOTTOM_RIGHT, BOTTOM_LEFT, TOP_LEFT];
const WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;

pub struct Renderer {
    texture: SrgbTexture2d,
    program: Program,
    indices: IndexBuffer<u8>,
    uniforms: [[f32; 4]; 4],
    display: Display,
    vertex_buffer: VertexBuffer<Vertex>,
    next_frame: Instant,
}

impl Renderer {
    pub fn new(
        display: Display,
        initial_window_width: u32,
        initial_window_height: u32,
    ) -> Renderer {
        let texture =
            glium::texture::SrgbTexture2d::empty(&display, WIDTH as u32, HEIGHT as u32).unwrap();

        implement_vertex!(Vertex, position, tex_coords);

        let indices =
            glium::IndexBuffer::new(&display, glium::index::PrimitiveType::TriangleFan, &INDICES)
                .unwrap();

        let vertex_shader_src = include_str!("shaders/vs.vert");
        let fragment_shader_src = include_str!("shaders/fs.frag");

        let program =
            glium::Program::from_source(&display, vertex_shader_src, fragment_shader_src, None)
                .unwrap();

        let uniforms = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0f32],
        ];

        let vertex_buffer = glium::VertexBuffer::new(&display, &SQUARE).unwrap();

        let mut video_renderer = Renderer {
            texture,
            program,
            indices,
            uniforms,
            display,
            vertex_buffer,
            next_frame: Instant::now(),
        };

        video_renderer.resize_viewport(initial_window_width, initial_window_height);

        video_renderer
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let multiplier = core::cmp::min(width / WIDTH, height / HEIGHT);
        let surface_width = WIDTH * multiplier;
        let surface_height = HEIGHT * multiplier;

        let x = surface_width as f32 / width as f32;
        let y = surface_height as f32 / height as f32;

        // hand-written scale matrix
        self.uniforms = [
            [x, 0.0, 0.0, 0.0],
            [0.0, -y, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
    }

    fn update_texture(&mut self, rgba_pixel_data: &[u8]) {
        let image = glium::texture::RawImage2d::from_raw_rgba_reversed(
            rgba_pixel_data,
            (WIDTH as u32, HEIGHT as u32),
        );
        self.texture = glium::texture::SrgbTexture2d::new(&self.display, image).unwrap();
    }

    fn draw(&mut self) {
        let mut target = self.display.draw();
        target.clear_color(0.0, 0.0, 0.0, 1.0);

        let uniforms = uniform! {
            matrix: self.uniforms,
            tex: self.texture.sampled().magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest),
        };

        target
            .draw(
                &self.vertex_buffer,
                &self.indices,
                &self.program,
                &uniforms,
                &Default::default(),
            )
            .unwrap();

        target.finish().unwrap();
    }
}

impl ceres_core::VideoCallbacks for Renderer {
    fn draw(&mut self, rgba_data: &[u8]) {
        self.update_texture(rgba_data);

        let now = Instant::now();

        if now < self.next_frame {
            std::thread::sleep(self.next_frame - now);
        }

        self.draw();
        self.next_frame += ceres_core::FRAME_DUR;
    }
}
