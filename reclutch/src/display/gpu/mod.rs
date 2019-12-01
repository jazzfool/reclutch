use {
    super::*,
    image::ConvertBuffer,
    lyon::tessellation::{self as tess, basic_shapes as shapes},
    raw_window_handle::HasRawWindowHandle,
    std::{collections::HashMap, error::Error},
};

const SAMPLE_COUNT: u32 = 8;

struct CommandGroupData {
    display: Vec<DisplayCommand>,
    render: Vec<RenderCommand>,

    bounds: Rect,

    // TODO(jazzfool): combine these
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    ubo: wgpu::Buffer,

    locals_bind_group: wgpu::BindGroup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mesh {
    base_vertex: u32,
    indices: std::ops::Range<u32>,
}

#[derive(Debug)]
enum RenderCommand {
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

fn copy_to_array<T: Sized + Default + Copy>(src: &[T]) -> [T; 32] {
    assert!(
        src.len() <= 32,
        "WGPU implementation only supports up to 32 gradient stops; received {}",
        src.len()
    );
    let mut array = [T::default(); 32];
    let bytes = &src[..array.len()];
    array.copy_from_slice(src);
    array
}

fn color_to_bgra(color: Color) -> [f32; 4] {
    [color.blue, color.green, color.red, color.alpha]
}

fn color_to_rgba(color: Color) -> [f32; 4] {
    [color.red, color.green, color.blue, color.alpha]
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
enum RenderPaint {
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[repr(align(16))]
struct Vertex {
    pos: [f32; 2],
    tex_coord: [f32; 2],
    color: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Uniforms {
    ortho: nalgebra::Matrix4<f32>,
    transform: nalgebra::Matrix4<f32>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Locals {
    transform: nalgebra::Matrix4<f32>,
}

#[derive(Debug)]
enum Resource {
    Image(wgpu::TextureView),
    Font(wgpu_glyph::GlyphBrush<'static, ()>),
}

struct VertexCtor(Color, Option<Rect>);

impl tess::VertexConstructor<tess::FillVertex, Vertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: tess::FillVertex) -> Vertex {
        Vertex {
            pos: vertex.position.to_array(),
            tex_coord: self
                .1
                .map(|rect| {
                    let rel = vertex.position - rect.origin;
                    let max = rect.origin + rect.size;
                    Point::new(rel.x / max.x, rel.y / max.y).to_array()
                })
                .unwrap_or_else(|| [0.0; 2]),
            color: color_to_rgba(self.0),
        }
    }
}

impl tess::VertexConstructor<tess::StrokeVertex, Vertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: tess::StrokeVertex) -> Vertex {
        Vertex {
            pos: vertex.position.to_array(),
            tex_coord: self
                .1
                .map(|rect| {
                    let rel = vertex.position - rect.origin;
                    let max = rect.origin + rect.size;
                    Point::new(rel.x / max.x, rel.y / max.y).to_array()
                })
                .unwrap_or_else(|| [0.0; 2]),
            color: color_to_rgba(self.0),
        }
    }
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

fn load_shader(
    shader: &str,
    filename: &str,
    kind: shaderc::ShaderKind,
    compiler: &mut shaderc::Compiler,
    format: wgpu::TextureFormat,
    device: &wgpu::Device,
) -> Result<wgpu::ShaderModule, error::GpuError> {
    let options = shaderc::CompileOptions::new()
        .ok_or_else(|| error::GpuError::CompilerError("failed to create compile options".into()))?;
    let spirv = compiler
        .compile_into_spirv(shader, kind, filename, "main", Some(&options))
        .map_err(|err| error::GpuError::CompilerError(err.description().into()))?;
    Ok(device.create_shader_module(spirv.as_binary()))
}

fn create_pipeline(
    vs: &wgpu::ShaderModule,
    fs: &wgpu::ShaderModule,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { bind_group_layouts });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: vs,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: fs,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states: &[wgpu::ColorStateDescriptor {
            format,
            color_blend: wgpu::BlendDescriptor::REPLACE,
            alpha_blend: wgpu::BlendDescriptor::REPLACE,
            write_mask: wgpu::ColorWrite::ALL,
        }],
        depth_stencil_state: None,
        index_format: wgpu::IndexFormat::Uint16,
        vertex_buffers: &[wgpu::VertexBufferDescriptor {
            step_mode: wgpu::InputStepMode::Vertex,
            stride: std::mem::size_of::<Vertex>() as _,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    format: wgpu::VertexFormat::Float2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttributeDescriptor {
                    format: wgpu::VertexFormat::Float2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttributeDescriptor {
                    format: wgpu::VertexFormat::Float4,
                    offset: 16,
                    shader_location: 2,
                },
            ],
        }],
        sample_count: SAMPLE_COUNT,
        sample_mask: !0,
        alpha_to_coverage_enabled: true,
    })
}

fn create_render_paint(
    gd_item: &GraphicsDisplayItem,
    device: &wgpu::Device,
    pipelines: &PipelineData,
    resources: &HashMap<u64, Resource>,
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

fn create_msaa_framebuffer(
    device: &wgpu::Device,
    swap_chain_desc: &wgpu::SwapChainDescriptor,
    sample_count: u32,
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: swap_chain_desc.width,
        height: swap_chain_desc.height,
        depth: 1,
    };

    let frame_descriptor = &wgpu::TextureDescriptor {
        size,
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: swap_chain_desc.format,
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
    };

    device
        .create_texture(frame_descriptor)
        .create_default_view()
}

fn upload_to_buffer<T: 'static + Sized + Copy>(
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

#[derive(Debug)]
struct PipelineData {
    fill_pipeline: wgpu::RenderPipeline,
    linear_gradient_pipeline: wgpu::RenderPipeline,
    radial_gradient_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,

    globals_bind_group: wgpu::BindGroup,
    locals_bind_group_layout: wgpu::BindGroupLayout,
    gradient_bind_group_layout: wgpu::BindGroupLayout,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
}

#[derive(Debug)]
enum SaveState<T: std::fmt::Debug, L: std::fmt::Debug> {
    Transform(T),
    Layer(L),
}

#[derive(Debug)]
struct SaveStack<T: std::fmt::Debug, L: std::fmt::Debug> {
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
    fn new() -> Self {
        Default::default()
    }

    fn save(&mut self, state: SaveState<T, L>) {
        *match &state {
            SaveState::Transform(_) => &mut self.top_transform,
            SaveState::Layer(_) => &mut self.top_layer,
        } = Some(self.stack.len());

        self.stack.push(state);
    }

    fn restore(&mut self) -> Option<SaveState<T, L>> {
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

    fn peek_transform(&self) -> Option<&T> {
        self.top_transform.map(|idx| {
            if let SaveState::Transform(transform) = self.stack.get(idx).unwrap() {
                transform
            } else {
                panic!()
            }
        })
    }

    fn peek_layer(&self) -> Option<&L> {
        self.top_layer.map(|idx| {
            if let SaveState::Layer(layer) = self.stack.get(idx).unwrap() {
                layer
            } else {
                panic!()
            }
        })
    }
}

pub struct GpuGraphicsDisplay {
    command_groups: linked_hash_map::LinkedHashMap<u64, CommandGroupData>,
    next_command_group_id: u64,
    resources: HashMap<u64, Resource>,
    next_resource_id: u64,

    globals: Uniforms,
    globals_buffer: wgpu::Buffer,
    pipelines: PipelineData,
    device: wgpu::Device,
    queue: wgpu::Queue,
    swap_chain: wgpu::SwapChain,
    swap_chain_desc: wgpu::SwapChainDescriptor,
    surface: wgpu::Surface,
    msaa_framebuffer: wgpu::TextureView,
}

impl GpuGraphicsDisplay {
    pub fn new(
        window: &impl HasRawWindowHandle,
        width: u32,
        height: u32,
    ) -> Result<Self, error::GpuError> {
        let surface = wgpu::Surface::create(window);

        let adapter = wgpu::Adapter::request(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::Default,
            backends: wgpu::BackendBit::PRIMARY,
        })
        .ok_or_else(|| error::GpuError::AdapterError)?;

        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            extensions: wgpu::Extensions {
                anisotropic_filtering: false,
            },
            limits: wgpu::Limits::default(),
        });

        let swap_chain_desc = wgpu::SwapChainDescriptor {
            width,
            height,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            present_mode: wgpu::PresentMode::Vsync,
        };

        let swap_chain = device.create_swap_chain(&surface, &swap_chain_desc);

        let msaa_framebuffer = create_msaa_framebuffer(&device, &swap_chain_desc, SAMPLE_COUNT);

        let global_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                }],
            });

        let locals_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                }],
            });

        let gradient_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                }],
            });

        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[
                    wgpu::BindGroupLayoutBinding {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutBinding {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler,
                    },
                ],
            });

        let globals = Uniforms {
            ortho: nalgebra::Matrix4::new_orthographic(
                0.0,
                width as _,
                0.0,
                height as _,
                -1.0,
                0.0,
            ),
            transform: nalgebra::Matrix4::identity(),
        };

        let globals_buffer = device
            .create_buffer_mapped(1, wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST)
            .fill_from_slice(&[globals]);

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &global_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &globals_buffer,
                    range: 0..std::mem::size_of::<Uniforms>() as _,
                },
            }],
        });

        let mut compiler = shaderc::Compiler::new().ok_or_else(|| {
            error::GpuError::CompilerError("failed to create shaderc compiler".into())
        })?;

        let vertex_shader = load_shader(
            "
            #version 430

            layout(location = 0) in vec2 v_pos;
            layout(location = 1) in vec2 v_tex_coord;
            layout(location = 2) in vec4 v_color;

            layout(location = 0) out vec2 f_pos;
            layout(location = 1) out vec2 f_tex_coord;
            layout(location = 2) out vec4 f_color;

            layout(set = 0, binding = 0) uniform Uniforms {
                mat4 u_ortho;
                mat4 u_transform;
            };

            layout(set = 1, binding = 0) uniform Locals {
                mat4 l_transform;
            };

            void main() {
                f_pos = v_pos;
                f_tex_coord = v_tex_coord;
                f_color = v_color;

                gl_Position = u_ortho * u_transform * l_transform * vec4(v_pos, 0.0, 1.0);
            }
            ",
            "vertex.vert",
            shaderc::ShaderKind::Vertex,
            &mut compiler,
            swap_chain_desc.format,
            &device,
        )?;

        let fill_pipeline = create_pipeline(
            &vertex_shader,
            &load_shader(
                "
                #version 430

                layout(location = 2) in vec4 f_color;

                layout(location = 0) out vec4 o_color;

                void main() {
                    o_color = f_color;
                }
                ",
                "fill.frag",
                shaderc::ShaderKind::Fragment,
                &mut compiler,
                swap_chain_desc.format,
                &device,
            )?,
            &[&global_bind_group_layout, &locals_bind_group_layout],
            &device,
            swap_chain_desc.format,
        );

        let linear_gradient_pipeline = create_pipeline(
            &vertex_shader,
            &load_shader(
                "
                #version 430

                layout(location = 0) in vec2 f_pos;

                layout(location = 0) out vec4 o_color;

                layout(std140) struct Stop {
                    vec4 pos; // not actually a vec4, more like f32 + 16 byte alignment.
                    vec4 color;
                };

                layout(set = 2, binding = 0) uniform Gradient {
                    vec2 start;
                    vec2 end;
                    Stop stops[32];
                    uint n_stops;
                };

                void main() {
                    // taken from https://github.com/Rokotyan/Linear-Gradient-Shader

                    float alpha = atan(-end.y + start.y, end.x - start.x);
                    float start_rot_x = start.x * cos(alpha) - start.y * sin(alpha);
                    float end_rot_x = end.x * cos(alpha) - end.y * sin(alpha);
                    float d = end_rot_x - start_rot_x;
                    float x_rot = f_pos.x * cos(alpha) - f_pos.y * sin(alpha);
                    o_color = mix(
                        stops[0].color,
                        stops[1].color,
                        smoothstep(
                            start_rot_x + stops[0].pos.x * d,
                            start_rot_x + stops[1].pos.x * d,
                            x_rot
                        )
                    );
                    for (int i = 1; i < n_stops - 1; ++i) {
                        o_color = mix(
                            o_color,
                            stops[i + 1].color,
                            smoothstep(
                                start_rot_x + stops[i].pos.x * d,
                                start_rot_x + stops[i + 1].pos.x * d,
                                x_rot
                            )
                        );
                    }
                }
                ",
                "linear_gradient.frag",
                shaderc::ShaderKind::Fragment,
                &mut compiler,
                swap_chain_desc.format,
                &device,
            )?,
            &[
                &global_bind_group_layout,
                &locals_bind_group_layout,
                &gradient_bind_group_layout,
            ],
            &device,
            swap_chain_desc.format,
        );

        let radial_gradient_pipeline = create_pipeline(
            &vertex_shader,
            &load_shader(
                "
                #version 430

                layout(location = 0) in vec2 f_pos;

                layout(location = 0) out vec4 o_color;

                layout(std140) struct Stop {
                    vec4 pos; // not actually a vec4, more like f32 + 16 byte alignment.
                    vec4 color;
                };

                layout(set = 2, binding = 0) uniform Gradient {
                    vec2 center;
                    vec2 radii;
                    Stop stops[32];
                    uint n_stops;
                };

                void main() {
                    // modification of https://github.com/Rokotyan/Linear-Gradient-Shader

                    float ratio = distance(center, f_pos) / distance(center, radii);
                    o_color = mix(
                        stops[0].color,
                        stops[1].color,
                        smoothstep(
                            stops[0].pos.x,
                            stops[1].pos.x,
                            ratio
                        )
                    );
                    for (int i = 1; i < n_stops - 1; ++i) {
                        o_color = mix(
                            o_color,
                            stops[i + 1].color,
                            smoothstep(
                                stops[i].pos.x,
                                stops[i + 1].pos.x,
                                ratio
                            )
                        );
                    }
                }
                ",
                "radial_gradient.frag",
                shaderc::ShaderKind::Fragment,
                &mut compiler,
                swap_chain_desc.format,
                &device,
            )?,
            &[
                &global_bind_group_layout,
                &locals_bind_group_layout,
                &gradient_bind_group_layout,
            ],
            &device,
            swap_chain_desc.format,
        );

        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare_function: wgpu::CompareFunction::Always,
        });

        let image_pipeline = create_pipeline(
            &vertex_shader,
            &load_shader(
                "
                #version 430

                layout(location = 1) in vec2 f_tex_coord;

                layout(location = 0) out vec4 o_color;

                layout(set = 1, binding = 0) uniform texture2D u_texture;
                layout(set = 1, binding = 1) uniform sampler u_sampler;

                void main() {
                    o_color = texture(sampler2D(u_texture, u_sampler), f_tex_coord);
                }
                ",
                "image.frag",
                shaderc::ShaderKind::Fragment,
                &mut compiler,
                swap_chain_desc.format,
                &device,
            )?,
            &[
                &global_bind_group_layout,
                &locals_bind_group_layout,
                &image_bind_group_layout,
            ],
            &device,
            swap_chain_desc.format,
        );

        Ok(GpuGraphicsDisplay {
            command_groups: linked_hash_map::LinkedHashMap::new(),
            next_command_group_id: 0,
            resources: HashMap::new(),
            next_resource_id: 0,

            globals,
            globals_buffer,
            pipelines: PipelineData {
                fill_pipeline,
                linear_gradient_pipeline,
                radial_gradient_pipeline,
                image_pipeline,

                globals_bind_group,
                locals_bind_group_layout,
                image_bind_group_layout,
                gradient_bind_group_layout,
                image_sampler,
            },
            device,
            queue,
            swap_chain,
            swap_chain_desc,
            surface,
            msaa_framebuffer,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.swap_chain_desc.width, self.swap_chain_desc.height)
    }
}

impl GraphicsDisplay for GpuGraphicsDisplay {
    fn resize(&mut self, size: (u32, u32)) -> Result<(), Box<dyn std::error::Error>> {
        if size.0 == 0 || size.1 == 0 {
            return Ok(());
        }

        self.swap_chain_desc.width = size.0;
        self.swap_chain_desc.height = size.1;
        self.swap_chain = self
            .device
            .create_swap_chain(&self.surface, &self.swap_chain_desc);
        self.msaa_framebuffer =
            create_msaa_framebuffer(&self.device, &self.swap_chain_desc, SAMPLE_COUNT);

        self.globals.ortho = nalgebra::Matrix4::new_orthographic(
            0.0,
            self.swap_chain_desc.width as _,
            0.0,
            self.swap_chain_desc.height as _,
            -1.0,
            1.0,
        );

        upload_to_buffer(
            &self.device,
            &mut self.queue,
            &[self.globals.clone()],
            &self.globals_buffer,
        );

        Ok(())
    }

    fn new_resource(
        &mut self,
        descriptor: ResourceDescriptor,
    ) -> Result<ResourceReference, error::ResourceError> {
        let read_data = |data: &ResourceData| -> Result<Vec<u8>, error::ResourceError> {
            match data {
                ResourceData::File(path) => Ok(std::fs::read(path)?),
                ResourceData::Data(shared_data) => match shared_data {
                    SharedData::RefCount(ref_data) => Ok((*ref_data).to_vec()),
                    SharedData::Static(static_data) => Ok(static_data.to_vec()),
                },
            }
        };

        let id = self.next_resource_id;

        let (rid, res) = match descriptor {
            ResourceDescriptor::Image(img_data) => (ResourceReference::Image(id), {
                let (bgra_data, width, height) = match img_data {
                    ImageData::Raw(ref raster_data, raster_info) => (
                        match raster_info.format {
                            RasterImageFormat::Rgba8 => {
                                let rgba: image::ImageBuffer<image::Bgra<u8>, Vec<u8>> =
                                    image::RgbaImage::from_vec(
                                        raster_info.size.0,
                                        raster_info.size.1,
                                        read_data(raster_data)?,
                                    )
                                    .ok_or_else(|| error::ResourceError::InvalidData)?
                                    .convert();
                                rgba.into_vec()
                            }
                            RasterImageFormat::Bgra8 => read_data(raster_data)?,
                        },
                        raster_info.size.0,
                        raster_info.size.1,
                    ),
                    ImageData::Encoded(ref encoded_data) => {
                        let img = image::load_from_memory(&read_data(encoded_data)?)
                            .map_err(|_| error::ResourceError::InvalidData)?
                            .to_bgra();
                        let (width, height) = (img.width(), img.height());
                        (img.into_vec(), width, height)
                    }
                };

                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth: 1,
                    },
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    sample_count: 1,
                    usage: wgpu::TextureUsage::SAMPLED,
                    mip_level_count: 1,
                    array_layer_count: 1,
                });

                let temp_buf = self
                    .device
                    .create_buffer_mapped(bgra_data.len(), wgpu::BufferUsage::COPY_SRC)
                    .fill_from_slice(&bgra_data);

                let mut encoder = self
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

                encoder.copy_buffer_to_texture(
                    wgpu::BufferCopyView {
                        buffer: &temp_buf,
                        image_height: height,
                        offset: 0,
                        row_pitch: width * 4,
                    },
                    wgpu::TextureCopyView {
                        texture: &texture,
                        mip_level: 1,
                        origin: wgpu::Origin3d {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        array_layer: 1,
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth: 1,
                    },
                );

                self.queue.submit(&[encoder.finish()]);

                Resource::Image(texture.create_default_view())
            }),
            ResourceDescriptor::Font(ref font_data) => (
                ResourceReference::Font(id),
                Resource::Font(
                    wgpu_glyph::GlyphBrushBuilder::using_font_bytes(
                        wgpu_glyph::SharedBytes::ByArc(read_data(font_data)?.into()),
                    )
                    .build(&mut self.device, self.swap_chain_desc.format),
                ),
            ),
        };

        self.next_resource_id += 1;
        self.resources.insert(id, res);

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
                        let paint = create_render_paint(
                            gd_item,
                            &self.device,
                            &self.pipelines,
                            &self.resources,
                        )
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

        let ubo = self
            .device
            .create_buffer_mapped(1, wgpu::BufferUsage::UNIFORM)
            .fill_from_slice(&[locals]);
        let locals_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.pipelines.locals_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &ubo,
                    range: 0..std::mem::size_of::<Locals>() as _,
                },
            }],
        });

        self.next_command_group_id += 1;
        self.command_groups.insert(
            id,
            CommandGroupData {
                display: commands.to_vec(),
                render,
                bounds: display_list_bounds(commands).map_err(|err| Box::new(err))?,
                vbo: self
                    .device
                    .create_buffer_mapped(vertices.len(), wgpu::BufferUsage::VERTEX)
                    .fill_from_slice(&vertices),
                ibo: self
                    .device
                    .create_buffer_mapped(indices.len(), wgpu::BufferUsage::INDEX)
                    .fill_from_slice(&indices),
                ubo,
                locals_bind_group,
            },
        );

        Ok(CommandGroupHandle::new(id))
    }

    fn get_command_group(&self, handle: CommandGroupHandle) -> Option<&[DisplayCommand]> {
        self.command_groups
            .get(&handle.id())
            .map(|cmdgroup| &cmdgroup.display[..])
    }

    fn modify_command_group(
        &mut self,
        handle: CommandGroupHandle,
        commands: &[DisplayCommand],
        protected: Option<bool>,
    ) {
        //unimplemented!()
    }

    fn maintain_command_group(&mut self, handle: CommandGroupHandle) {
        self.command_groups.get_refresh(&handle.id());
    }

    fn remove_command_group(&mut self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        Some(self.command_groups.remove(&handle.id())?.display)
    }

    fn before_exit(&mut self) {
        self.device.poll(true);
    }

    fn present(&mut self, cull: Option<Rect>) -> Result<(), error::DisplayError> {
        let cmds = self.command_groups.values().filter_map(|cmd_group| {
            if cull
                .map(|cull| cull.intersects(&cmd_group.bounds))
                .unwrap_or(true)
            {
                Some(cmd_group)
            } else {
                None
            }
        });

        let resources = &self.resources;
        let mut save_stack: SaveStack<nalgebra::Matrix4<f32>, wgpu::RenderPass> = SaveStack::new();

        let frame = self.swap_chain.get_next_texture();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &self.msaa_framebuffer,
                    resolve_target: Some(&frame.view),
                    clear_color: wgpu::Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    },
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                }],
                depth_stencil_attachment: None,
            });

            pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);

            for cmd_group in cmds {
                draw_command_group(cmd_group, &mut pass, &self.pipelines, &mut save_stack)?;
            }
        }

        self.queue.submit(&[encoder.finish()]);

        Ok(())
    }
}

fn draw_command_group(
    cmd_group: &CommandGroupData,
    pass: &mut wgpu::RenderPass,
    pipelines: &PipelineData,
    save_stack: &mut SaveStack<nalgebra::Matrix4<f32>, wgpu::RenderPass>,
) -> Result<(), error::DisplayError> {
    pass.set_vertex_buffers(0, &[(&cmd_group.vbo, 0)]);
    pass.set_index_buffer(&cmd_group.ibo, 0);
    pass.set_bind_group(1, &cmd_group.locals_bind_group, &[]);

    for cmd in &cmd_group.render {
        match cmd {
            RenderCommand::DrawMesh { mesh, paint } => {
                match paint {
                    RenderPaint::Fill => pass.set_pipeline(&pipelines.fill_pipeline),
                    RenderPaint::LinearGradient(ref bind_group) => {
                        pass.set_pipeline(&pipelines.linear_gradient_pipeline);
                        pass.set_bind_group(2, bind_group, &[]);
                    }
                    RenderPaint::RadialGradient(ref bind_group) => {
                        pass.set_pipeline(&pipelines.radial_gradient_pipeline);
                        pass.set_bind_group(2, bind_group, &[]);
                    }
                    RenderPaint::Image(ref bind_group) => {
                        pass.set_pipeline(&pipelines.image_pipeline);
                        pass.set_bind_group(2, bind_group, &[]);
                    }
                }

                pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex as _, 0..1);
            }
            _ => {}
        }
    }

    Ok(())
}
