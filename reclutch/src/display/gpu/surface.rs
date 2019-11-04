use raw_window_handle::HasRawWindowHandle;

use super::*;

const GLOBALS_SIZE: u32 = std::mem::size_of::<GlobalsUniform>() as u32;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const SAMPLE_COUNT: u32 = 4;

#[allow(dead_code)]
pub(crate) struct GpuSurface {
    _surface: wgpu::Surface,
    device: wgpu::Device,
    globals: GlobalsUniform,
    globals_uniform_buffer: wgpu::Buffer,
    uniform_bind_group: (wgpu::BindGroupLayout, wgpu::BindGroup),
    framebuffer: (wgpu::Texture, wgpu::TextureView),
    swap_chain: wgpu::SwapChain,

    fill_pipeline: GpuFillPipeline,
}

impl GpuSurface {
    pub fn new(size: (u32, u32), window: &impl HasRawWindowHandle) -> Result<Self, failure::Error> {
        let surface = wgpu::Surface::create(window);

        let adapter = wgpu::Adapter::request(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::Default,
            backends: wgpu::BackendBit::PRIMARY,
        })
        .ok_or(GpuApiError)?;

        let (device, _queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            extensions: wgpu::Extensions {
                anisotropic_filtering: false,
            },
            limits: wgpu::Limits::default(),
        });

        let globals = GlobalsUniform {
            ortho: nalgebra::Matrix4::new_orthographic(
                0.0,
                size.0 as f32,
                size.1 as f32,
                0.0,
                0.0,
                1.0,
            ),
            transform: nalgebra::Matrix4::identity(),
        };

        let globals_uniform_buffer = device
            .create_buffer_mapped(1, wgpu::BufferUsage::UNIFORM)
            .fill_from_slice(&[globals.clone()]);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[wgpu::BindGroupLayoutBinding {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &globals_uniform_buffer,
                    range: 0..GLOBALS_SIZE as u64,
                },
            }],
        });

        let fill_pipeline = GpuFillPipeline::new(&device, &bind_group_layout)?;

        let swap_chain_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: COLOR_FORMAT,
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Vsync,
        };

        let framebuffer = create_msaa_framebuffer(&device, &swap_chain_desc, SAMPLE_COUNT);

        let swap_chain = device.create_swap_chain(&surface, &swap_chain_desc);

        Ok(GpuSurface {
            _surface: surface,
            device,
            globals,
            globals_uniform_buffer,
            uniform_bind_group: (bind_group_layout, bind_group),
            framebuffer,
            swap_chain,
            fill_pipeline,
        })
    }
}

fn create_msaa_framebuffer(device: &wgpu::Device, swap_chain_desc: &wgpu::SwapChainDescriptor, sample_count: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let msaa_texture_extent = wgpu::Extent3d {
        width: swap_chain_desc.width,
        height: swap_chain_desc.height,
        depth: 1,
    };

    let msaa_frame_descriptor = &wgpu::TextureDescriptor {
        size: msaa_texture_extent,
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: swap_chain_desc.format,
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
    };

    let texture = device.create_texture(msaa_frame_descriptor);
    let texture_view = texture.create_default_view();

    (texture, texture_view)
}

#[allow(dead_code)]
struct GpuFillPipeline {
    pipeline: wgpu::RenderPipeline,
    vs_module: wgpu::ShaderModule,
    fs_module: wgpu::ShaderModule,
}

impl GpuFillPipeline {
    pub fn new(
        device: &wgpu::Device,
        globals_bind_group: &wgpu::BindGroupLayout,
    ) -> Result<Self, failure::Error> {
        let vs_module = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(
            &include_bytes!("shaders/solid.vert.spv")[..],
        ))?);
        let fs_module = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(
            &include_bytes!("shaders/solid.frag.spv")[..],
        ))?);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[globals_bind_group],
        });

        let depth_stencil_state = Some(wgpu::DepthStencilStateDescriptor {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Greater,
            stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_read_mask: 0,
            stencil_write_mask: 0,
        });

        let render_pipeline_descriptor = wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_module,
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
                format: COLOR_FORMAT,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: depth_stencil_state.clone(),
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttributeDescriptor {
                        offset: 0,
                        format: wgpu::VertexFormat::Float2,
                        shader_location: 0,
                    },
                    wgpu::VertexAttributeDescriptor {
                        offset: 8,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 1,
                    },
                ],
            }],
            sample_count: SAMPLE_COUNT,
            sample_mask: 0,
            alpha_to_coverage_enabled: false,
        };

        let pipeline = device.create_render_pipeline(&render_pipeline_descriptor);

        Ok(GpuFillPipeline {
            pipeline,
            vs_module,
            fs_module,
        })
    }
}
