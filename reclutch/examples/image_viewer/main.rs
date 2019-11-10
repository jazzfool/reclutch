#![allow(unused_imports)]

use {
    glutin::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    reclutch::{
        display::{
            self, Color, CommandGroup, DisplayClip, DisplayCommand, DisplayItem, Filter, FontInfo,
            GraphicsDisplay, GraphicsDisplayItem, GraphicsDisplayPaint, GraphicsDisplayStroke,
            Point, Rect, ResourceData, ResourceDescriptor, ResourceReference, Size, StyleColor,
            TextDisplayItem, Vector,
        },
        event::{RcEventListener, RcEventQueue},
        prelude::*,
        Widget, WidgetChildren,
    },
};

struct Globals {
    hidpi_factor: f64,
    cursor: Point,
    size: Size,
}

#[derive(Debug, Clone, Copy)]
enum TitlebarEvent {
    BeginClick(Point),
    Move(Vector),
    EndClick,
}

#[derive(WidgetChildren)]
struct Titlebar {
    pub move_event: RcEventQueue<TitlebarEvent>,
    position: Point,
    cursor_anchor: Option<Point>,
    global_listener: RcEventListener<WindowEvent>,
    command_group: CommandGroup,
    width: f32,
    text: String,
    font: FontInfo,
    font_resource: Option<ResourceReference>,
}

impl Titlebar {
    fn new(
        position: Point,
        width: f32,
        text: String,
        global: &mut RcEventQueue<WindowEvent>,
    ) -> Self {
        Titlebar {
            move_event: RcEventQueue::new(),
            position,
            cursor_anchor: None,
            global_listener: global.listen(),
            command_group: CommandGroup::new(),
            width,
            text,
            font: FontInfo::from_name("Segoe UI", &["SF Display", "Arial"]).unwrap(),
            font_resource: None,
        }
    }

    fn set_position(&mut self, position: Point) {
        self.position = position;
        self.command_group.repaint();
    }
}

impl Widget for Titlebar {
    type Aux = Globals;

    fn bounds(&self) -> Rect {
        Rect::new(self.position, Size::new(self.width, 30.0))
    }

    fn update(&mut self, aux: &mut Globals) {
        for event in self.global_listener.peek() {
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let position = position.to_physical(aux.hidpi_factor);
                    let position = Point::new(position.x as _, position.y as _);

                    if let Some(ref cursor_anchor) = self.cursor_anchor {
                        self.move_event
                            .push(TitlebarEvent::Move(position - *cursor_anchor));
                    }
                }
                WindowEvent::MouseInput {
                    button: glutin::event::MouseButton::Left,
                    state,
                    ..
                } => match state {
                    glutin::event::ElementState::Pressed => {
                        if self.bounds().contains(aux.cursor.clone()) {
                            self.cursor_anchor = Some(aux.cursor.clone());
                            self.move_event
                                .push(TitlebarEvent::BeginClick(aux.cursor.clone()));
                        }
                    }
                    glutin::event::ElementState::Released => {
                        if self.cursor_anchor.is_some() {
                            self.cursor_anchor = None;
                            self.move_event.push(TitlebarEvent::EndClick);
                        }
                    }
                },
                _ => {}
            }
        }
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        if self.font_resource.is_none() {
            self.font_resource = display
                .new_resource(ResourceDescriptor::Font(ResourceData::Data(
                    self.font.data().unwrap(),
                )))
                .ok();
        }

        let bounds = self.bounds();

        self.command_group.push(
            display,
            &[
                DisplayCommand::BackdropFilter(
                    DisplayClip::Rectangle {
                        rect: bounds.clone(),
                        antialias: true,
                    },
                    Filter::Blur(10.0, 10.0),
                ),
                DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::Rectangle {
                    rect: bounds.clone(),
                    paint: GraphicsDisplayPaint::Fill(StyleColor::Color(Color::new(
                        1.0, 1.0, 1.0, 0.7,
                    ))),
                })),
                DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::Line {
                    a: Point::new(bounds.origin.x, bounds.origin.y + bounds.size.height),
                    b: Point::new(
                        bounds.origin.x + bounds.size.width,
                        bounds.origin.y + bounds.size.height,
                    ),
                    stroke: GraphicsDisplayStroke {
                        thickness: 1.0,
                        antialias: false,
                        ..Default::default()
                    },
                })),
                DisplayCommand::Item(DisplayItem::Text(TextDisplayItem {
                    text: self.text.clone(),
                    font: self.font_resource.as_ref().unwrap().clone(),
                    font_info: self.font.clone(),
                    size: 22.0,
                    bottom_left: bounds.origin + Size::new(5.0, 22.0),
                    color: StyleColor::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
                })),
            ],
        );
    }
}

#[derive(WidgetChildren)]
struct Panel {
    #[widget_child]
    titlebar: Titlebar,
    position_anchor: Option<Point>,
    position: Point,
    size: Size,
    global_listener: RcEventListener<WindowEvent>,
    titlebar_move_listener: RcEventListener<TitlebarEvent>,
    command_group: CommandGroup,
    image: Option<ResourceReference>,
}

impl Panel {
    fn new(position: Point, size: Size, global: &mut RcEventQueue<WindowEvent>) -> Self {
        let titlebar = Titlebar::new(
            position.clone(),
            size.width - 1.0,
            "Reclutch Logo".into(),
            global,
        );
        let titlebar_move_listener = titlebar.move_event.listen();

        Panel {
            titlebar,
            position_anchor: None,
            position,
            size,
            global_listener: global.listen(),
            titlebar_move_listener,
            command_group: CommandGroup::new(),
            image: None,
        }
    }

    fn fit_in_window(&mut self, size: &Size) {
        let window_rect = Rect::new(Point::default(), size.clone());
        let bounds = self.bounds();

        let vert = if bounds.min_y() < window_rect.min_y() {
            window_rect.min_y() - bounds.min_y()
        } else if bounds.max_y() > window_rect.max_y() {
            window_rect.max_y() - bounds.max_y()
        } else {
            0.0
        };

        let horiz = if bounds.min_x() < window_rect.min_x() {
            window_rect.min_x() - bounds.min_x()
        } else if bounds.max_x() > window_rect.max_x() {
            window_rect.max_x() - bounds.max_x()
        } else {
            0.0
        };

        self.position += Vector::new(horiz, vert);
    }
}

impl Widget for Panel {
    type Aux = Globals;

    fn bounds(&self) -> Rect {
        Rect::new(self.position, self.size)
    }

    fn update(&mut self, aux: &mut Globals) {
        for child in self.children_mut() {
            child.update(aux);
        }

        for event in self.titlebar_move_listener.peek() {
            match event {
                TitlebarEvent::BeginClick(_) => {
                    self.position_anchor = Some(self.position);
                }
                TitlebarEvent::Move(delta) => {
                    if let Some(position_anchor) = self.position_anchor {
                        self.position = position_anchor + delta;

                        self.fit_in_window(&aux.size);

                        self.titlebar.set_position(self.position.clone());
                        self.command_group.repaint();
                    }
                }
                TitlebarEvent::EndClick => {
                    self.position_anchor = None;
                }
            }
        }

        for event in self.global_listener.peek() {
            match event {
                WindowEvent::Resized(_) => {
                    self.fit_in_window(&aux.size);

                    self.titlebar.set_position(self.position.clone());
                    self.command_group.repaint();
                }
                _ => (),
            }
        }
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        if self.image.is_none() {
            self.image = display
                .new_resource(ResourceDescriptor::Image(ResourceData::File(
                    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), file!()))
                        .parent()
                        .unwrap()
                        .join("../../../.media/reclutch.png")
                        .canonicalize()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .into(),
                )))
                .ok();
        }

        let bounds = self.bounds();

        self.command_group.push(
            display,
            &[
                DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::Rectangle {
                    rect: bounds.clone(),
                    paint: GraphicsDisplayPaint::Fill(StyleColor::Color(Color::new(
                        0.9, 0.9, 0.9, 1.0,
                    ))),
                })),
                DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::Image {
                    src: None,
                    dst: bounds.clone(),
                    resource: self.image.clone().unwrap(),
                })),
                DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::Rectangle {
                    rect: bounds.inflate(0.0, 1.0).round_out(),
                    paint: GraphicsDisplayPaint::Stroke(GraphicsDisplayStroke {
                        color: StyleColor::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
                        thickness: 1.0,
                        antialias: false,
                        ..Default::default()
                    }),
                })),
            ],
        );

        for child in self.children_mut() {
            child.draw(display);
        }
    }
}

#[cfg(feature = "skia")]
fn main() {
    let window_size = (500u32, 500u32);

    let event_loop = EventLoop::new();

    let wb = glutin::window::WindowBuilder::new()
        .with_title("Image Viewer with Reclutch")
        .with_inner_size(
            glutin::dpi::PhysicalSize::new(window_size.0 as _, window_size.1 as _)
                .to_logical(event_loop.primary_monitor().hidpi_factor()),
        )
        .with_min_inner_size(
            glutin::dpi::PhysicalSize::new(400.0, 200.0)
                .to_logical(event_loop.primary_monitor().hidpi_factor()),
        );

    let context = glutin::ContextBuilder::new()
        //.with_vsync(true) // fast dragging motion at the cost of high GPU usage
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

    display
        .push_command_group(&[DisplayCommand::Clear(Color::new(1.0, 1.0, 1.0, 1.0))])
        .unwrap();

    let mut latest_window_size = window_size;
    let mut global_q = RcEventQueue::new();

    let mut globals = Globals {
        hidpi_factor: context.window().hidpi_factor(),
        cursor: Point::default(),
        size: Size::new(window_size.0 as _, window_size.1 as _),
    };

    let mut panel = Panel::new(
        Point::new(20.0, 20.0),
        Size::new(378.4, 100.0),
        &mut global_q,
    );

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { ref event, .. } = event {
            global_q.push((*event).clone());
        }

        match event {
            Event::WindowEvent {
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

                panel.draw(&mut display);
                display.present(None).unwrap();
                context.swap_buffers().unwrap();
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                let position = position.to_physical(globals.hidpi_factor);
                globals.cursor = Point::new(position.x as _, position.y as _);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                let size = size.to_physical(context.window().hidpi_factor());
                latest_window_size = (size.width as _, size.height as _);
                globals.size.width = size.width as _;
                globals.size.height = size.height as _;
            }
            _ => return,
        }

        panel.update(&mut globals);
        context.window().request_redraw();
    });
}

#[cfg(not(feature = "skia"))]
fn main() {
    compile_error!("this example requires the Skia backend")
}
