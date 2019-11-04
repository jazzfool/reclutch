mod surface;
mod tess;

use raw_window_handle::HasRawWindowHandle;

use super::*;

#[derive(Debug, Clone)]
pub struct GpuApiError;

impl std::fmt::Display for GpuApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "An underlying GPU API error occurred")
    }
}

impl std::error::Error for GpuApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

pub struct GpuGraphicsDisplay {
    _surface: surface::GpuSurface,
}

impl GpuGraphicsDisplay {
    pub fn new(size: (u32, u32), window: &impl HasRawWindowHandle) -> Result<Self, failure::Error> {
        Ok(GpuGraphicsDisplay {
            _surface: surface::GpuSurface::new(size, window)?,
        })
    }
}

impl GraphicsDisplay for GpuGraphicsDisplay {
    fn resize(&mut self, _size: (u32, u32)) {
        unimplemented!()
    }

    fn push_command_group(
        &mut self,
        _commands: &[DisplayCommand],
    ) -> Result<CommandGroupHandle, failure::Error> {
        unimplemented!()
    }

    fn get_command_group(&self, _handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        unimplemented!()
    }

    fn modify_command_group(&mut self, _handle: CommandGroupHandle, _commands: &[DisplayCommand]) {
        unimplemented!()
    }

    fn remove_command_group(&mut self, _handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {
        unimplemented!()
    }

    fn before_exit(&mut self) {
        unimplemented!()
    }

    fn present(&mut self, _cull: Option<Rect>) {
        unimplemented!()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GlobalsUniform {
    ortho: nalgebra::Matrix4<f32>,
    transform: nalgebra::Matrix4<f32>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
}
