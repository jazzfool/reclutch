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

pub(crate) fn create_filter_pipeline(
    vs: &wgpu::ShaderModule,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    filter: Filter,
) -> Result<wgpu::RenderPipeline, error::GpuError> {
    let mut compiler = shaderc::Compiler::new().ok_or_else(|| {
        error::GpuError::CompilerError("failed to create shaderc compiler".into())
    })?;

    Ok(create_pipeline(
        vs,
        &load_shader(
            match filter {
                Filter::Blur(sigma_x, sigma_y) => {
                    "
                    #version 430

                    layout(location = 1) in vec2 f_tex_coord;

                    layout(location = 0) out vec4 o_color;

                    layout(set = 2, binding = 0) uniform texture2D u_texture;
                    layout(set = 2, binding = 1) uniform sampler u_sampler;

                    void main() {
                        const float offset[3] = float[](0.0, 1.3846153846, 3.2307692308);
                        const float weight[3] = float[](0.2270270270, 0.3162162162, 0.0702702703);
                    
                        o_color = texture(sampler2D(u_texture, u_sampler), f_tex_coord / 1024.0) * weight[0];

                        for (int i = 1; i < 3; ++i) {
                            o_color = texture(sampler2D(u_texture, u_sampler), (f_tex_coord + vec2(0.0, offset[i])) / 1024.0) * weight[i];
                            o_color = texture(sampler2D(u_texture, u_sampler), (f_tex_coord - vec2(0.0, offset[i])) / 1024.0) * weight[i]; 
                        }
                    }
                "
                }
                Filter::Invert => "",
            },
            match filter {
                Filter::Blur(_, _) => "blur.glsl",
                Filter::Invert => "invert.glsl",
            },
            shaderc::ShaderKind::Fragment,
            &mut compiler,
            format,
            device,
        )?,
        bind_group_layouts,
        device,
        format,
    ))
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
    pub globals_bind_group_layout: wgpu::BindGroupLayout,
    pub locals_bind_group_layout: wgpu::BindGroupLayout,
    pub gradient_bind_group_layout: wgpu::BindGroupLayout,
    pub image_bind_group_layout: wgpu::BindGroupLayout,
    pub image_sampler: wgpu::Sampler,

    pub vertex_shader: wgpu::ShaderModule,
    pub texture_format: wgpu::TextureFormat,
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
