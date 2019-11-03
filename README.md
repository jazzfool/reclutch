<p align="left">
    <img src=".media/reclutch.png" width="256px"/>
</p>

## Gain control of your UI again

## Features:
- Barebones (i.e. no widget toolkit or graphics backend provided).
- Retained-mode rendering.
- Object-oriented widgets in idiomatic Rust.
- Renderer-agnostic.

Currently there is no default graphics backend, but there is a GPU implementation planned some day.

## Example
All rendering details have been excluded for simplicity.
```rust
struct Button {
    pub button_press: Event<()>,
    global_listener: EventListener<WindowEvent>,
}

impl Button {
    pub fn new(global: &mut Event<WindowEvent>) -> Self {
        Button {
            button_press: Event::new(),
            global_listener: global.new_listener(),
        }
    }
}

impl Widget for Button {
    pub fn bounds(&self) -> Rect { /* --snip-- */ }

    pub fn update(&mut self) {
        for event in self.global_listener.peek() {
            match event {
                WindowEvent::OnClick(_) => self.button_press.push(()),
                _ => (),
            }
        }
    }

    pub fn draw(&mut self, display: &mut dyn GraphicsDisplay) { /* --snip */ }
}

```

The classic counter example can be found in examples/overview.

## Children
Children are stored manually by the implementing widget type.

```rust
struct ExampleWidget {
    child: AnotherWidget,
}

impl Widget for ExampleWidget {
    // --snip--

    fn children(&self) -> Vec<&dyn Widget<()>> {
        vec![&self.child]
    }

    fn children_mut(&mut self) -> Vec<&mut dyn Widget<()>> {
        vec![&mut self.child]
    }

    // --snip--
}
```

Then all the other functions (`draw`, `update`, maybe even `bounds` for parent clipping) are propagated manually (or your API can have a function which automatically and recursively invokes for both parent and child);

```rust
fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
    // do our own rendering here...

    // ...then propagate to children
    for child in self.children_mut() {
        child.draw(display);
    }
}
```

Perhaps there's room for some macro magic here.

## Rendering
Rendering is done through "command groups". It's designed in a way that both a retained-mode renderer (e.g. WebRender) and an immediate-mode renderer (Direct2D, Skia, Cairo) can be implemented.

```rust
struct VisualWidget {
    command_group: Option<CommandGroupHandle>,
}

impl Widget for VisualWidget {
    // --snip--

    // Draws a nice red rectangle.
    fn draw(&mut self, display: &mut dyn GraphicsDisplay) {
        // If self.command_group is `None` then `display.push_command_group` otherwise `display.modify_command_group`.
        ok_or_push(&mut self.command_group, display, &[
            DisplayCommand::Item(DisplayItem::Graphics(GraphicsDisplayItem::Rectangle {
                rect: Rect::new(Point::new(10.0, 10.0), Size::new(30.0, 50.0)),
                paint: GraphicsDisplayPaint::Fill(StyleColor::Color(Color::new(1.0, 0.0, 0.0, 1.0))),
            })),
        ]);

        draw_children();
    }

    // --snip--
}
```

## Callbacks
There are no closures when it comes to callbacks, as it would be too much work to have it fit in safe Rust (and it definitely wouldn't be ergonomic to use).

Instead, Reclutch uses a simple event queue with listener primitives.

Essentially, the event has a list of all events emitted, stored as a vector, and a list of all listeners, stored as a map. The key of the map is what the listener-facing API stores.
The value of the map is simply an index. This index keeps track of the last time the event queue was peeked for a specific listener.
This also allows for simple cleanup of event data not needed (i.e. event data seen by all the listeners); the cleanup function looks for the lowest listener index and removes everything before it.
Further, it's not a "standalone" event system, it only works in a widget environment (or any environment with persistent updates).
The memory cleanup system resembles an extremely simple garbage collector.

Here's an example of it's usage outside a widget (with manual updating);
```rust
let mut event: Event<i32> = Event::new();

event.push(10); // no listeners, so this event won't be received by anyone.
event.cleanup(); // this removes that "10" we just pushed, because it's not needed by any listeners (because there are no listeners).

let listener = event.new_listener();

event.push(1);
event.push(2);

event.cleanup(); // this doesn't do anything; our listener hasn't seen these events so they aren't cleaned up.

// here is how listeners respond to events.
for num in listener.peek() {
    print!("{} ", num);
} // prints: "1 2 "

event.cleanup(); // this removes the "1" and "2" events we pushed because all the listeners have seen them.

std::mem::drop(listener); // you should do this if you're not using a listener so it doesn't hold back cleanup.
```
