//! Robust implementation of `reclutch::display::GraphicsDisplay` using Google's Skia.

use super::*;
use crate::error;
use skia_safe as sk;
use std::collections::HashMap;
use std::io::Read;

/// Contains information about an existing OpenGL framebuffer.
#[derive(Debug, Clone, Copy)]
pub struct SkiaOpenGlFramebuffer {
    pub size: (i32, i32),
    pub framebuffer_id: u32,
}

/// Contains information about an existing OpenGL texture.
#[derive(Debug, Clone, Copy)]
pub struct SkiaOpenGlTexture {
    pub size: (i32, i32),
    pub mip_mapped: bool,
    pub texture_id: u32,
    pub target: u32,
}

enum SurfaceType {
    OpenGlFramebuffer(SkiaOpenGlFramebuffer),
    OpenGlTexture(SkiaOpenGlTexture),
}

enum Resource {
    Image(sk::Image),
}

/// Converts [`DisplayCommand`](DisplayCommand) to immediate-mode Skia commands.
pub struct SkiaGraphicsDisplay {
    surface: sk::Surface,
    surface_type: SurfaceType,
    command_groups: indexmap::IndexMap<u64, (Vec<DisplayCommand>, Rect)>,
    next_command_group_id: u64,
    resources: HashMap<u64, Resource>,
    next_resource_id: u64,
}

impl SkiaGraphicsDisplay {
    /// Creates a new [`SkiaGraphicsDisplay`](SkiaGraphicsDisplay) with the Skia OpenGL backend, drawing into an existing framebuffer.
    /// This assumes that an OpenGL context has already been set up.
    /// This also assumes that the color format is RGBA with 8-bit components.
    pub fn new_gl_framebuffer(target: &SkiaOpenGlFramebuffer) -> Result<Self, error::SkiaError> {
        Ok(Self {
            surface: Self::new_gl_framebuffer_surface(target)?,
            surface_type: SurfaceType::OpenGlFramebuffer(*target),
            command_groups: indexmap::IndexMap::new(),
            next_command_group_id: 0,
            resources: HashMap::new(),
            next_resource_id: 0,
        })
    }

    /// Creates a new [`SkiaGraphicsDisplay`](SkiaGraphicsDisplay) with the Skia OpenGL backend, drawing into an existing texture.
    /// This assumes that an OpenGL context has already been set up.
    /// This also assumes that the color format is RGBA with 8-bit components
    pub fn new_gl_texture(target: &SkiaOpenGlTexture) -> Result<Self, error::SkiaError> {
        Ok(Self {
            surface: Self::new_gl_texture_surface(target)?,
            surface_type: SurfaceType::OpenGlTexture(*target),
            command_groups: indexmap::IndexMap::new(),
            next_command_group_id: 0,
            resources: HashMap::new(),
            next_resource_id: 0,
        })
    }

    /// Returns the size of the underlying surface.
    pub fn size(&self) -> (i32, i32) {
        match self.surface_type {
            SurfaceType::OpenGlFramebuffer(SkiaOpenGlFramebuffer { size, .. })
            | SurfaceType::OpenGlTexture(SkiaOpenGlTexture { size, .. }) => size,
        }
    }

    fn new_gl_framebuffer_surface(
        target: &SkiaOpenGlFramebuffer,
    ) -> Result<sk::Surface, error::SkiaError> {
        let mut context = Self::new_gl_context()?;
        let surface = {
            let info = sk::gpu::BackendRenderTarget::new_gl(
                target.size,
                None,
                8,
                sk::gpu::gl::FramebufferInfo {
                    fboid: target.framebuffer_id,
                    format: gl::RGBA8,
                },
            );

            sk::Surface::from_backend_render_target(
                &mut context,
                &info,
                sk::gpu::SurfaceOrigin::BottomLeft,
                sk::ColorType::RGBA8888,
                sk::ColorSpace::new_srgb(),
                None,
            )
            .ok_or(error::SkiaError::InvalidTarget(String::from("framebuffer")))?
        };

        Ok(surface)
    }

    fn new_gl_texture_surface(target: &SkiaOpenGlTexture) -> Result<sk::Surface, error::SkiaError> {
        let mut context = Self::new_gl_context()?;
        let surface = {
            let info = unsafe {
                sk::gpu::BackendTexture::new_gl(
                    target.size,
                    if target.mip_mapped {
                        sk::gpu::MipMapped::Yes
                    } else {
                        sk::gpu::MipMapped::No
                    },
                    sk::gpu::gl::TextureInfo::from_target_and_id(target.target, target.texture_id),
                )
            };

            sk::Surface::from_backend_texture(
                &mut context,
                &info,
                sk::gpu::SurfaceOrigin::BottomLeft,
                None,
                sk::ColorType::RGBA8888,
                sk::ColorSpace::new_srgb(),
                None,
            )
            .ok_or(error::SkiaError::InvalidTarget(String::from("texture")))?
        };

        Ok(surface)
    }

    fn new_gl_context() -> Result<sk::gpu::Context, error::SkiaError> {
        let interface = sk::gpu::gl::Interface::new_native();
        sk::gpu::Context::new_gl(interface).ok_or(error::SkiaError::InvalidContext)
    }
}

impl GraphicsDisplay for SkiaGraphicsDisplay {
    fn resize(&mut self, size: (u32, u32)) -> Result<(), Box<dyn std::error::Error>> {
        match self.surface_type {
            SurfaceType::OpenGlFramebuffer(ref mut target) => {
                target.size = (size.0 as i32, size.1 as i32);
                self.surface = Self::new_gl_framebuffer_surface(target)?;
            }
            SurfaceType::OpenGlTexture(ref mut target) => {
                target.size = (size.0 as i32, size.1 as i32);
                self.surface = Self::new_gl_texture_surface(target)?;
            }
        };

        Ok(())
    }

    fn new_resource(
        &mut self,
        descriptor: ResourceDescriptor,
    ) -> Result<ResourceReference, error::ResourceError> {
        let res = match descriptor {
            ResourceDescriptor::ImageFile(path) => {
                if path.is_file() {
                    let mut data = Vec::new();
                    std::fs::File::open(path)?.read_to_end(&mut data)?;
                    Resource::Image(
                        sk::Image::from_encoded(sk::Data::new_copy(&data), None)
                            .ok_or(error::ResourceError::InvalidData)?,
                    )
                } else {
                    return Err(error::ResourceError::InvalidPath(
                        path.to_string_lossy().to_string(),
                    ));
                }
            }
            ResourceDescriptor::ImageData(data) => Resource::Image(
                sk::Image::from_encoded(sk::Data::new_copy(&data), None)
                    .ok_or(error::ResourceError::InvalidData)?,
            ),
        };

        let id = self.next_resource_id;
        self.next_resource_id += 1;

        self.resources.insert(id, res);

        Ok(ResourceReference::Image(id))
    }

    fn remove_resource(&mut self, reference: ResourceReference) {
        if self.resources.contains_key(&reference.id()) {
            self.resources.remove(&reference.id());
        }
    }

    fn push_command_group(
        &mut self,
        commands: &[DisplayCommand],
    ) -> Result<CommandGroupHandle, Box<dyn std::error::Error>> {
        let id = self.next_command_group_id;

        self.command_groups
            .insert(id, (commands.to_owned(), display_list_bounds(commands)?));

        self.next_command_group_id += 1;

        Ok(CommandGroupHandle::new(id))
    }

    #[inline]
    fn get_command_group(&self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        Some(self.command_groups.get(&handle.id())?.0.clone())
    }

    fn modify_command_group(&mut self, handle: CommandGroupHandle, commands: &[DisplayCommand]) {
        if self.command_groups.contains_key(&handle.id()) {
            if let Ok(bounds) = display_list_bounds(commands) {
                self.command_groups
                    .insert(handle.id(), (commands.to_owned(), bounds));
            }
        }
    }

    #[inline]
    fn remove_command_group(&mut self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        Some(self.command_groups.remove(&handle.id())?.0)
    }

    #[inline]
    fn before_exit(&mut self) {
        self.surface.flush()
    }

    fn present(&mut self, cull: Option<Rect>) {
        // for loop written twice so that we only have to see if cull exists once
        if let Some(cull) = cull {
            let cmds = self.command_groups.values().cloned().collect::<Vec<_>>();
            for (ref cmd_group, ref bounds) in cmds {
                if cull.intersects(bounds) {
                    let count = self.surface.canvas().save();
                    draw_command_group(cmd_group, self);
                    // to ensure that no clips, transformations, layers, etc, "leak" from the command group.
                    self.surface.canvas().restore_to_count(count);
                }
            }
        } else {
            let cmds = self.command_groups.values().cloned().collect::<Vec<_>>();
            for (ref cmd_group, _) in cmds {
                let count = self.surface.canvas().save();
                draw_command_group(cmd_group, self);
                self.surface.canvas().restore_to_count(count);
            }
        }

        self.surface.flush();
    }
}

fn convert_color(color: &Color) -> sk::Color4f {
    sk::Color4f::new(color.red, color.green, color.blue, color.alpha)
}

fn convert_point(point: &Point) -> sk::Point {
    sk::Point::new(point.x, point.y)
}

// TODO(jazzfool): return errors (i.e. return Error<..> in `GraphicsDisplay::present`)
fn apply_color(color: &StyleColor, paint: &mut sk::Paint) {
    match color {
        StyleColor::Color(ref color) => {
            // we can afford to "make" the SRGB color space every time; it's actually a singleton in the C++ Skia code.
            paint.set_color4f(convert_color(color), &sk::ColorSpace::new_srgb());
        }
        StyleColor::LinearGradient(ref gradient) => {
            let (colors, stops): (Vec<_>, Vec<_>) = gradient
                .stops
                .iter()
                .map(|stop| (convert_color(&stop.1).to_color(), stop.0 as sk::scalar))
                .unzip();

            paint.set_shader(
                sk::gradient_shader::linear(
                    (convert_point(&gradient.start), convert_point(&gradient.end)),
                    sk::gradient_shader::GradientShaderColors::Colors(&colors[..]),
                    &stops[..],
                    sk::TileMode::default(),
                    None,
                    None,
                )
                .unwrap(),
            );
        }
        StyleColor::RadialGradient(ref gradient) => {
            let (colors, stops): (Vec<_>, Vec<_>) = gradient
                .stops
                .iter()
                .map(|stop| (convert_color(&stop.1).to_color(), stop.0 as sk::scalar))
                .unzip();

            paint.set_shader(sk::gradient_shader::radial(
                convert_point(&gradient.start),
                (gradient.end - gradient.start).length(),
                sk::gradient_shader::GradientShaderColors::Colors(&colors[..]),
                &stops[..],
                sk::TileMode::default(),
                None,
                None,
            ));
        }
    };
}

fn convert_line_cap(cap: &LineCap) -> sk::PaintCap {
    match cap {
        LineCap::Flat => sk::PaintCap::Butt,
        LineCap::Square => sk::PaintCap::Square,
        LineCap::Round => sk::PaintCap::Round,
    }
}

fn convert_line_join(join: &LineJoin) -> sk::PaintJoin {
    match join {
        LineJoin::Miter => sk::PaintJoin::Miter,
        LineJoin::Round => sk::PaintJoin::Round,
        LineJoin::Bevel => sk::PaintJoin::Bevel,
    }
}

fn convert_paint(paint: &GraphicsDisplayPaint) -> sk::Paint {
    match paint {
        GraphicsDisplayPaint::Fill(ref color) => {
            let mut paint = sk::Paint::default();
            paint.set_anti_alias(true);

            apply_color(color, &mut paint);

            paint
        }
        GraphicsDisplayPaint::Stroke(ref stroke) => {
            let mut paint = sk::Paint::default();
            paint.set_anti_alias(stroke.antialias);
            paint.set_style(sk::PaintStyle::Stroke);

            apply_color(&stroke.color, &mut paint);

            paint.set_stroke_width(stroke.thickness);
            paint.set_stroke_cap(convert_line_cap(&stroke.begin_cap));
            paint.set_stroke_join(convert_line_join(&stroke.join));
            paint.set_stroke_miter(stroke.miter_limit);

            paint
        }
    }
}

fn convert_rect(rect: &Rect) -> sk::Rect {
    sk::Rect::from_xywh(
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    )
}

fn apply_clip(canvas: &mut sk::Canvas, clip: &DisplayClip) {
    match clip {
        DisplayClip::Rectangle {
            ref rect,
            antialias,
        } => {
            canvas.clip_rect(convert_rect(rect), None, *antialias);
        }
        DisplayClip::RoundRectangle { ref rect, radii } => {
            canvas.clip_rrect(
                &sk::RRect::new_rect_radii(
                    convert_rect(rect),
                    &[
                        sk::Vector::new(radii[0], radii[0]),
                        sk::Vector::new(radii[1], radii[1]),
                        sk::Vector::new(radii[2], radii[2]),
                        sk::Vector::new(radii[3], radii[3]),
                    ],
                ),
                None,
                true,
            );
        }
        DisplayClip::Ellipse { ref center, radii } => {
            let mut path = sk::Path::new();
            path.add_oval(
                convert_rect(&Rect::new(
                    (center.x - radii.x, center.y - radii.y).into(),
                    (radii.x * 2.0, radii.y * 2.0).into(),
                )),
                None,
            );

            canvas.clip_path(&path, None, true);
        }
    };
}

fn draw_command_group(cmds: &[DisplayCommand], display: &mut SkiaGraphicsDisplay) {
    for cmd in cmds {
        match cmd {
            DisplayCommand::Item(item) => {
                match item {
                    DisplayItem::Graphics(ref item) => match item {
                        GraphicsDisplayItem::Line { a, b, stroke } => {
                            let paint =
                                convert_paint(&GraphicsDisplayPaint::Stroke((*stroke).clone()));
                            display.surface.canvas().draw_line(
                                convert_point(a),
                                convert_point(b),
                                &paint,
                            );
                        }
                        GraphicsDisplayItem::Rectangle { rect, paint } => {
                            let paint = convert_paint(paint);
                            display
                                .surface
                                .canvas()
                                .draw_rect(&convert_rect(rect), &paint);
                        }
                        GraphicsDisplayItem::RoundRectangle { rect, radii, paint } => {
                            let paint = convert_paint(paint);
                            display.surface.canvas().draw_rrect(
                                sk::RRect::new_rect_radii(
                                    convert_rect(rect),
                                    &[
                                        sk::Vector::new(radii[0], radii[0]),
                                        sk::Vector::new(radii[1], radii[1]),
                                        sk::Vector::new(radii[2], radii[2]),
                                        sk::Vector::new(radii[3], radii[3]),
                                    ],
                                ),
                                &paint,
                            );
                        }
                        GraphicsDisplayItem::Ellipse { paint, .. } => {
                            let paint = convert_paint(paint);
                            display
                                .surface
                                .canvas()
                                .draw_oval(convert_rect(&item.bounds()), &paint);
                        }
                        #[allow(irrefutable_let_patterns)]
                        GraphicsDisplayItem::Image { src, dst, resource } => {
                            if let ResourceReference::Image(ref id) = resource {
                                if let Resource::Image(ref img) =
                                    display.resources.get(id).expect("invalid resource ID")
                                {
                                    // TODO(jazzfool): this is really ugly, but I couldn't find a way to `src.map` since `draw_image_rect` expects a Option<(&Rect, ...)>
                                    if let Some(src_rect) = src {
                                        display.surface.canvas().draw_image_rect(
                                            (*img).clone(),
                                            Some((
                                                &convert_rect(src_rect),
                                                sk::SrcRectConstraint::Fast,
                                            )),
                                            &convert_rect(dst),
                                            &sk::Paint::default(),
                                        );
                                    } else {
                                        display.surface.canvas().draw_image_rect(
                                            (*img).clone(),
                                            None,
                                            &convert_rect(dst),
                                            &sk::Paint::default(),
                                        );
                                    }
                                } else {
                                    display.surface.canvas().draw_rect(
                                        &convert_rect(dst),
                                        &convert_paint(&GraphicsDisplayPaint::Fill(
                                            StyleColor::Color(Color::new(1.0, 0.0, 1.0, 1.0)),
                                        )),
                                    );
                                }
                            } else {
                                // TODO(jazzfool): should we panic instead here?
                                display.surface.canvas().draw_rect(
                                    &convert_rect(dst),
                                    &convert_paint(&GraphicsDisplayPaint::Fill(StyleColor::Color(
                                        Color::new(1.0, 0.0, 1.0, 1.0),
                                    ))),
                                );
                            }
                        }
                    },
                    DisplayItem::Text(ref item) => {
                        let paint = convert_paint(&GraphicsDisplayPaint::Fill(item.color.clone()));

                        display.surface.canvas().draw_text_blob(
                            &sk::TextBlob::from_text(
                                item.text.as_bytes(),
                                sk::TextEncoding::UTF8,
                                &sk::Font::new(
                                    sk::Typeface::new(item.font.name(), sk::FontStyle::default())
                                        .unwrap(),
                                    item.size,
                                ),
                            )
                            .unwrap(),
                            convert_point(&item.bottom_left),
                            &paint,
                        );
                    }
                }
            }
            DisplayCommand::BackdropFilter(ref clip, ref filter) => {
                let count = display.surface.canvas().save();

                {
                    let canvas = display.surface.canvas();
                    apply_clip(canvas, clip);
                }

                let bounds = clip.bounds();

                match filter {
                    Filter::Blur(sigma_x, sigma_y) => {
                        // TODO(jazzfool): cache blur filter
                        let snapshot_bounds = bounds.round_out();

                        if let Some(ref snapshot_rect) = snapshot_bounds.intersection(&Rect::new(
                            Point::default(),
                            Size::new(display.size().0 as _, display.size().1 as _),
                        )) {
                            let blur = sk::blur_image_filter::new(
                                (*sigma_x, *sigma_y),
                                display
                                    .surface
                                    .image_snapshot(/*convert_rect(snapshot_rect).round()*/)
                                    //.unwrap()
                                    .as_filter()
                                    .unwrap(),
                                &sk::ImageFilterCropRect::new(&convert_rect(&bounds), None),
                                sk::blur_image_filter::TileMode::Clamp,
                            )
                            .unwrap();

                            {
                                display
                                    .surface
                                    .canvas()
                                    .save_layer(&sk::SaveLayerRec::default().backdrop(&blur));
                            }
                        }
                    }
                    Filter::Invert => {
                        let mut paint = sk::Paint::default();

                        let mut color_matrix = sk::ColorMatrix::default();
                        color_matrix.set_scale(-1.0, -1.0, -1.0, None);

                        paint.set_color_filter(sk::ColorFilters::matrix(&color_matrix));

                        {
                            display
                                .surface
                                .canvas()
                                .save_layer(&sk::SaveLayerRec::default().paint(&paint));
                        }
                    }
                }

                display.surface.canvas().restore_to_count(count);
            }
            DisplayCommand::Clip(ref clip) => {
                apply_clip(display.surface.canvas(), clip);
            }
            DisplayCommand::Save => {
                display.surface.canvas().save();
            }
            DisplayCommand::SaveLayer(opacity) => {
                let mut paint = sk::Paint::default();
                paint.set_alpha_f(*opacity);

                {
                    display
                        .surface
                        .canvas()
                        .save_layer(&sk::SaveLayerRec::default().paint(&paint));
                }
            }
            DisplayCommand::Restore => {
                display.surface.canvas().restore();
            }
            DisplayCommand::Translate(ref offset) => {
                display
                    .surface
                    .canvas()
                    .translate(sk::Vector::new(offset.x, offset.y));
            }
            DisplayCommand::Scale(ref scale) => {
                display.surface.canvas().scale((scale.x, scale.y));
            }
            DisplayCommand::Rotate(ref angle) => {
                display.surface.canvas().rotate(angle.to_degrees(), None);
            }
            DisplayCommand::Clear(ref color) => {
                display
                    .surface
                    .canvas()
                    .clear(convert_color(color).to_color());
            }
        }
    }
}
