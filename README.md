<p align="left">
    <img src=".media/reclutch.png" width="256px"/>
</p>

## Gain control of your UI again

[![Build Status](https://travis-ci.com/jazzfool/reclutch.svg?branch=master)](https://travis-ci.com/jazzfool/reclutch)

## Features

- Barebones (i.e. no widget toolkit or layout library provided).
- [Events and event queues](event/README.md)
- Retained-mode rendering.
- Object-oriented widgets in idiomatic Rust.
- Renderer-agnostic.

There is an (optional) OpenGL Skia implementation for the renderer.

<p align="center">
    <img src=".media/showcase.png" width="90%"/>
</p>

## Example

All rendering details have been excluded for simplicity.

```rust
#[derive(WidgetChildren)]
struct Button {
    pub button_press: RcEventQueue<()>,
    global_listener: RcEventListener<WindowEvent>,
}

impl Button {
    pub fn new(global: &mut RcEventQueue<WindowEvent>) -> Self {
        Button {
            button_press: RcEventQueue::new(),
            global_listener: global.listen(),
        }
    }
}

impl Widget for Button {
    type UpdateAux = ();
    type GraphicalAux = ();
    type DisplayObject = DisplayCommand;

    pub fn bounds(&self) -> Rect { /* --snip-- */ }

    pub fn update(&mut self, _aux: &mut ()) {
        for event in self.global_listener.peek() {
            match event {
                WindowEvent::OnClick(_) => self.button_press.push(()),
                _ => (),
            }
        }
    }

    pub fn draw(&mut self, display: &mut dyn GraphicsDisplay, _aux: &mut ()) { /* --snip-- */ }
}
```

The classic counter example can be found in examples/overview.

---

## Children

Children are stored manually by the implementing widget type.

```rust
#[derive(WidgetChildren)]
struct ExampleWidget {
    #[widget_child]
    child: AnotherWidget,
}
```

Which expands to exactly...

```rust
impl reclutch::widget::WidgetChildren for ExampleWidget {
    fn children(
        &self,
    ) -> Vec<
        &dyn WidgetChildren<
            UpdateAux = Self::UpdateAux,
            GraphicalAux = Self::GraphicalAux,
            DisplayObject = Self::DisplayObject,
        >,
    > {
        vec![&self.child]
    }

    fn children_mut(
        &mut self,
    ) -> Vec<
        &mut dyn WidgetChildren<
            UpdateAux = Self::UpdateAux,
            GraphicalAux = Self::GraphicalAux,
            DisplayObject = Self::DisplayObject,
        >,
    > {
        vec![&mut self.child]
    }
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

You can also create your own `WidgetChildren` type with extra bounds and use it with the same derive functionality:
```rust
trait CustomWidgetChildren: Widget + ThemeableWidget {
    fn children(
        &self,
    ) -> Vec<
        &dyn WidgetChildren<
            UpdateAux = Self::UpdateAux,
            GraphicalAux = Self::GraphicalAux,
            DisplayObject = Self::DisplayObject,
        >,
    > {
        Vec::new()
    }

    // ... children_mut ...
}

impl<T> reclutch::widget::WidgetChildren for WidgetBox<T>
where
    T: CustomWidgetChildren
{
    // delegate to CustomWidgetChildren
}

// then...

#[derive(WidgetChildren)]
#[widget_children_trait(CustomWidgetChildren)]
struct WidgetA {
    #[widget_child]
    child: WidgetB,
}

// and now, assuming for example that ThemeableWidget has a method called
// `change_theme` implemented on both WidgetA and WidgetB,

fn main() {
    let widget_a = WidgetA::new();
    for themeable in widget_a.children_mut() {
        themeable.change_theme(ColorfulTheme::new());
    }
}

// is now possible. Further, any pre-existing code pertaining to `WidgetChildren`
// remains valid.
```

**Note:** `WidgetChildren` requires that `Widget` is implemented.

The derive functionality is a feature, enabled by default.

## Rendering

Rendering is done through "command groups". It's designed in a way that both a retained-mode renderer (e.g. WebRender) and an immediate-mode renderer (Direct2D, Skia, Cairo) can be implemented.

```rust
struct VisualWidget {
    command_group: CommandGroup,
}

impl Widget for VisualWidget {
    // --snip--

    fn update(&mut self, _aux: &mut ()) {
        if self.changed {
            self.command_group.repaint();
        }
    }

    // Draws a nice red rectangle.
    fn draw(&mut self, display: &mut dyn GraphicsDisplay, _aux: &mut ()) {
        // Only pushes/modifies command group if a repaint is needed.
        self.command_group.push(display, &[
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

## Updating

The `update` method on widgets is an opportunity for widgets to update layout, animations, etc. and more importantly handle events that have been emitted since the last `update`.

Widgets have an associated type; `UpdateAux` which allows for a global object to be passed around during updating. This is useful for things like updating a layout.

Here's a simple example;

```rust
type UpdateAux = Globals;

fn update(&mut self, aux: &mut Globals) {
    if aux.layout.node_is_dirty(self.layout_node) {
        self.bounds = aux.layout.get_node(self.layout_node);
        self.command_group.repaint();
    }

    self.update_animations(aux.delta_time());

    // propagation is done manually
    for child in self.children_mut() {
        child.update(aux);
    }

    // if your UI doesn't update constantly, then you must check child events *after* propagation,
    // but if it does update constantly, then it's more of a micro-optimization, since any missed events
    // will come back around next update.
    for press_event in self.button_press_listener.peek() {
        self.on_button_press(press_event);
    }
}
```
