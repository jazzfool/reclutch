use {
    pixels::{Pixels, PixelsBuilder, SurfaceTexture},
    raqote::{DrawOptions, DrawTarget},
    reclutch::display::{
        CommandGroupHandle, DisplayCommand, DisplayItem, GraphicsDisplay, GraphicsDisplayItem,
        GraphicsDisplayPaint, GraphicsDisplayStroke, Point, Rect,
    },
    std::collections::HashMap,
    winit::{
        event_loop::EventLoop,
        window::{Window, WindowBuilder},
    },
};

pub struct CpuGraphicsDisplay {
    pub(crate) window: Window,
    pub pixels: Pixels,
    pub draw_target: DrawTarget,
    cmds: HashMap<u64, Vec<DisplayCommand>>,
    next_command_group_handle: u64,
}

impl CpuGraphicsDisplay {
    pub fn new(size: (u32, u32), event_loop: &EventLoop<()>) -> Result<Self, failure::Error> {
        let window = WindowBuilder::new()
            .with_title("Counter with Reclutch")
            .with_inner_size(
                winit::dpi::PhysicalSize::new(size.0 as _, size.1 as _)
                    .to_logical(event_loop.primary_monitor().hidpi_factor()),
            )
            .with_resizable(false)
            .build(&event_loop)?;

        let surface = pixels::wgpu::Surface::create(&window);

        let surface_texture = SurfaceTexture::new(size.0, size.1, surface);
        let pixels = PixelsBuilder::new(size.0, size.1, surface_texture)
            .texture_format(pixels::wgpu::TextureFormat::Bgra8UnormSrgb)
            .request_adapter_options(pixels::wgpu::RequestAdapterOptions {
                power_preference: pixels::wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .build()?;

        Ok(CpuGraphicsDisplay {
            window,
            pixels,
            draw_target: DrawTarget::new(size.0 as i32, size.1 as i32),
            cmds: HashMap::new(),
            next_command_group_handle: 0,
        })
    }
}

impl GraphicsDisplay for CpuGraphicsDisplay {
    fn resize(&mut self, size: (u32, u32)) {
        self.pixels.resize(size.0, size.1);
        self.draw_target = DrawTarget::new(size.0 as i32, size.1 as i32);
    }

    fn push_command_group(
        &mut self,
        commands: &[DisplayCommand],
    ) -> Result<CommandGroupHandle, failure::Error> {
        let id = self.next_command_group_handle;
        self.next_command_group_handle += 1;

        self.cmds.insert(id, commands.to_owned());

        Ok(CommandGroupHandle::new(id))
    }

    fn get_command_group(&self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        self.cmds
            .get(&handle.id())
            .map(|cmds| cmds.iter().cloned().collect::<Vec<_>>())
    }

    fn modify_command_group(&mut self, handle: CommandGroupHandle, commands: &[DisplayCommand]) {
        if self.cmds.contains_key(&handle.id()) {
            self.cmds.insert(handle.id(), commands.to_owned());
        }
    }

    fn remove_command_group(&mut self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        self.cmds.remove(&handle.id())
    }

    fn before_exit(&mut self) {}

    fn present(&mut self, cull: Option<Rect>) {
        if let Some(rect) = cull {
            self.draw_target.push_clip_rect(convert_rect(&rect));
        }

        self.draw_target.clear(raqote::SolidSource {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        });

        for cmd_group in self.cmds.values() {
            for cmd in cmd_group {
                match cmd {
                    DisplayCommand::Item(item) => match item {
                        DisplayItem::Graphics(graphics) => {
                            let path = convert_item(&graphics);
                            let paint = match graphics {
                                GraphicsDisplayItem::Line { stroke, .. } => {
                                    GraphicsDisplayPaint::Stroke(stroke.clone())
                                }
                                GraphicsDisplayItem::Rectangle { paint, .. }
                                | GraphicsDisplayItem::RoundRectangle { paint, .. }
                                | GraphicsDisplayItem::Ellipse { paint, .. } => paint.clone(),
                            };

                            match paint {
                                GraphicsDisplayPaint::Fill(style_color) => {
                                    let color = style_color.color_or_black();
                                    self.draw_target.fill(
                                        &path,
                                        &raqote::Source::Solid(raqote::SolidSource {
                                            r: (color.red.min(1.0) * 255.0) as _,
                                            g: (color.green.min(1.0) * 255.0) as _,
                                            b: (color.blue.min(1.0) * 255.0) as _,
                                            a: 255,
                                        }),
                                        &DrawOptions {
                                            alpha: color.alpha,
                                            antialias: raqote::AntialiasMode::Gray,
                                            ..Default::default()
                                        },
                                    );
                                }
                                GraphicsDisplayPaint::Stroke(stroke) => {
                                    let color = stroke.color.color_or_black();
                                    self.draw_target.stroke(
                                        &path,
                                        &raqote::Source::Solid(raqote::SolidSource {
                                            r: (color.red.min(1.0) * 255.0) as _,
                                            g: (color.green.min(1.0) * 255.0) as _,
                                            b: (color.blue.min(1.0) * 255.0) as _,
                                            a: 255,
                                        }),
                                        &convert_stroke(&stroke),
                                        &DrawOptions {
                                            alpha: color.alpha,
                                            antialias: raqote::AntialiasMode::Gray,
                                            ..Default::default()
                                        },
                                    )
                                }
                            }
                        }
                        DisplayItem::Text(text) => {
                            let color = text.color.color_or_black();
                            self.draw_target.draw_text(
                                &text.font.font,
                                text.size,
                                &text.text,
                                convert_point(&text.bottom_left).to_f32(),
                                &raqote::Source::Solid(raqote::SolidSource {
                                    r: (color.red.min(1.0) * 255.0) as _,
                                    g: (color.green.min(1.0) * 255.0) as _,
                                    b: (color.blue.min(1.0) * 255.0) as _,
                                    a: 255,
                                }),
                                &DrawOptions {
                                    alpha: color.alpha,
                                    antialias: raqote::AntialiasMode::Gray,
                                    ..Default::default()
                                },
                            );
                        }
                    },
                    _ => (),
                }
            }
        }

        if self.pixels.get_frame().len() == self.draw_target.get_data_u8_mut().len() {
            self.pixels
                .get_frame()
                .copy_from_slice(self.draw_target.get_data_u8_mut());
            self.pixels.render();
        }
    }
}

fn convert_stroke(stroke: &GraphicsDisplayStroke) -> raqote::StrokeStyle {
    use reclutch::display::{LineCap, LineJoin};

    raqote::StrokeStyle {
        width: stroke.thickness,
        cap: match stroke.begin_cap {
            LineCap::Flat => raqote::LineCap::Butt,
            LineCap::Square => raqote::LineCap::Square,
            LineCap::Round => raqote::LineCap::Round,
        },
        join: match stroke.join {
            LineJoin::Miter => raqote::LineJoin::Miter,
            LineJoin::Round => raqote::LineJoin::Round,
            LineJoin::Bevel => raqote::LineJoin::Bevel,
        },
        miter_limit: stroke.miter_limit,
        ..Default::default()
    }
}

fn convert_rect(rect: &Rect) -> raqote::IntRect {
    raqote::IntRect::new(
        raqote::IntPoint::new(rect.origin.x as _, rect.origin.y as _),
        raqote::IntPoint::new(
            (rect.origin.x + rect.size.width) as _,
            (rect.origin.y + rect.size.height) as _,
        ),
    )
}

fn convert_point(point: &Point) -> raqote::IntPoint {
    raqote::IntPoint::new(point.x as _, point.y as _)
}

fn convert_item(item: &GraphicsDisplayItem) -> raqote::Path {
    match item {
        GraphicsDisplayItem::Line { a, b, .. } => {
            let mut pb = raqote::PathBuilder::new();
            pb.move_to(a.x, a.y);
            pb.line_to(b.x, b.y);
            pb.finish()
        }
        GraphicsDisplayItem::Rectangle { rect, .. } => {
            let mut pb = raqote::PathBuilder::new();
            pb.rect(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );
            pb.finish()
        }
        GraphicsDisplayItem::RoundRectangle { rect, radii, .. } => {
            let max_radii = {
                let min_edge = rect.size.width.min(rect.size.height);
                min_edge / 2.0
            };

            let radii = {
                let mut r = Vec::new();
                for rad in radii {
                    r.push(rad.min(max_radii));
                }
                r
            };

            let mut pb = raqote::PathBuilder::new();
            pb.move_to(rect.origin.x + radii[0], rect.origin.y);
            pb.line_to(rect.origin.x + rect.size.width - radii[1], rect.origin.y);
            pb.line_to(rect.origin.x + rect.size.width, rect.origin.y + radii[2]);
            pb.line_to(
                rect.origin.x + rect.size.width,
                rect.origin.y + rect.size.height - radii[2],
            );
            pb.line_to(
                rect.origin.x + rect.size.width - radii[2],
                rect.origin.y + rect.size.height,
            );
            pb.line_to(rect.origin.x + radii[3], rect.origin.y + rect.size.height);
            pb.line_to(rect.origin.x, rect.origin.y + rect.size.height - radii[3]);
            pb.line_to(rect.origin.x, rect.origin.y + radii[0]);
            pb.close();
            pb.arc(
                rect.origin.x + radii[0],
                rect.origin.y + radii[0],
                radii[0],
                270.0,
                360.0,
            );
            pb.move_to(
                rect.origin.x + rect.size.width - radii[1],
                rect.origin.y + radii[1],
            );
            pb.arc(
                rect.origin.x + rect.size.width - radii[1],
                rect.origin.y + radii[1],
                radii[1],
                0.0,
                90.0,
            );
            pb.move_to(
                rect.origin.x + rect.size.width - radii[2],
                rect.origin.y + rect.size.height - radii[2],
            );
            pb.arc(
                rect.origin.x + rect.size.width - radii[2],
                rect.origin.y + rect.size.height - radii[2],
                radii[2],
                90.0,
                180.0,
            );
            pb.move_to(
                rect.origin.x + radii[3],
                rect.origin.y + rect.size.height - radii[3],
            );
            pb.arc(
                rect.origin.x + radii[3],
                rect.origin.y + rect.size.height - radii[3],
                radii[3],
                180.0,
                270.0,
            );
            pb.finish()
        }
        GraphicsDisplayItem::Ellipse { center, radii, .. } => {
            let mut pb = raqote::PathBuilder::new();
            pb.arc(center.x, center.y, radii.x, 0.0, 360.0);
            pb.close();
            pb.finish()
        }
    }
}
