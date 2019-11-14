#[macro_use]
extern crate glium;

use glium::Surface;
use {
    glutin::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    reclutch::display::{
        self, Color, DisplayListBuilder, Filter, GraphicsDisplay, GraphicsDisplayPaint, Point,
        Rect, Size,
    },
};

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

implement_vertex!(Vertex, position, normal);

const fn vertex(pos: [i8; 3], nor: [i8; 3]) -> Vertex {
    Vertex {
        position: [pos[0] as _, pos[1] as _, pos[2] as _],
        normal: [nor[0] as _, nor[1] as _, nor[2] as _],
    }
}

#[derive(Copy, Clone)]
struct TextureVertex {
    position: [f32; 3],
    tex_coord: [f32; 2],
}

implement_vertex!(TextureVertex, position, tex_coord);

const fn texture_vertex(pos: [i8; 2], tex: [i8; 2]) -> TextureVertex {
    TextureVertex {
        position: [pos[0] as _, pos[1] as _, 0.0],
        tex_coord: [tex[0] as _, tex[1] as _],
    }
}

const CUBE_VERTICES: [Vertex; 24] = [
    vertex([-1, -1, 1], [0, 0, 1]),
    vertex([1, -1, 1], [0, 0, 1]),
    vertex([1, 1, 1], [0, 0, 1]),
    vertex([-1, 1, 1], [0, 0, 1]),
    vertex([-1, 1, -1], [0, 0, -1]),
    vertex([1, 1, -1], [0, 0, -1]),
    vertex([1, -1, -1], [0, 0, -1]),
    vertex([-1, -1, -1], [0, 0, -1]),
    vertex([1, -1, -1], [1, 0, 0]),
    vertex([1, 1, -1], [1, 0, 0]),
    vertex([1, 1, 1], [1, 0, 0]),
    vertex([1, -1, 1], [1, 0, 0]),
    vertex([-1, -1, 1], [-1, 0, 0]),
    vertex([-1, 1, 1], [-1, 0, 0]),
    vertex([-1, 1, -1], [-1, 0, 0]),
    vertex([-1, -1, -1], [-1, 0, 0]),
    vertex([1, 1, -1], [0, 1, 0]),
    vertex([-1, 1, -1], [0, 1, 0]),
    vertex([-1, 1, 1], [0, 1, 0]),
    vertex([1, 1, 1], [0, 1, 0]),
    vertex([1, -1, 1], [0, -1, 0]),
    vertex([-1, -1, 1], [0, -1, 0]),
    vertex([-1, -1, -1], [0, -1, 0]),
    vertex([1, -1, -1], [0, -1, 0]),
];

const CUBE_INDICES: [u32; 36] = [
    0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16, 17, 18,
    18, 19, 16, 20, 21, 22, 22, 23, 20,
];

const QUAD_VERTICES: [TextureVertex; 4] = [
    texture_vertex([-1, -1], [0, 0]),
    texture_vertex([-1, 1], [0, 1]),
    texture_vertex([1, 1], [1, 1]),
    texture_vertex([1, -1], [1, 0]),
];

const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

fn main() {
    let window_size = (500u32, 500u32);

    let event_loop = EventLoop::new();

    let wb = glutin::window::WindowBuilder::new()
        .with_title("OpenGL 3D with Reclutch")
        .with_inner_size(
            glutin::dpi::PhysicalSize::new(window_size.0 as _, window_size.1 as _)
                .to_logical(event_loop.primary_monitor().hidpi_factor()),
        )
        .with_resizable(false);

    let cb = glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_srgb(true);

    let gl_display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let vertex_buffer = glium::VertexBuffer::new(&gl_display, &CUBE_VERTICES).unwrap();
    let indices = glium::IndexBuffer::new(
        &gl_display,
        glium::index::PrimitiveType::TrianglesList,
        &CUBE_INDICES,
    )
    .unwrap();

    let quad_vertex_buffer = glium::VertexBuffer::new(&gl_display, &QUAD_VERTICES).unwrap();
    let quad_indices = glium::IndexBuffer::new(
        &gl_display,
        glium::index::PrimitiveType::TrianglesList,
        &QUAD_INDICES,
    )
    .unwrap();

    let vertex_shader_src = r#"
        #version 150

        in vec3 position;
        in vec3 normal;

        out vec3 v_normal;

        uniform mat4 matrix;

        void main() {
            v_normal = transpose(inverse(mat3(matrix))) * normal;
            gl_Position = matrix * vec4(position, 1.0);
        }
    "#;

    let fragment_shader_src = r#"
        #version 150

        in vec3 v_normal;
        out vec4 frag_color;

        uniform vec3 light;

        void main() {
            float brightness = dot(normalize(v_normal), normalize(light));
            vec3 dark = vec3(0.32, 0.5, 0.5);
            vec3 regular = vec3(0.55, 0.9, 0.9);
            frag_color = vec4(mix(dark, regular, brightness), 1.0);
        }
    "#;

    let quad_vertex_shader_src = r#"
        #version 140

        in vec3 position;
        in vec2 tex_coord;

        out vec2 frag_tex_coord;

        void main() {
            frag_tex_coord = tex_coord;
            gl_Position = vec4(position, 1.0);
        }
    "#;

    let quad_fragment_shader_src = r#"
        #version 150

        in vec2 frag_tex_coord;
        out vec4 color;

        uniform sampler2D tex;

        void main() {
            color = texture(tex, frag_tex_coord);
        }
    "#;

    let program =
        glium::Program::from_source(&gl_display, vertex_shader_src, fragment_shader_src, None)
            .unwrap();

    let quad_program = glium::Program::from_source(
        &gl_display,
        quad_vertex_shader_src,
        quad_fragment_shader_src,
        None,
    )
    .unwrap();

    let out_texture = glium::texture::SrgbTexture2d::empty_with_format(
        &gl_display,
        glium::texture::SrgbFormat::U8U8U8U8,
        glium::texture::MipmapsOption::NoMipmap,
        window_size.0,
        window_size.1,
    )
    .unwrap();
    let out_texture_depth =
        glium::texture::DepthTexture2d::empty(&gl_display, window_size.0, window_size.1).unwrap();

    let mut skia_context = Some(unsafe {
        glutin::ContextBuilder::new()
            .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (3, 3)))
            .with_shared_lists(&gl_display.gl_window())
            .with_srgb(true)
            .build_headless(
                &event_loop,
                glutin::dpi::PhysicalSize::new(window_size.0 as _, window_size.1 as _),
            )
            .unwrap()
            .make_current()
            .unwrap()
    });

    use glium::GlObject as _;

    let mut display =
        display::skia::SkiaGraphicsDisplay::new_gl_texture(&display::skia::SkiaOpenGlTexture {
            size: (window_size.0 as _, window_size.1 as _),
            texture_id: out_texture.get_id(),
            mip_mapped: false,
        })
        .unwrap();

    {
        let rect = Rect::new(Point::new(150.0, 150.0), Size::new(100.0, 150.0));

        let mut builder = DisplayListBuilder::new();

        builder.push_round_rectangle_backdrop(rect, [20.0; 4], Filter::Blur(10.0, 10.0));

        builder.push_round_rectangle(
            rect,
            [20.0; 4],
            GraphicsDisplayPaint::Fill(Color::new(0.0, 0.0, 0.0, 0.2).into()),
        );

        display.push_command_group(&builder.build()).unwrap();
    }

    let mut latest_window_size = window_size;

    let mut roll = 0.0;
    let mut pitch = 0.0;
    let mut yaw = 0.0;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667),
        );

        match event {
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                roll += 0.001;
                pitch += 0.002;
                yaw += 0.003;

                let mut out_texture_fb = glium::framebuffer::SimpleFrameBuffer::with_depth_buffer(
                    &gl_display,
                    &out_texture,
                    &out_texture_depth,
                )
                .unwrap();

                let mut frame_target = gl_display.draw();
                let target = &mut out_texture_fb;

                let na_matrix =
                    nalgebra::Matrix4::from_euler_angles(roll, pitch, yaw).append_scaling(0.25);
                let matrix: &[[f32; 4]; 4] = na_matrix.as_ref();

                let params = glium::DrawParameters {
                    depth: glium::Depth {
                        test: glium::draw_parameters::DepthTest::IfLess,
                        write: true,
                        ..Default::default()
                    },
                    ..Default::default()
                };

                target.clear_color_and_depth((1.0, 1.0, 1.0, 1.0), 1.0);
                target
                    .draw(
                        &vertex_buffer,
                        &indices,
                        &program,
                        &uniform! { matrix: *matrix, light: [-1.0, 0.4, 0.9f32] },
                        &params,
                    )
                    .unwrap();

                skia_context =
                    Some(unsafe { skia_context.take().unwrap().make_current().unwrap() });

                if display.size().0 != latest_window_size.0 as _
                    || display.size().1 != latest_window_size.1 as _
                {
                    display
                        .resize((latest_window_size.0 as _, latest_window_size.1 as _))
                        .unwrap();
                }

                display.present(None).unwrap();

                frame_target
                    .draw(
                        &quad_vertex_buffer,
                        &quad_indices,
                        &quad_program,
                        &uniform! { tex: &out_texture },
                        &Default::default(),
                    )
                    .unwrap();
                frame_target.finish().unwrap();
            }
            Event::EventsCleared => {
                gl_display.gl_window().window().request_redraw();
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                let size = size.to_physical(gl_display.gl_window().window().hidpi_factor());
                latest_window_size = (size.width as _, size.height as _);
            }
            _ => return,
        }
    });
}
