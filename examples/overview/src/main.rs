// The classic counter GUI.
//
// Unfortunately there isn't a default graphics backend yet, which means this example has no graphical I/O.

use reclutch::{
    display::{
        ok_or_push, Color, CommandGroupHandle, DisplayCommand, DisplayItem, FontInfo,
        GraphicsDisplay, GraphicsDisplayItem, GraphicsDisplayPaint, Point, Rect, Size, StyleColor,
        TextDisplayItem,
    },
    widget::{Event, EventListener, Widget},
};

enum WindowEvent {
    Click(Point),
}

struct Counter {
    count: i32,

    button: Button,
    button_press_listener: EventListener<Point>,
    global_listener: EventListener<WindowEvent>,
    command_group: Option<CommandGroupHandle>,
}

impl Counter {
    pub fn new(global: &mut Event<WindowEvent>) -> Self {
        let button = Button::new(global);
        let button_press_listener = button.press_event.new_listener();

        Self {
            count: 0,
            button,
            button_press_listener,
            global_listener: global.new_listener(),
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
        self.global_listener.with(|events| {
            for event in events {
                match event {
                    WindowEvent::Click(ref pt) => {
                        println!("Counter clicked at: {:?}", pt);
                    }
                }
            }
        });

        for child in self.children_mut() {
            child.update();
        }

        let count = &mut self.count;
        self.button_press_listener.with(|events| {
            for _event in events {
                *count += 1;
                println!("Counter increased: {}.", count);
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
                size: 12.0,
                bottom_left: bounds.origin.add_size(&Size::new(0.0, 12.0)),
                color: StyleColor::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
            }))],
        );
    }
}

struct Button {
    pub press_event: Event<Point>,

    global_listener: EventListener<WindowEvent>,
    command_group: Option<CommandGroupHandle>,
}

impl Button {
    pub fn new(global: &mut Event<WindowEvent>) -> Self {
        Self {
            press_event: Event::new(),
            global_listener: global.new_listener(),
            command_group: None,
        }
    }
}

impl Widget for Button {
    fn bounds(&self) -> Rect {
        Rect::new(Point::new(10.0, 10.0), Size::new(50.0, 20.0))
    }

    fn update(&mut self) {
        let bounds = self.bounds();
        let press_event = &mut self.press_event;
        self.global_listener.with(|events| {
            for event in events {
                match event {
                    WindowEvent::Click(pt) => {
                        if bounds.contains(*pt) {
                            press_event.push(*pt);
                            println!("Button clicked at: {:?}", pt);
                        }
                    }
                }
            }
        })
    }

    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        let bounds = self.bounds();
        ok_or_push(
            &mut self.command_group,
            display,
            &[
                DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::RoundRectangle {
                    rect: bounds,
                    radii: [10.0; 4],
                    paint: GraphicsDisplayPaint::Fill(StyleColor::Color(Color::new(
                        0.20, 0.55, 0.65, 1.0,
                    ))),
                })),
                DisplayCommand::Item(DisplayItem::Text(TextDisplayItem {
                    text: "Count Up".to_owned(),
                    font: FontInfo::new("Arial", &["Helvetica", "Segoe UI", "Lucida Grande"])
                        .unwrap(),
                    size: 12.0,
                    bottom_left: bounds
                        .origin
                        .add_size(&Size::new(10.0, bounds.size.height / 2.0)),
                    color: StyleColor::Color(Color::new(1.0, 1.0, 1.0, 1.0)),
                })),
            ],
        );
    }
}

fn main() {
    let mut window = Event::new();

    let mut counter = Counter::new(&mut window);

    counter.update();

    window.push(WindowEvent::Click(Point::new(-23.0, 14.0)));

    counter.update();

    window.push(WindowEvent::Click(Point::new(20.0, 11.0)));
    window.push(WindowEvent::Click(Point::new(11.0, 11.0)));

    counter.update();
}
