use {super::*, raw_window_handle::HasRawWindowHandle};

struct CommandGroupData {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
}

struct Mesh {}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
struct Vertex {
    pos: [f32; 2],
    normal: [f32; 2],
    tex_coord: [f32; 2],
    color: [f32; 4],
}

fn clip_to_gd_item(clip: &DisplayClip) -> GraphicsDisplayItem {}

fn tessellate(gd_item: &GraphicsDisplayItem) -> (Vec<Vertex>, Vec<u16>) {}

pub struct GpuGraphicsDisplay {}

impl GpuGraphicsDisplay {
    pub fn new(loader: GlLoader) -> Self {}
}

impl GraphicsDisplay for GpuGraphicsDisplay {
    fn resize(&mut self, size: (u32, u32)) -> Result<(), Box<dyn std::error::Error>> {}

    fn new_resource(
        &mut self,
        descriptor: ResourceDescriptor,
    ) -> Result<ResourceReference, error::ResourceError> {
    }

    fn remove_resource(&mut self, reference: ResourceReference) {}

    fn push_command_group(
        &mut self,
        commands: &[DisplayCommand],
        protected: Option<bool>,
    ) -> Result<CommandGroupHandle, Box<dyn std::error::Error>> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for cmd in commands {
            match cmd {
                DisplayCommand::Item(item) => match item {
                    DisplayItem::Graphics(gd_item) => {}
                    DisplayItem::Text(td_item) => {}
                },
                DisplayCommand::BackdropFilter(clip, filter) => {}
            }
        }

        let id = self;

        Ok()
    }

    fn get_command_group(&self, handle: CommandGroupHandle) -> Option<&[DisplayCommand]> {}

    fn modify_command_group(
        &mut self,
        handle: CommandGroupHandle,
        commands: &[DisplayCommand],
        protected: Option<bool>,
    ) {
    }

    fn maintain_command_group(&mut self, handle: CommandGroupHandle) {}

    fn remove_command_group(&mut self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>> {}

    fn before_exit(&mut self) {}

    fn present(&mut self, cull: Option<Rect>) -> Result<(), error::DisplayError> {}
}
