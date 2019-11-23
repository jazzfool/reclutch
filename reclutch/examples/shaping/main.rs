use {
    glium::glutin::{
        event::{Event as WinitEvent, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    reclutch::display::{
        self, Color, DisplayListBuilder, DisplayText, FontInfo, GraphicsDisplay as _, Point,
        ResourceData, ResourceDescriptor, ShapedGlyph, SharedData, TextDisplayItem, Vector,
    },
};

const FONT_SIZE: i32 = 64;

fn shape_with_harfbuzz(text: &str, size: i32) -> Vec<ShapedGlyph> {
    use harfbuzz_rs as hb;

    let face = hb::Face::from_bytes(include_bytes!("NotoSans.ttf"), 0);
    let mut font = hb::Font::new(face);

    font.set_scale(size, size);

    let buffer = hb::UnicodeBuffer::new().add_str(text);
    let output = hb::shape(&font, buffer, &[]);

    output
        .get_glyph_positions()
        .iter()
        .zip(output.get_glyph_infos())
        .map(|(position, info)| ShapedGlyph {
            codepoint: info.codepoint,
            offset: Vector::new(position.x_offset as _, position.y_offset as _),
            advance: Vector::new(position.x_advance as _, position.y_advance as _),
        })
        .collect()
}

fn shape_with_rusttype(text: &str, size: i32) -> Vec<ShapedGlyph> {
    use rusttype::Font;

    let font =
        Font::from_bytes(rusttype::SharedBytes::ByRef(include_bytes!("NotoSans.ttf"))).unwrap();

    font.layout(
        text,
        rusttype::Scale::uniform(size as f32 * 1.35),
        rusttype::Point { x: 0.0, y: 0.0 },
    )
    .map(|glyph| ShapedGlyph {
        codepoint: glyph.id().0,
        offset: Vector::new(0.0, glyph.position().y),
        advance: Vector::new(glyph.unpositioned().h_metrics().advance_width, 0.0),
    })
    .collect()
}

fn main() {
    let window_size = (500u32, 500u32);

    let event_loop = EventLoop::new();

    let wb = glutin::window::WindowBuilder::new()
        .with_title("Text shaping with Reclutch")
        .with_inner_size(
            glutin::dpi::PhysicalSize::new(window_size.0 as _, window_size.1 as _)
                .to_logical(event_loop.primary_monitor().hidpi_factor()),
        );

    let context = glutin::ContextBuilder::new()
        .with_vsync(true)
        .build_windowed(wb, &event_loop)
        .unwrap();

    let context = unsafe { context.make_current().unwrap() };

    let mut display = display::skia::SkiaGraphicsDisplay::new_gl_framebuffer(
        &display::skia::SkiaOpenGlFramebuffer {
            framebuffer_id: 0,
            size: (window_size.0 as _, window_size.1 as _),
        },
    )
    .unwrap();

    let mut latest_window_size = window_size;

    {
        let font_data = std::sync::Arc::new(include_bytes!("NotoSans.ttf").to_vec());
        let font_resource = display
            .new_resource(ResourceDescriptor::Font(ResourceData::Data(
                SharedData::RefCount(font_data.clone()),
            )))
            .unwrap();
        let font_info = FontInfo::from_data(font_data, 0).unwrap();

        let text_blobs = vec![
            TextDisplayItem {
                font: font_resource.clone(),
                font_info: font_info.clone(),
                size: 32.0,
                text: String::from("HarfBuzz").into(),
                color: Color::new(0.0, 0.0, 0.0, 1.0).into(),
                bottom_left: Point::new(40.0, 42.0),
            },
            TextDisplayItem {
                font: font_resource.clone(),
                font_info: font_info.clone(),
                size: FONT_SIZE as _,
                text: DisplayText::Shaped(shape_with_harfbuzz("एकोऽयम्", FONT_SIZE)),
                color: Color::new(0.0, 0.0, 0.0, 1.0).into(),
                bottom_left: Point::new(40.0, FONT_SIZE as f32 + 60.0),
            },
            TextDisplayItem {
                font: font_resource.clone(),
                font_info: font_info.clone(),
                size: 32.0,
                text: String::from("RustType").into(),
                color: Color::new(0.0, 0.0, 0.0, 1.0).into(),
                bottom_left: Point::new(40.0, 190.0),
            },
            TextDisplayItem {
                font: font_resource.clone(),
                font_info: font_info.clone(),
                size: FONT_SIZE as f32,
                text: DisplayText::Shaped(shape_with_rusttype("एकोऽयम्", FONT_SIZE)),
                color: Color::new(0.0, 0.0, 0.0, 1.0).into(),
                bottom_left: Point::new(40.0, FONT_SIZE as f32 + 210.0),
            },
        ];

        let mut builder = DisplayListBuilder::new();

        builder.push_clear(Color::new(1.0, 1.0, 1.0, 1.0));

        for text_blob in text_blobs.into_iter() {
            let bbox = text_blob.bounds().unwrap();

            builder.push_round_rectangle(
                bbox,
                [5.0; 4],
                display::GraphicsDisplayPaint::Fill(Color::new(0.0, 0.4, 1.0, 0.25).into()),
            );
            builder.push_text(text_blob);
        }

        display.push_command_group(&builder.build(), None).unwrap();
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            WinitEvent::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                if display.size().0 != latest_window_size.0 as _
                    || display.size().1 != latest_window_size.1 as _
                {
                    display
                        .resize((latest_window_size.0 as _, latest_window_size.1 as _))
                        .unwrap();
                }

                display.present(None).unwrap();
                context.swap_buffers().unwrap();
            }
            WinitEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            WinitEvent::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                let size = size.to_physical(context.window().hidpi_factor());
                latest_window_size = (size.width as _, size.height as _);
            }
            _ => return,
        }

        context.window().request_redraw();
    });
}
