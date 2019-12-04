mod cmd;
mod pipe;

use {
    super::*,
    cmd::*,
    image::ConvertBuffer,
    lyon::tessellation::{self as tess, basic_shapes as shapes},
    pipe::*,
    raw_window_handle::HasRawWindowHandle,
    std::{collections::HashMap, error::Error},
};

pub(crate) mod common {
    pub use {
        super::{cmd::*, pipe::*, *},
        crate::error,
        lyon::tessellation as tess,
    };
}

pub(crate) const SAMPLE_COUNT: u32 = 8;

pub(crate) fn copy_to_array<T: Sized + Default + Copy>(src: &[T]) -> [T; 32] {
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

pub(crate) fn color_to_bgra(color: Color) -> [f32; 4] {
    [color.blue, color.green, color.red, color.alpha]
}

pub(crate) fn color_to_rgba(color: Color) -> [f32; 4] {
    [color.red, color.green, color.blue, color.alpha]
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[repr(align(16))]
pub(crate) struct Vertex {
    pos: [f32; 2],
    tex_coord: [f32; 2],
    color: [f32; 4],
}

pub(crate) struct VertexCtor(Color, Option<Rect>);

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

pub struct GpuGraphicsDisplay {
    command_groups: linked_hash_map::LinkedHashMap<u64, CommandGroupData>,
    next_command_group_id: u64,
    resources: ResourceMap,
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

                layout(set = 2, binding = 0) uniform texture2D u_texture;
                layout(set = 2, binding = 1) uniform sampler u_sampler;

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
                    usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
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
                        mip_level: 0,
                        array_layer: 0,
                        origin: wgpu::Origin3d {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
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

        self.next_command_group_id += 1;
        self.command_groups.insert(
            id,
            CommandGroupData::new(
                commands,
                &self.device,
                &self.pipelines,
                &self.resources,
                protected,
            )?,
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
