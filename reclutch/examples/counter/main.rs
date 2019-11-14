// The classic counter GUI.

#[macro_use]
extern crate reclutch_derive;

use {
    glutin::{
        event::{Event as WinitEvent, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    reclutch::{
        display::{
            self, Color, CommandGroup, DisplayListBuilder, FontInfo, GraphicsDisplay,
            GraphicsDisplayPaint, Point, Rect, ResourceData, ResourceDescriptor, ResourceReference,
            Size, TextDisplayItem,
        },
        event::{RcEventListener, RcEventQueue},
        prelude::*,
        Widget,
    },
};

#[derive(Debug, Clone, Copy)]
enum GlobalEvent {
    Click(Point),
    MouseMove(Point),
}

#[derive(WidgetChildren)]
struct Counter {
    count: i32,

    #[widget_child]
    button_increase: Button,
    #[widget_child]
    button_decrease: Button,
    button_increase_press_listener: RcEventListener<Point>,
    button_decrease_press_listener: RcEventListener<Point>,
    command_group: CommandGroup,
    font_info: FontInfo,
    font: Option<ResourceReference>,
}

impl Counter {
    pub fn new(global: &mut RcEventQueue<GlobalEvent>) -> Self {
        let button_increase = Button::new(String::from("Count Up"), Point::new(10.0, 40.0), global);
        let button_decrease =
            Button::new(String::from("Count Down"), Point::new(10.0, 100.0), global);
        let button_increase_press_listener = button_increase.press_event.listen();
        let button_decrease_press_listener = button_decrease.press_event.listen();

        Self {
            count: 0,
            button_increase,
            button_decrease,
            button_increase_press_listener,
            button_decrease_press_listener,
            command_group: CommandGroup::new(),
            font_info: FontInfo::from_name("Arial", &["Helvetica", "Segoe UI", "Lucida Grande"])
                .unwrap(),
            font: None,
        }
    }
}

impl Widget for Counter {
    type Aux = ();

    fn bounds(&self) -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0))
    }

    fn update(&mut self, aux: &mut ()) {
        for child in self.children_mut() {
            child.update(aux);
        }

        for _event in self.button_increase_press_listener.peek() {
            self.count += 1;
            self.command_group.repaint();
        }

        for _event in self.button_decrease_press_listener.peek() {
            self.count -= 1;
            self.command_group.repaint();
        }
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        if self.font.is_none() {
            self.font = display
                .new_resource(ResourceDescriptor::Font(ResourceData::Data(
                    self.font_info.data().unwrap(),
                )))
                .ok();
        }

        let bounds = self.bounds();

        let mut builder = DisplayListBuilder::new();

        builder.push_clear(Color::new(1.0, 1.0, 1.0, 1.0));

        builder.push_text(TextDisplayItem {
            text: format!("Count: {}", self.count).into(),
            font: self.font.as_ref().unwrap().clone(),
            font_info: self.font_info.clone(),
            size: 23.0,
            bottom_left: bounds.origin.add_size(&Size::new(10.0, 22.0)),
            color: Color::new(0.0, 0.0, 0.0, 1.0).into(),
        });

        self.command_group.push(display, &builder.build());

        for child in self.children_mut() {
            child.draw(display);
        }
    }
}

#[derive(WidgetChildren)]
struct Button {
    pub press_event: RcEventQueue<Point>,

    pub text: String,
    pub position: Point,

    hover: bool,
    global_listener: RcEventListener<GlobalEvent>,
    command_group: CommandGroup,
    font_info: FontInfo,
    font: Option<ResourceReference>,
}

impl Button {
    pub fn new(text: String, position: Point, global: &mut RcEventQueue<GlobalEvent>) -> Self {
        Self {
            press_event: RcEventQueue::new(),
            text,
            position,
            hover: false,
            global_listener: global.listen(),
            command_group: CommandGroup::new(),
            font_info: FontInfo::from_name("Arial", &["Helvetica", "Segoe UI", "Lucida Grande"])
                .unwrap(),
            font: None,
        }
    }
}

impl Widget for Button {
    type Aux = ();

    fn bounds(&self) -> Rect {
        Rect::new(self.position, Size::new(150.0, 50.0))
    }

    fn update(&mut self, _aux: &mut ()) {
        let bounds = self.bounds();

        for event in self.global_listener.peek() {
            match event {
                GlobalEvent::Click(pt) => {
                    if bounds.contains(pt) {
                        self.press_event.push(pt);
                    }
                }
                GlobalEvent::MouseMove(pt) => {
                    let before = std::mem::replace(&mut self.hover, bounds.contains(pt));
                    if self.hover != before {
                        self.command_group.repaint();
                    }
                }
            }
        }
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        if self.font.is_none() {
            self.font = display
                .new_resource(ResourceDescriptor::Font(ResourceData::Data(
                    self.font_info.data().unwrap(),
                )))
                .ok();
        }

        let bounds = self.bounds();
        let color = if self.hover {
            Color::new(0.25, 0.60, 0.70, 1.0)
        } else {
            Color::new(0.20, 0.55, 0.65, 1.0)
        };

        let mut builder = DisplayListBuilder::new();

        builder.push_round_rectangle(bounds, [10.0; 4], GraphicsDisplayPaint::Fill(color.into()));

        builder.push_text(TextDisplayItem {
            text: self.text.clone().into(),
            font: self.font.as_ref().unwrap().clone(),
            font_info: self.font_info.clone(),
            size: 22.0,
            bottom_left: bounds
                .origin
                .add_size(&Size::new(10.0, bounds.size.height / 2.0)),
            color: Color::new(1.0, 1.0, 1.0, 1.0).into(),
        });

        self.command_group.push(display, &builder.build());
    }
}

fn main() {
    let window_size = (500u32, 500u32);

    let event_loop = EventLoop::new();

    let wb = glutin::window::WindowBuilder::new()
        .with_title("Counter with Reclutch")
        .with_inner_size(
            glutin::dpi::PhysicalSize::new(window_size.0 as _, window_size.1 as _)
                .to_logical(event_loop.primary_monitor().hidpi_factor()),
        );

    let context = glutin::ContextBuilder::new()
        .with_vsync(true)
        .build_windowed(wb, &event_loop)
        .unwrap();

    let context = unsafe { context.make_current().unwrap() };

    gl::load_with(|proc| context.get_proc_address(proc) as _);

    let mut display = display::skia::SkiaGraphicsDisplay::new_gl_framebuffer(
        &display::skia::SkiaOpenGlFramebuffer {
            framebuffer_id: 0,
            size: (window_size.0 as _, window_size.1 as _),
        },
    )
    .unwrap();

    // set up the UI
    let mut window_q = RcEventQueue::new();
    let mut counter = Counter::new(&mut window_q);
    let mut cursor = Point::default();

    let mut latest_window_size = window_size;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            WinitEvent::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                if display.size().0 != latest_window_size.0 as _
                    || display.size().1 != latest_window_size.1 as _
                {
                    display
                        .resize((latest_window_size.0 as _, latest_window_size.1 as _))
                        .unwrap();
                }

                counter.draw(&mut display);
                display.present(None).unwrap();
                context.swap_buffers().unwrap();
            }
            WinitEvent::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                let position = position.to_physical(context.window().hidpi_factor());
                cursor = Point::new(position.x as _, position.y as _);

                window_q.push(GlobalEvent::MouseMove(cursor));
            }
            WinitEvent::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: glutin::event::ElementState::Pressed,
                        button: glutin::event::MouseButton::Left,
                        ..
                    },
                ..
            } => {
                window_q.push(GlobalEvent::Click(cursor));
            }
            WinitEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            WinitEvent::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                let size = size.to_physical(context.window().hidpi_factor());
                latest_window_size = (size.width as _, size.height as _);
            }
            _ => return,
        }

        counter.update(&mut ());
        context.window().request_redraw();
    });
}
