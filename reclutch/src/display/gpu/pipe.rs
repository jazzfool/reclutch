use {super::common::*, std::collections::HashMap, std::error::Error};

pub(crate) fn create_pipeline(
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

pub(crate) fn load_shader(
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

pub(crate) type ResourceMap = HashMap<u64, Resource>;

#[derive(Debug)]
pub(crate) struct PipelineData {
    pub fill_pipeline: wgpu::RenderPipeline,
    pub linear_gradient_pipeline: wgpu::RenderPipeline,
    pub radial_gradient_pipeline: wgpu::RenderPipeline,
    pub image_pipeline: wgpu::RenderPipeline,

    pub globals_bind_group: wgpu::BindGroup,
    pub locals_bind_group_layout: wgpu::BindGroupLayout,
    pub gradient_bind_group_layout: wgpu::BindGroupLayout,
    pub image_bind_group_layout: wgpu::BindGroupLayout,
    pub image_sampler: wgpu::Sampler,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct Uniforms {
    pub ortho: nalgebra::Matrix4<f32>,
    pub transform: nalgebra::Matrix4<f32>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct Locals {
    pub transform: nalgebra::Matrix4<f32>,
}
