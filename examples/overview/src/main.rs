// The classic counter GUI.
//
// Be warned: this runs quite slow due to the rendering being CPU based.

pub mod cpu;

use {
    reclutch::{
        display::{
            ok_or_push, Color, CommandGroupHandle, DisplayCommand, DisplayItem, FontInfo,
            GraphicsDisplay, GraphicsDisplayItem, GraphicsDisplayPaint, GraphicsDisplayStroke,
            Point, Rect, Size, StyleColor, TextDisplayItem,
        },
        event::{Event, EventListener},
        widget::Widget,
    },
    winit::{
        event::{Event as WinitEvent, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
};

enum GlobalEvent {
    Click(Point),
    MouseMove(Point),
}

struct Counter {
    count: i32,

    button: Button,
    button_press_listener: EventListener<Point>,
    command_group: Option<CommandGroupHandle>,
}

impl Counter {
    pub fn new(global: &mut Event<GlobalEvent>) -> Self {
        let button = Button::new(global);
        let button_press_listener = button.press_event.new_listener();

        Self {
            count: 0,
            button,
            button_press_listener,
            command_group: None,
        }
    }
}

impl Widget for Counter {
    fn children(&self) -> Vec<&dyn Widget> {
        vec![&self.button]
    }

    fn children_mut(&mut self) -> Vec<&mut dyn Widget> {
        vec![&mut self.button]
    }

    fn bounds(&self) -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0))
    }

    fn update(&mut self) {
        for child in self.children_mut() {
            child.update();
        }

        let count = &mut self.count;
        self.button_press_listener.with(|events| {
            for _event in events {
                *count += 1;
            }
        });
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        let bounds = self.bounds();
        ok_or_push(
            &mut self.command_group,
            display,
            &[DisplayCommand::Item(DisplayItem::Text(TextDisplayItem {
                text: format!("Count: {}", self.count),
                font: FontInfo::new("Arial", &["Helvetica", "Segoe UI", "Lucida Grande"]).unwrap(),
                size: 23.0,
                bottom_left: bounds.origin.add_size(&Size::new(10.0, 22.0)),
                color: StyleColor::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
            }))],
        );

        for child in self.children_mut() {
            child.draw(display);
        }
    }
}

struct Button {
    pub press_event: Event<Point>,

    hover: bool,
    global_listener: EventListener<GlobalEvent>,
    command_group: Option<CommandGroupHandle>,
}

impl Button {
    pub fn new(global: &mut Event<GlobalEvent>) -> Self {
        Self {
            press_event: Event::new(),
            hover: false,
            global_listener: global.new_listener(),
            command_group: None,
        }
    }
}

impl Widget for Button {
    fn bounds(&self) -> Rect {
        Rect::new(Point::new(10.0, 40.0), Size::new(150.0, 50.0))
    }

    fn update(&mut self) {
        let bounds = self.bounds();
        let press_event = &mut self.press_event;

        let hover = &mut self.hover;

        self.global_listener.with(|events| {
            for event in events {
                match event {
                    GlobalEvent::Click(pt) => {
                        if bounds.contains(*pt) {
                            press_event.push(*pt);
                        }
                    }
                    GlobalEvent::MouseMove(pt) => {
                        *hover = bounds.contains(*pt);
                    }
                }
            }
        })
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        let bounds = self.bounds();
        let color = if self.hover {
            Color::new(0.25, 0.60, 0.70, 1.0)
        } else {
            Color::new(0.20, 0.55, 0.65, 1.0)
        };

        ok_or_push(
            &mut self.command_group,
            display,
            &[
                DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::RoundRectangle {
                    rect: bounds,
                    radii: [10.0; 4],
                    paint: GraphicsDisplayPaint::Fill(StyleColor::Color(color)),
                })),
                DisplayCommand::Item(DisplayItem::Text(TextDisplayItem {
                    text: "Count Up".to_owned(),
                    font: FontInfo::new("Arial", &["Helvetica", "Segoe UI", "Lucida Grande"])
                        .unwrap(),
                    size: 22.0,
                    bottom_left: bounds
                        .origin
                        .add_size(&Size::new(10.0, bounds.size.height / 2.0)),
                    color: StyleColor::Color(Color::new(1.0, 1.0, 1.0, 1.0)),
                })),
            ],
        );
    }
}

fn main() -> Result<(), failure::Error> {
    let mut window_size = (500u32, 500u32);

    let event_loop = EventLoop::new();

    let mut display = cpu::CpuGraphicsDisplay::new(window_size, &event_loop)?;

    // set up the UI
    let mut window_q = Event::new();
    let mut counter = Counter::new(&mut window_q);
    let mut cursor = Point::default();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            WinitEvent::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                counter.draw(&mut display);
                display.present(None);
            }
            WinitEvent::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                let position = position.to_physical(display.window.hidpi_factor());
                cursor = Point::new(position.x as _, position.y as _);

                window_q.push(GlobalEvent::MouseMove(cursor));
            }
            WinitEvent::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: winit::event::ElementState::Pressed,
                        button: winit::event::MouseButton::Left,
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
            _ => (),
        }

        counter.update();
        display.window.request_redraw();

        window_q.cleanup();
    });

    Ok(())
}
