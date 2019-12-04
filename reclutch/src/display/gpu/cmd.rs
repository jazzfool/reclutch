//! Everything related to converting high-level vector graphics into GPU commands.

use super::common::{tess::basic_shapes as shapes, *};

pub(crate) fn upload_to_buffer<T: 'static + Sized + Copy>(
    device: &wgpu::Device,
    queue: &mut wgpu::Queue,
    contents: &[T],
    buffer: &wgpu::Buffer,
) {
    let temp_buf = device
        .create_buffer_mapped(contents.len(), wgpu::BufferUsage::COPY_SRC)
        .fill_from_slice(contents);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });
    encoder.copy_buffer_to_buffer(
        &temp_buf,
        0,
        buffer,
        0,
        (std::mem::size_of::<T>() * contents.len()) as _,
    );
    queue.submit(&[encoder.finish()]);
}

fn clip_to_gd_item(clip: &DisplayClip) -> GraphicsDisplayItem {
    unimplemented!()
}

fn convert_stroke(stroke: GraphicsDisplayStroke) -> tess::StrokeOptions {
    tess::StrokeOptions::default()
        .with_line_width(stroke.thickness)
        .with_line_cap(match stroke.cap {
            LineCap::Flat => tess::LineCap::Butt,
            LineCap::Square => tess::LineCap::Square,
            LineCap::Round => tess::LineCap::Round,
        })
        .with_line_join(match stroke.join {
            LineJoin::Miter => tess::LineJoin::Miter,
            LineJoin::Round => tess::LineJoin::Round,
            LineJoin::Bevel => tess::LineJoin::Bevel,
        })
        .with_miter_limit(stroke.miter_limit)
}

fn tessellate(gd_item: &GraphicsDisplayItem) -> (Vec<Vertex>, Vec<u16>) {
    let mut buffer: tess::VertexBuffers<Vertex, u16> = Default::default();

    match gd_item {
        GraphicsDisplayItem::Line { a, b, stroke } => shapes::stroke_polyline(
            [*a, *b].iter().copied(),
            false,
            &convert_stroke(stroke.clone()),
            &mut tess::BuffersBuilder::new(
                &mut buffer,
                VertexCtor(stroke.color.color_or_black(), None),
            ),
        ),
        GraphicsDisplayItem::Rectangle { rect, paint } => match paint {
            GraphicsDisplayPaint::Fill(color) => shapes::fill_rectangle(
                rect,
                &Default::default(),
                &mut tess::BuffersBuilder::new(
                    &mut buffer,
                    VertexCtor(color.color_or_black(), None),
                ),
            ),
            GraphicsDisplayPaint::Stroke(stroke) => shapes::stroke_rectangle(
                rect,
                &convert_stroke(stroke.clone()),
                &mut tess::BuffersBuilder::new(
                    &mut buffer,
                    VertexCtor(stroke.color.color_or_black(), None),
                ),
            ),
        },
        GraphicsDisplayItem::RoundRectangle { rect, radii, paint } => match paint {
            GraphicsDisplayPaint::Fill(color) => shapes::fill_rounded_rectangle(
                rect,
                &shapes::BorderRadii::new(radii[0], radii[1], radii[2], radii[3]),
                &Default::default(),
                &mut tess::BuffersBuilder::new(
                    &mut buffer,
                    VertexCtor(color.color_or_black(), None),
                ),
            ),
            GraphicsDisplayPaint::Stroke(stroke) => shapes::stroke_rounded_rectangle(
                rect,
                &shapes::BorderRadii::new(radii[0], radii[1], radii[2], radii[3]),
                &convert_stroke(stroke.clone()),
                &mut tess::BuffersBuilder::new(
                    &mut buffer,
                    VertexCtor(stroke.color.color_or_black(), None),
                ),
            ),
        },
        GraphicsDisplayItem::Ellipse {
            center,
            radii,
            paint,
        } => match paint {
            GraphicsDisplayPaint::Fill(color) => shapes::fill_ellipse(
                *center,
                *radii,
                Angle::zero(),
                &Default::default(),
                &mut tess::BuffersBuilder::new(
                    &mut buffer,
                    VertexCtor(color.color_or_black(), None),
                ),
            ),
            GraphicsDisplayPaint::Stroke(stroke) => shapes::stroke_ellipse(
                *center,
                *radii,
                Angle::zero(),
                &convert_stroke(stroke.clone()),
                &mut tess::BuffersBuilder::new(
                    &mut buffer,
                    VertexCtor(stroke.color.color_or_black(), None),
                ),
            ),
        },
        GraphicsDisplayItem::Image { dst, .. } => shapes::fill_rectangle(
            dst,
            &Default::default(),
            &mut tess::BuffersBuilder::new(
                &mut buffer,
                VertexCtor(Color::new(0.0, 0.0, 0.0, 1.0), Some(*dst)),
            ),
        ),
    };

    (buffer.vertices, buffer.indices)
}

fn create_render_paint(
    gd_item: &GraphicsDisplayItem,
    device: &wgpu::Device,
    pipelines: &super::pipe::PipelineData,
    resources: &super::pipe::ResourceMap,
) -> Result<(RenderPaint, Option<wgpu::Buffer>), error::DisplayError> {
    match gd_item {
        GraphicsDisplayItem::Line { stroke, .. } => Ok(RenderPaint::from_style_color(
            stroke.color.clone(),
            device,
            pipelines,
        )),
        GraphicsDisplayItem::Rectangle { paint, .. }
        | GraphicsDisplayItem::RoundRectangle { paint, .. }
        | GraphicsDisplayItem::Ellipse { paint, .. } => match paint {
            GraphicsDisplayPaint::Fill(color) => Ok(RenderPaint::from_style_color(
                color.clone(),
                device,
                pipelines,
            )),
            GraphicsDisplayPaint::Stroke(stroke) => Ok(RenderPaint::from_style_color(
                stroke.color.clone(),
                device,
                pipelines,
            )),
        },
        GraphicsDisplayItem::Image { resource, .. } => {
            if let ResourceReference::Image(ref id) = resource {
                if let Some(Resource::Image(ref texture_view)) = resources.get(id) {
                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &pipelines.image_bind_group_layout,
                        bindings: &[
                            wgpu::Binding {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(texture_view),
                            },
                            wgpu::Binding {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&pipelines.image_sampler),
                            },
                        ],
                    });

                    Ok((RenderPaint::Image(bind_group), None))
                } else {
                    Err(error::DisplayError::InvalidResource(*id))
                }
            } else {
                Err(error::DisplayError::MismatchedResource(resource.id()))
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum SaveState<T: std::fmt::Debug, L: std::fmt::Debug> {
    Transform(T),
    Layer(L),
}

#[derive(Debug)]
pub(crate) struct SaveStack<T: std::fmt::Debug, L: std::fmt::Debug> {
    stack: Vec<SaveState<T, L>>,
    top_transform: Option<usize>,
    top_layer: Option<usize>,
}

impl<T: std::fmt::Debug, L: std::fmt::Debug> Default for SaveStack<T, L> {
    fn default() -> Self {
        SaveStack {
            stack: Vec::new(),
            top_transform: None,
            top_layer: None,
        }
    }
}

impl<T: std::fmt::Debug, L: std::fmt::Debug> SaveStack<T, L> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn save(&mut self, state: SaveState<T, L>) {
        *match &state {
            SaveState::Transform(_) => &mut self.top_transform,
            SaveState::Layer(_) => &mut self.top_layer,
        } = Some(self.stack.len());

        self.stack.push(state);
    }

    pub fn restore(&mut self) -> Option<SaveState<T, L>> {
        let state = if self.stack.len() > 0 {
            Some(self.stack.remove(self.stack.len() - 1))
        } else {
            None
        };

        match &state {
            Some(SaveState::Transform(_)) => {
                self.top_transform = self.stack.iter().rev().position(|state| {
                    if let SaveState::Transform(_) = state {
                        true
                    } else {
                        false
                    }
                })
            }
            Some(SaveState::Layer(_)) => {
                self.top_layer = self.stack.iter().rev().position(|state| {
                    if let SaveState::Layer(_) = state {
                        true
                    } else {
                        false
                    }
                })
            }
            _ => (),
        };

        state
    }

    pub fn peek_transform(&self) -> Option<&T> {
        self.top_transform.map(|idx| {
            if let SaveState::Transform(transform) = self.stack.get(idx).unwrap() {
                transform
            } else {
                panic!()
            }
        })
    }

    pub fn peek_layer(&self) -> Option<&L> {
        self.top_layer.map(|idx| {
            if let SaveState::Layer(layer) = self.stack.get(idx).unwrap() {
                layer
            } else {
                panic!()
            }
        })
    }
}

pub(crate) struct CommandGroupData {
    pub display: Vec<DisplayCommand>,
    pub render: Vec<RenderCommand>,

    pub bounds: Rect,

    // TODO(jazzfool): combine these
    pub vbo: wgpu::Buffer,
    pub ibo: wgpu::Buffer,
    pub ubo: wgpu::Buffer,

    pub locals_bind_group: wgpu::BindGroup,
}

impl CommandGroupData {
    pub fn new(
        commands: &[DisplayCommand],
        device: &wgpu::Device,
        pipelines: &super::pipe::PipelineData,
        resources: &super::pipe::ResourceMap,
        protected: Option<bool>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut render = Vec::new();

        let locals = Locals {
            transform: nalgebra::Matrix4::identity(),
        };

        let mut save_count = 0;

        for cmd in commands {
            match cmd {
                DisplayCommand::Item(item) => match item {
                    DisplayItem::Graphics(gd_item) => {
                        let paint = create_render_paint(gd_item, device, pipelines, resources)
                            .map_err(|err| Box::new(err))?
                            .0;

                        let (mesh_vertices, mesh_indices) = tessellate(gd_item);

                        let mesh = Mesh {
                            base_vertex: vertices.len() as _,
                            indices: indices.len() as _..(indices.len() + mesh_indices.len()) as _,
                        };

                        vertices.extend(mesh_vertices.into_iter());
                        indices.extend(mesh_indices.into_iter());

                        render.push(RenderCommand::DrawMesh { mesh, paint });
                    }
                    DisplayItem::Text(td_item) => {}
                },
                DisplayCommand::BackdropFilter(clip, filter) => {}
                DisplayCommand::Save => {
                    save_count += 1;
                    render.push(RenderCommand::Save);
                }
                DisplayCommand::SaveLayer(opacity) => {
                    save_count += 1;
                    render.push(RenderCommand::SaveTexture(*opacity));
                }
                DisplayCommand::Restore => {
                    if save_count > 0 {
                        save_count -= 1;
                        render.push(RenderCommand::Restore);
                    } // else error?
                }
                _ => (),
            }
        }

        if protected.unwrap_or(true) {
            for i in 0..save_count {
                render.push(RenderCommand::Restore);
            }
        }

        let ubo = device
            .create_buffer_mapped(1, wgpu::BufferUsage::UNIFORM)
            .fill_from_slice(&[locals]);
        let locals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &pipelines.locals_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &ubo,
                    range: 0..std::mem::size_of::<Locals>() as _,
                },
            }],
        });

        Ok(CommandGroupData {
            display: commands.to_vec(),
            render,
            bounds: display_list_bounds(commands).map_err(|err| Box::new(err))?,
            vbo: device
                .create_buffer_mapped(vertices.len(), wgpu::BufferUsage::VERTEX)
                .fill_from_slice(&vertices),
            ibo: device
                .create_buffer_mapped(indices.len(), wgpu::BufferUsage::INDEX)
                .fill_from_slice(&indices),
            ubo,
            locals_bind_group,
        })
    }

    pub fn modify(&mut self, new_cmds: &[DisplayCommand]) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mesh {
    pub base_vertex: u32,
    pub indices: std::ops::Range<u32>,
}

#[derive(Debug)]
pub(crate) enum RenderCommand {
    DrawMesh {
        mesh: Mesh,
        paint: RenderPaint,
    },
    DrawBackdropFilter {
        clip: Mesh,
        filter_pipe: wgpu::RenderPipeline,
    },
    DrawMeshToStencil {
        mesh: Mesh,
    },
    Save,
    SaveTexture(f32),
    Restore,
    Translate,
    Scale,
    Rotate,
    Clear,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
struct GpuGradientStop {
    pos: f32,
    _pad: [u32; 3],
    color: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
struct GpuGradient {
    start: [f32; 2],
    end: [f32; 2],
    stops: [GpuGradientStop; 32],
    length: u32,
}

impl GpuGradient {
    fn new(gradient: &Gradient) -> Self {
        let mut stops: Vec<_> = gradient
            .stops
            .iter()
            .map(|(pos, color)| GpuGradientStop {
                pos: *pos,
                _pad: [0; 3],
                color: color_to_rgba(*color),
            })
            .collect();

        let length = stops.len() as u32;
        stops.resize(
            32,
            GpuGradientStop {
                pos: 0.0,
                _pad: [0; 3],
                color: [0.0; 4],
            },
        );

        let stops = copy_to_array(&stops);

        GpuGradient {
            start: gradient.start.to_array(),
            end: gradient.end.to_array(),
            stops,
            length,
        }
    }
}

#[derive(Debug)]
pub(crate) enum RenderPaint {
    Fill,
    LinearGradient(wgpu::BindGroup),
    RadialGradient(wgpu::BindGroup),
    Image(wgpu::BindGroup),
}

impl RenderPaint {
    fn from_style_color(
        sc: StyleColor,
        device: &wgpu::Device,
        pipelines: &PipelineData,
    ) -> (Self, Option<wgpu::Buffer>) {
        match sc {
            StyleColor::Color(_) => (RenderPaint::Fill, None),
            StyleColor::LinearGradient(ref gradient) => {
                let gradient = GpuGradient::new(gradient);
                let buf = device
                    .create_buffer_mapped(1, wgpu::BufferUsage::UNIFORM)
                    .fill_from_slice(&[gradient]);
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &pipelines.gradient_bind_group_layout,
                    bindings: &[wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &buf,
                            range: 0..std::mem::size_of::<GpuGradient>() as _,
                        },
                    }],
                });

                (RenderPaint::LinearGradient(bind_group), buf.into())
            }
            StyleColor::RadialGradient(ref gradient) => {
                let gradient = GpuGradient::new(gradient);
                let buf = device
                    .create_buffer_mapped(1, wgpu::BufferUsage::UNIFORM)
                    .fill_from_slice(&[gradient]);
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &pipelines.gradient_bind_group_layout,
                    bindings: &[wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &buf,
                            range: 0..std::mem::size_of::<GpuGradient>() as _,
                        },
                    }],
                });
                (RenderPaint::RadialGradient(bind_group), buf.into())
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum Resource {
    Image(wgpu::TextureView),
    Font(wgpu_glyph::GlyphBrush<'static, ()>),
}
