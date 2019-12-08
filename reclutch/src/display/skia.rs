//! Robust implementation of [`GraphicsDisplay`](../trait.GraphicsDisplay.html) using Google's Skia.

use super::*;
use {crate::error, skia_safe as sk, std::collections::HashMap};

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
}

enum SurfaceType {
    OpenGlFramebuffer(SkiaOpenGlFramebuffer),
    OpenGlTexture(SkiaOpenGlTexture),
}

enum Resource {
    Image(sk::Image),
    Font(sk::Typeface),
}

/// Converts [`DisplayCommand`](../enum.DisplayCommand.html) to immediate-mode Skia commands.
pub struct SkiaGraphicsDisplay {
    surface: sk::Surface,
    surface_type: SurfaceType,
    context: sk::gpu::Context,
    command_groups: linked_hash_map::LinkedHashMap<u64, (Vec<DisplayCommand>, Rect, bool)>,
    next_command_group_id: u64,
    resources: HashMap<u64, Resource>,
    next_resource_id: u64,
}

impl SkiaGraphicsDisplay {
    /// Creates a new [`SkiaGraphicsDisplay`](struct.SkiaGraphicsDisplay.html) with the Skia OpenGL backend, drawing into an existing framebuffer.
    /// This assumes that an OpenGL context has already been set up.
    /// This also assumes that the color format is RGBA with 8-bit components.
    pub fn new_gl_framebuffer(target: &SkiaOpenGlFramebuffer) -> Result<Self, error::SkiaError> {
        let (surface, context) = Self::new_gl_framebuffer_surface(target)?;
        Ok(Self {
            surface,
            surface_type: SurfaceType::OpenGlFramebuffer(*target),
            context,
            command_groups: linked_hash_map::LinkedHashMap::new(),
            next_command_group_id: 0,
            resources: HashMap::new(),
            next_resource_id: 0,
        })
    }

    /// Creates a new [`SkiaGraphicsDisplay`](struct.SkiaGraphicsDisplay.html) with the Skia OpenGL backend, drawing into an existing texture.
    /// This assumes that an OpenGL context has already been set up.
    /// This also assumes that the color format is RGBA with 8-bit components
    pub fn new_gl_texture(target: &SkiaOpenGlTexture) -> Result<Self, error::SkiaError> {
        let (surface, context) = Self::new_gl_texture_surface(target)?;
        Ok(Self {
            surface,
            surface_type: SurfaceType::OpenGlTexture(*target),
            context,
            command_groups: linked_hash_map::LinkedHashMap::new(),
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
    ) -> Result<(sk::Surface, sk::gpu::Context), error::SkiaError> {
        let mut context = Self::new_gl_context()?;

        Ok((
            SkiaGraphicsDisplay::new_gl_framebuffer_from_context(target, &mut context)?,
            context,
        ))
    }

    fn new_gl_framebuffer_from_context(
        target: &SkiaOpenGlFramebuffer,
        context: &mut sk::gpu::Context,
    ) -> Result<sk::Surface, error::SkiaError> {
        let info = sk::gpu::BackendRenderTarget::new_gl(
            target.size,
            None,
            8,
            sk::gpu::gl::FramebufferInfo {
                fboid: target.framebuffer_id,
                format: gl::RGBA8,
            },
        );

        Ok(sk::Surface::from_backend_render_target(
            context,
            &info,
            sk::gpu::SurfaceOrigin::BottomLeft,
            sk::ColorType::RGBA8888,
            sk::ColorSpace::new_srgb(),
            None,
        )
        .ok_or_else(|| error::SkiaError::InvalidTarget(String::from("framebuffer")))?)
    }

    fn new_gl_texture_surface(
        target: &SkiaOpenGlTexture,
    ) -> Result<(sk::Surface, sk::gpu::Context), error::SkiaError> {
        let mut context = Self::new_gl_context()?;

        Ok((
            SkiaGraphicsDisplay::new_gl_texture_from_context(target, &mut context)?,
            context,
        ))
    }

    fn new_gl_texture_from_context(
        target: &SkiaOpenGlTexture,
        context: &mut sk::gpu::Context,
    ) -> Result<sk::Surface, error::SkiaError> {
        let info = unsafe {
            sk::gpu::BackendTexture::new_gl(
                target.size,
                if target.mip_mapped {
                    sk::gpu::MipMapped::Yes
                } else {
                    sk::gpu::MipMapped::No
                },
                sk::gpu::gl::TextureInfo {
                    format: gl::RGBA8,
                    target: gl::TEXTURE_2D,
                    id: target.texture_id,
                },
            )
        };

        Ok(sk::Surface::from_backend_texture(
            context,
            &info,
            sk::gpu::SurfaceOrigin::BottomLeft,
            None,
            sk::ColorType::RGBA8888,
            sk::ColorSpace::new_srgb(),
            None,
        )
        .ok_or_else(|| error::SkiaError::InvalidTarget(String::from("texture")))?)
    }

    fn new_gl_context() -> Result<sk::gpu::Context, error::SkiaError> {
        sk::gpu::Context::new_gl(sk::gpu::gl::Interface::new_native())
            .ok_or(error::SkiaError::InvalidContext)
    }
}

impl GraphicsDisplay for SkiaGraphicsDisplay {
    fn resize(&mut self, size: (u32, u32)) -> Result<(), Box<dyn std::error::Error>> {
        self.surface = match self.surface_type {
            SurfaceType::OpenGlFramebuffer(ref mut target) => {
                target.size = (size.0 as i32, size.1 as i32);
                Self::new_gl_framebuffer_from_context(target, &mut self.context)
            }
            SurfaceType::OpenGlTexture(ref mut target) => {
                target.size = (size.0 as i32, size.1 as i32);
                Self::new_gl_texture_from_context(target, &mut self.context)
            }
        }?;

        Ok(())
    }

    fn new_resource(
        &mut self,
        descriptor: ResourceDescriptor,
    ) -> Result<ResourceReference, error::ResourceError> {
        let load_data = |data: ResourceData| -> Result<sk::Data, error::ResourceError> {
            Ok(match data {
                ResourceData::File(path) => {
                    if !path.is_file() {
                        return Err(error::ResourceError::InvalidPath(
                            path.to_string_lossy().to_string(),
                        ));
                    }

                    sk::Data::new_copy(&std::fs::read(path)?)
                }
                ResourceData::Data(data) => sk::Data::new_copy(match data {
                    SharedData::RefCount(ref data) => &(*data),
                    SharedData::Static(data) => data,
                }),
            })
        };

        let id = self.next_resource_id;
        let (rid, res) = match &descriptor {
            ResourceDescriptor::Image(data) => (
                ResourceReference::Image(id),
                Resource::Image(match data {
                    ImageData::Encoded(data) => {
                        sk::Image::from_encoded(load_data(data.clone())?, None)
                            .ok_or(error::ResourceError::InvalidData)?
                    }
                    ImageData::Raw(data, info) => sk::Image::from_raster_data(
                        &sk::ImageInfo::new(
                            sk::ISize::new(info.size.0 as _, info.size.1 as _),
                            match info.format {
                                RasterImageFormat::Rgba8 => sk::ColorType::RGBA8888,
                                RasterImageFormat::Bgra8 => sk::ColorType::BGRA8888,
                            },
                            sk::AlphaType::Unpremul,
                            None,
                        ),
                        load_data(data.clone())?,
                        info.size.0 as usize * 4, // width * 4 bytes -> 4 x 8-bit components
                    )
                    .ok_or(error::ResourceError::InvalidData)?,
                }),
            ),
            ResourceDescriptor::Font(data) => (
                ResourceReference::Font(id),
                Resource::Font(
                    sk::Typeface::from_data(load_data(data.clone())?, None)
                        .ok_or(error::ResourceError::InvalidData)?,
                ),
            ),
        };

        self.resources.insert(id, res);
        self.next_resource_id += 1;

        Ok(rid)
    }

    fn remove_resource(&mut self, reference: ResourceReference) {
        self.resources.remove(&reference.id());
    }

    fn push_command_group(
        &mut self,
        commands: &[DisplayCommand],
        protected: Option<bool>,
    ) -> Result<CommandGroupHandle, Box<dyn std::error::Error>> {
        let id = self.next_command_group_id;

        self.command_groups.insert(
            id,
            (
                commands.to_owned(),
                display_list_bounds(commands)?,
                protected.unwrap_or(true),
            ),
        );

        self.next_command_group_id += 1;

        Ok(CommandGroupHandle::new(id))
    }

    #[inline]
    fn get_command_group(&self, handle: CommandGroupHandle) -> Option<&[DisplayCommand]> {
        self.command_groups
            .get(&handle.id())
            .map(|cmdgroup| &cmdgroup.0[..])
    }

    fn modify_command_group(
        &mut self,
        handle: CommandGroupHandle,
        commands: &[DisplayCommand],
        protected: Option<bool>,
    ) {
        if self.command_groups.contains_key(&handle.id()) {
            if let Ok(bounds) = display_list_bounds(commands) {
                self.command_groups.insert(
                    handle.id(),
                    (commands.to_owned(), bounds, protected.unwrap_or(true)),
                );
            }
        }
    }

    fn maintain_command_group(&mut self, handle: CommandGroupHandle) {
        self.command_groups.get_refresh(&handle.id());
    }

    #[inline]
    fn remove_command_group(&mut self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        Some(self.command_groups.remove(&handle.id())?.0)
    }

    #[inline]
    fn before_exit(&mut self) {
        self.surface.flush()
    }

    fn present(&mut self, cull: Option<Rect>) -> Result<(), error::DisplayError> {
        let cmds = self
            .command_groups
            .values()
            .map(|cmds| (&cmds.0, &cmds.1, &cmds.2))
            .filter_map(|(cmd_group, bounds, protected)| {
                if cull.map(|cull| cull.intersects(bounds)).unwrap_or(true) {
                    Some((cmd_group, protected))
                } else {
                    None
                }
            });
        let resources = &self.resources;
        let size = self.size();
        let surface = &mut self.surface;
        for cmd_group in cmds {
            let count = if *cmd_group.1 {
                Some(surface.canvas().save())
            } else {
                None
            };

            draw_command_group(cmd_group.0, surface, resources, size)?;

            if let Some(count) = count {
                surface.canvas().restore_to_count(count);
            }
        }

        surface.flush();

        Ok(())
    }
}

fn convert_color(color: Color) -> sk::Color4f {
    sk::Color4f::new(color.red, color.green, color.blue, color.alpha)
}

fn convert_point(point: Point) -> sk::Point {
    sk::Point::new(point.x, point.y)
}

fn apply_color(color: &StyleColor, paint: &mut sk::Paint) -> Result<(), error::SkiaError> {
    match color {
        StyleColor::Color(ref color) => {
            // we can afford to "make" the SRGB color space every time; it's actually a singleton in the C++ Skia code.
            paint.set_color4f(convert_color(*color), &sk::ColorSpace::new_srgb());
        }
        StyleColor::LinearGradient(ref gradient) => {
            let (colors, stops): (Vec<_>, Vec<_>) = gradient
                .stops
                .iter()
                .map(|stop| (convert_color(stop.1).to_color(), stop.0 as sk::scalar))
                .unzip();

            paint.set_shader(
                sk::gradient_shader::linear(
                    (convert_point(gradient.start), convert_point(gradient.end)),
                    sk::gradient_shader::GradientShaderColors::Colors(&colors[..]),
                    &stops[..],
                    sk::TileMode::default(),
                    None,
                    None,
                )
                .ok_or(error::SkiaError::UnknownError)?,
            );
        }
        StyleColor::RadialGradient(ref gradient) => {
            let (colors, stops): (Vec<_>, Vec<_>) = gradient
                .stops
                .iter()
                .map(|stop| (convert_color(stop.1).to_color(), stop.0 as sk::scalar))
                .unzip();

            paint.set_shader(sk::gradient_shader::radial(
                convert_point(gradient.start),
                (gradient.end - gradient.start).length(),
                sk::gradient_shader::GradientShaderColors::Colors(&colors[..]),
                &stops[..],
                sk::TileMode::default(),
                None,
                None,
            ));
        }
    };

    Ok(())
}

fn convert_line_cap(cap: LineCap) -> sk::PaintCap {
    match cap {
        LineCap::Flat => sk::PaintCap::Butt,
        LineCap::Square => sk::PaintCap::Square,
        LineCap::Round => sk::PaintCap::Round,
    }
}

fn convert_line_join(join: LineJoin) -> sk::PaintJoin {
    match join {
        LineJoin::Miter => sk::PaintJoin::Miter,
        LineJoin::Round => sk::PaintJoin::Round,
        LineJoin::Bevel => sk::PaintJoin::Bevel,
    }
}

fn apply_filter_to_paint(paint: &mut sk::Paint, filter: Option<Filter>) {
    if let Some(filter) = filter {
        match filter {
            Filter::Blur(sigma_x, sigma_y) => {
                paint.set_image_filter(sk::image_filters::blur(
                    (sigma_x, sigma_y),
                    sk::TileMode::Decal,
                    None,
                    None,
                ));
            }
            Filter::Invert => {
                let mut color_matrix = sk::ColorMatrix::default();
                color_matrix.set_20(&[
                    -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0, 1.0, 0.0,
                    1.0, 1.0, 1.0, 1.0, 0.0,
                ]);

                paint.set_color_filter(sk::ColorFilters::matrix(&color_matrix));
            }
        }
    }
}

fn convert_paint(
    gdpaint: &GraphicsDisplayPaint,
    filter: Option<Filter>,
) -> Result<sk::Paint, error::SkiaError> {
    let mut paint = sk::Paint::default();

    match gdpaint {
        GraphicsDisplayPaint::Fill(ref color) => {
            paint.set_anti_alias(true);

            apply_color(color, &mut paint)?;
        }
        GraphicsDisplayPaint::Stroke(ref stroke) => {
            paint.set_anti_alias(stroke.antialias);
            paint.set_style(sk::PaintStyle::Stroke);

            apply_color(&stroke.color, &mut paint)?;

            paint.set_stroke_width(stroke.thickness);
            paint.set_stroke_cap(convert_line_cap(stroke.cap));
            paint.set_stroke_join(convert_line_join(stroke.join));
            paint.set_stroke_miter(stroke.miter_limit);
        }
    }

    apply_filter_to_paint(&mut paint, filter);

    Ok(paint)
}

fn convert_rect(rect: &Rect) -> sk::Rect {
    sk::Rect::from_xywh(
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    )
}

fn convert_display_text(
    text: &DisplayText,
    font: sk::Font,
) -> Result<sk::TextBlob, error::SkiaError> {
    match text {
        DisplayText::Simple(ref text) => {
            sk::TextBlob::from_text(text.as_bytes(), sk::TextEncoding::UTF8, &font)
                .ok_or(error::SkiaError::UnknownError)
        }
        DisplayText::Shaped(ref glyphs) => {
            let mut builder = sk::TextBlobBuilder::new();
            let blob_glyphs = builder.alloc_run_pos(font, glyphs.len(), None);

            let mut xy = Point::new(0.0, 0.0);
            for (i, glyph) in glyphs.iter().enumerate() {
                blob_glyphs.0[i] = glyph.codepoint as u16;
                blob_glyphs.1[i].x = xy.x + glyph.offset.x;
                blob_glyphs.1[i].y = xy.y - glyph.offset.y;
                xy += glyph.advance;
            }

            builder.make().ok_or(error::SkiaError::UnknownError)
        }
    }
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

// The meat of this module.
// If there are any drawing bugs, they probably happen here.
fn draw_command_group(
    cmds: &[DisplayCommand],
    surface: &mut sk::Surface,
    resources: &HashMap<u64, Resource>,
    size: (i32, i32),
) -> Result<(), error::DisplayError> {
    for cmd in cmds {
        match cmd {
            DisplayCommand::Item(item, filter) => match item {
                DisplayItem::Graphics(ref item) => match item {
                    GraphicsDisplayItem::Line { a, b, stroke } => {
                        let paint = convert_paint(
                            &GraphicsDisplayPaint::Stroke((*stroke).clone()),
                            *filter,
                        )
                        .map_err(|e| error::DisplayError::InternalError(e.into()))?;
                        surface
                            .canvas()
                            .draw_line(convert_point(*a), convert_point(*b), &paint);
                    }
                    GraphicsDisplayItem::Rectangle { rect, paint } => {
                        let paint = convert_paint(paint, *filter)
                            .map_err(|e| error::DisplayError::InternalError(e.into()))?;
                        surface.canvas().draw_rect(&convert_rect(rect), &paint);
                    }
                    GraphicsDisplayItem::RoundRectangle { rect, radii, paint } => {
                        let paint = convert_paint(paint, *filter)
                            .map_err(|e| error::DisplayError::InternalError(e.into()))?;
                        surface.canvas().draw_rrect(
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
                        surface.canvas().draw_oval(
                            convert_rect(&item.bounds()),
                            &convert_paint(paint, *filter)
                                .map_err(|e| error::DisplayError::InternalError(e.into()))?,
                        );
                    }
                    GraphicsDisplayItem::Image { src, dst, resource } => {
                        if let ResourceReference::Image(ref id) = resource {
                            if let Resource::Image(ref img) = resources
                                .get(id)
                                .ok_or(error::DisplayError::InvalidResource(*id))?
                            {
                                surface.canvas().save();

                                let mut paint = sk::Paint::default();
                                paint.set_filter_quality(sk::FilterQuality::Medium); // TODO(jazzfool): perhaps we can expose the image filter quality?

                                apply_filter_to_paint(&mut paint, *filter);

                                apply_clip(
                                    surface.canvas(),
                                    &DisplayClip::Rectangle {
                                        rect: *dst,
                                        antialias: true,
                                    },
                                );

                                let o_src = src.map(|src_rect| convert_rect(&src_rect));
                                surface.canvas().draw_image_rect(
                                    (*img).clone(),
                                    o_src
                                        .as_ref()
                                        .map(|src_rect| (src_rect, sk::SrcRectConstraint::Fast)),
                                    &convert_rect(dst),
                                    &paint,
                                );

                                surface.canvas().restore();
                            }
                        } else {
                            return Err(error::DisplayError::MismatchedResource(resource.id()));
                        }
                    }
                },
                DisplayItem::Text(ref item) => {
                    if let ResourceReference::Font(ref id) = item.font {
                        if let Resource::Font(ref typeface) = resources
                            .get(id)
                            .ok_or(error::DisplayError::InvalidResource(*id))?
                        {
                            let paint = convert_paint(
                                &GraphicsDisplayPaint::Fill(item.color.clone()),
                                *filter,
                            )
                            .map_err(|e| error::DisplayError::InternalError(e.into()))?;

                            surface.canvas().draw_text_blob(
                                &convert_display_text(
                                    &item.text,
                                    sk::Font::new(typeface.clone(), item.size),
                                )
                                .map_err(|e| error::DisplayError::InternalError(e.into()))?,
                                convert_point(item.bottom_left),
                                &paint,
                            );
                        }
                    } else {
                        return Err(error::DisplayError::MismatchedResource(item.font.id()));
                    }
                }
            },
            DisplayCommand::BackdropFilter(ref clip, ref filter) => {
                let count = surface.canvas().save();

                apply_clip(surface.canvas(), clip);

                let bounds = clip.bounds();

                match filter {
                    Filter::Blur(sigma_x, sigma_y) => {
                        // TODO(jazzfool): cache blur filter (figure out a way to cache by floats)
                        if let Some(ref _snapshot_rect) = bounds.round_out().intersection(
                            &Rect::new(Point::default(), Size::new(size.0 as _, size.1 as _)),
                        ) {
                            let blur = sk::image_filters::blur(
                                (*sigma_x, *sigma_y),
                                sk::TileMode::Clamp,
                                None,
                                &convert_rect(&bounds).round(),
                            )
                            .ok_or_else(|| {
                                error::DisplayError::InternalError(Box::new(
                                    error::SkiaError::UnknownError,
                                ))
                            })?;

                            surface
                                .canvas()
                                .save_layer(&sk::SaveLayerRec::default().backdrop(&blur));
                        }
                    }
                    Filter::Invert => {
                        let mut paint = sk::Paint::default();

                        let mut color_matrix = sk::ColorMatrix::default();
                        color_matrix.set_20(&[
                            -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0,
                            1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0,
                        ]);

                        paint.set_color_filter(sk::ColorFilters::matrix(&color_matrix));

                        surface
                            .canvas()
                            .save_layer(&sk::SaveLayerRec::default().paint(&paint));
                    }
                }

                surface.canvas().restore_to_count(count);
            }
            DisplayCommand::Clip(ref clip) => {
                apply_clip(surface.canvas(), clip);
            }
            DisplayCommand::Save => {
                surface.canvas().save();
            }
            DisplayCommand::SaveLayer(opacity) => {
                let mut paint = sk::Paint::default();
                paint.set_alpha_f(*opacity);

                surface
                    .canvas()
                    .save_layer(&sk::SaveLayerRec::default().paint(&paint));
            }
            DisplayCommand::Restore => {
                surface.canvas().restore();
            }
            DisplayCommand::Translate(ref offset) => {
                surface
                    .canvas()
                    .translate(sk::Vector::new(offset.x, offset.y));
            }
            DisplayCommand::Scale(ref scale) => {
                surface.canvas().scale((scale.x, scale.y));
            }
            DisplayCommand::Rotate(ref angle) => {
                surface.canvas().rotate(angle.to_degrees(), None);
            }
            DisplayCommand::Clear(ref color) => {
                surface.canvas().clear(convert_color(*color).to_color());
            }
        }
    }

    Ok(())
}
