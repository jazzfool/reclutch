use {
    glium::glutin::{
        self,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    reclutch::{
        display::{
            self, Color, CommandGroup, DisplayCommand, DisplayListBuilder, Filter, FontInfo,
            GraphicsDisplay, GraphicsDisplayPaint, GraphicsDisplayStroke, ImageData, Point, Rect,
            ResourceData, ResourceDescriptor, ResourceReference, SharedData, Size, TextDisplayItem,
            Vector,
        },
        event::{merge::Merge, RcEventListener, RcEventQueue},
        gl,
        prelude::*,
        WidgetChildren,
    },
};

#[derive(Clone)]
struct ConsumableEvent<T>(std::rc::Rc<std::cell::RefCell<Option<T>>>);

impl<T> ConsumableEvent<T> {
    fn new(val: T) -> Self {
        ConsumableEvent(std::rc::Rc::new(std::cell::RefCell::new(Some(val))))
    }

    fn with<P: FnMut(&T) -> bool>(&self, mut pred: P) -> Option<T> {
        if self.0.borrow().is_some() {
            if pred(self.0.borrow().as_ref().unwrap()) {
                return self.0.replace(None);
            }
        }

        None
    }
}

#[derive(Clone)]
enum GlobalEvent {
    MouseClick(ConsumableEvent<Point>),
    MouseRelease(Point),
    MouseMove(Point),
    WindowResize,
}

struct Globals {
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
    global_listener: RcEventListener<GlobalEvent>,
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
        global: &mut RcEventQueue<GlobalEvent>,
    ) -> Self {
        Titlebar {
            move_event: RcEventQueue::default(),
            position,
            cursor_anchor: None,
            global_listener: global.listen(),
            command_group: CommandGroup::new(),
            width,
            text,
            font: FontInfo::from_name("Segoe UI", &["SF Display", "Arial"], None).unwrap(),
            font_resource: None,
        }
    }

    fn set_position(&mut self, position: Point) {
        self.position = position;
        self.command_group.repaint();
    }
}

impl Widget for Titlebar {
    type UpdateAux = Globals;
    type GraphicalAux = ();
    type DisplayObject = DisplayCommand;

    fn bounds(&self) -> Rect {
        Rect::new(self.position, Size::new(self.width, 30.0))
    }

    fn update(&mut self, _aux: &mut Globals) {
        for event in self.global_listener.peek() {
            match event {
                GlobalEvent::MouseClick(click) => {
                    if let Some(ref position) =
                        click.with(|pos| self.bounds().contains(pos.clone()))
                    {
                        self.cursor_anchor = Some(position.clone());
                        self.move_event.emit_owned(TitlebarEvent::BeginClick(position.clone()));
                    }
                }
                GlobalEvent::MouseRelease(_) => {
                    if self.cursor_anchor.is_some() {
                        self.cursor_anchor = None;
                        self.move_event.emit_owned(TitlebarEvent::EndClick);
                    }
                }
                GlobalEvent::MouseMove(pos) => {
                    if let Some(ref cursor_anchor) = self.cursor_anchor {
                        self.move_event.emit_owned(TitlebarEvent::Move(pos - *cursor_anchor));
                    }
                }
                _ => (),
            }
        }
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay, _aux: &mut ()) {
        if self.font_resource.is_none() {
            self.font_resource = display
                .new_resource(ResourceDescriptor::Font(ResourceData::Data(SharedData::RefCount(
                    std::sync::Arc::new(self.font.data().unwrap()),
                ))))
                .ok();
        }

        let bounds = self.bounds();

        let mut builder = DisplayListBuilder::new();

        builder.push_rectangle_backdrop(bounds, true, Filter::Blur(10.0, 10.0));

        builder.push_rectangle(
            bounds,
            GraphicsDisplayPaint::Fill(Color::new(1.0, 1.0, 1.0, 0.6).into()),
            None,
        );

        builder.push_line(
            Point::new(bounds.origin.x, bounds.origin.y + bounds.size.height),
            Point::new(bounds.origin.x + bounds.size.width, bounds.origin.y + bounds.size.height),
            GraphicsDisplayStroke { thickness: 1.0, antialias: false, ..Default::default() },
            None,
        );

        builder.push_text(
            TextDisplayItem {
                text: self.text.clone().into(),
                font: self.font_resource.as_ref().unwrap().clone(),
                font_info: self.font.clone(),
                size: 22.0,
                bottom_left: bounds.origin + Size::new(5.0, 22.0),
                color: Color::new(0.0, 0.0, 0.0, 1.0).into(),
            },
            None,
        );

        self.command_group.push(display, &builder.build(), Default::default(), None, None).unwrap();
    }
}

#[derive(WidgetChildren)]
struct Panel {
    pub on_click: RcEventQueue<*const Panel>,
    #[widget_child]
    titlebar: Titlebar,
    position_anchor: Option<Point>,
    position: Point,
    size: Size,
    global_listener: RcEventListener<GlobalEvent>,
    titlebar_move_listener: RcEventListener<TitlebarEvent>,
    command_group: CommandGroup,
    image_data: &'static [u8],
    image: Option<ResourceReference>,
}

impl Panel {
    fn new(
        position: Point,
        size: Size,
        text: String,
        image_data: &'static [u8],
        global: &mut RcEventQueue<GlobalEvent>,
    ) -> Self {
        let titlebar = Titlebar::new(position.clone(), size.width - 1.0, text, global);
        let titlebar_move_listener = titlebar.move_event.listen();

        Panel {
            on_click: RcEventQueue::default(),
            titlebar,
            position_anchor: None,
            position,
            size,
            global_listener: global.listen(),
            titlebar_move_listener,
            command_group: CommandGroup::new(),
            image_data,
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
    type UpdateAux = Globals;
    type GraphicalAux = ();
    type DisplayObject = DisplayCommand;

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
                    self.on_click.emit_owned(self as _);
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
                GlobalEvent::MouseClick(click) => {
                    if let Some(_) = click.with(|pos| self.bounds().contains(pos.clone())) {
                        self.on_click.emit_owned(self as _);
                        self.command_group.repaint();
                        self.titlebar.command_group.repaint();
                    }
                }
                GlobalEvent::WindowResize => {
                    self.fit_in_window(&aux.size);

                    self.titlebar.set_position(self.position.clone());
                    self.command_group.repaint();
                }
                _ => (),
            }
        }
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay, aux: &mut ()) {
        if self.image.is_none() {
            self.image = display
                .new_resource(ResourceDescriptor::Image(ImageData::Encoded(ResourceData::Data(
                    SharedData::Static(self.image_data),
                ))))
                .ok();
        }

        let bounds = self.bounds();

        let mut builder = DisplayListBuilder::new();

        builder.push_rectangle_backdrop(bounds, true, Filter::Blur(5.0, 5.0));

        builder.push_rectangle(
            bounds,
            GraphicsDisplayPaint::Fill(Color::new(0.9, 0.9, 0.9, 0.5).into()),
            None,
        );

        builder.push_image(None, bounds, self.image.clone().unwrap(), None);

        builder.push_rectangle(
            bounds.inflate(0.0, 0.5),
            GraphicsDisplayPaint::Stroke(GraphicsDisplayStroke {
                color: Color::new(0.0, 0.0, 0.0, 1.0).into(),
                thickness: 1.0,
                antialias: false,
                ..Default::default()
            }),
            None,
        );

        self.command_group.push(display, &builder.build(), Default::default(), None, None).unwrap();

        for child in self.children_mut() {
            child.draw(display, aux);
        }
    }
}

#[derive(WidgetChildren)]
struct PanelContainer {
    #[vec_widget_child]
    panels: Vec<Panel>,
    listeners: Vec<RcEventListener<*const Panel>>,
}

impl PanelContainer {
    fn new() -> Self {
        PanelContainer { panels: Vec::new(), listeners: Vec::new() }
    }

    fn add_panel(&mut self, panel: Panel) {
        let on_click_listener = panel.on_click.listen();
        self.panels.push(panel);
        self.listeners.push(on_click_listener);
    }
}

impl Widget for PanelContainer {
    type UpdateAux = Globals;
    type GraphicalAux = ();
    type DisplayObject = DisplayCommand;

    fn update(&mut self, globals: &mut Globals) {
        // propagate back to front so that panels rendered front-most get events first.
        for child in self.children_mut().iter_mut().rev() {
            child.update(globals);
        }

        {
            // collect all the panel events into a single vec
            let mut panel_events = Vec::new();
            for listener in &self.listeners {
                listener.extend_other(&mut panel_events);
            }

            for event in panel_events {
                if let Some(panel_idx) = self.panels.iter().position(|p| p as *const Panel == event)
                {
                    let last = self.panels.len() - 1;
                    self.panels.swap(panel_idx, last);
                }
            }
        }
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay, aux: &mut ()) {
        for child in self.children_mut() {
            child.draw(display, aux);
        }
    }
}

fn main() {
    let window_size = (500u32, 500u32);

    let event_loop = EventLoop::new();

    let wb = glutin::window::WindowBuilder::new()
        .with_title("Image Viewer with Reclutch")
        .with_inner_size(glutin::dpi::PhysicalSize::new(window_size.0 as f64, window_size.1 as f64))
        .with_min_inner_size(glutin::dpi::PhysicalSize::new(400.0, 200.0));

    let context = glutin::ContextBuilder::new()
        .with_vsync(true) // fast dragging motion at the cost of high GPU usage
        .build_windowed(wb, &event_loop)
        .unwrap();

    let context = unsafe { context.make_current().unwrap() };

    gl::load_with(|s| context.get_proc_address(s));

    let mut fboid = 0;
    unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid) };

    let mut display = display::skia::SkiaGraphicsDisplay::new_gl_framebuffer(
        |s| context.get_proc_address(s),
        &display::skia::SkiaOpenGlFramebuffer {
            framebuffer_id: fboid as _,
            size: (window_size.0 as _, window_size.1 as _),
        },
    )
    .unwrap();

    display
        .push_command_group(
            &[DisplayCommand::Clear(Color::new(1.0, 1.0, 1.0, 1.0))],
            Default::default(),
            None,
            Some(false),
        )
        .unwrap();

    let mut global_q = RcEventQueue::default();

    let mut globals = Globals {
        cursor: Point::default(),
        size: Size::new(window_size.0 as _, window_size.1 as _),
    };

    let mut panel_container = PanelContainer::new();

    panel_container.add_panel(Panel::new(
        Point::new(10.0, 10.0),
        Size::new(288.0, 180.15),
        "Ferris".into(),
        include_bytes!("ferris.png"),
        &mut global_q,
    ));

    panel_container.add_panel(Panel::new(
        Point::new(30.0, 30.0),
        Size::new(300.0, 200.0),
        "Forest".into(),
        include_bytes!("image.jpg"),
        &mut global_q,
    ));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::RedrawRequested { .. } => {
                panel_container.draw(&mut display, &mut ());
                display.present(None).unwrap();
                context.swap_buffers().unwrap();
            }
            Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                globals.cursor = Point::new(position.x as _, position.y as _);
                global_q.emit_owned(GlobalEvent::MouseMove(globals.cursor.clone()));
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput { button: glutin::event::MouseButton::Left, state, .. },
                ..
            } => match state {
                glutin::event::ElementState::Pressed => {
                    global_q.emit_owned(GlobalEvent::MouseClick(ConsumableEvent::new(
                        globals.cursor.clone(),
                    )));
                }
                glutin::event::ElementState::Released => {
                    global_q.emit_owned(GlobalEvent::MouseRelease(globals.cursor.clone()));
                }
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                display.resize((size.width as _, size.height as _)).unwrap();
                context.resize(size);

                globals.size.width = size.width as _;
                globals.size.height = size.height as _;
                global_q.emit_owned(GlobalEvent::WindowResize);
            }
            _ => return,
        }

        panel_container.update(&mut globals);
        context.window().request_redraw();
    });
}
