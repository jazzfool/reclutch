<p align="center">
    <img src=".media/reclutch.png" width="256px"/>
</p>

## <p align="center">Gain control of your UI again</p>

[![Build Status](https://travis-ci.com/jazzfool/reclutch.svg?branch=master)](https://travis-ci.com/jazzfool/reclutch)

## Features

- Barebones (i.e. no widget toolkit or layout library provided).
- [Events and event queues](event/README.md)
- Retained-mode rendering.
- Object-oriented widgets in idiomatic Rust.
- Renderer-agnostic.
- Text utilities (line-wrapping, support for text shaping).

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
    #[vec_widget_child]
    children: Vec<AnotherWidget>,
}
```

Which expands to exactly...

```rust
impl reclutch::widget::WidgetChildren for ExampleWidget {
    fn children(
        &self,
    ) -> Vec<
        &dyn reclutch::widget::WidgetChildren<
            UpdateAux = Self::UpdateAux,
            GraphicalAux = Self::GraphicalAux,
            DisplayObject = Self::DisplayObject,
        >,
    > {
        let mut children = Vec::with_capacity(1 + self.children.len());
        children.push(&self.child as _);
        for child in &self.children {
            children.push(child as _);
        }
        children
    }

    fn children_mut(
        &mut self,
    ) -> Vec<
        &mut dyn reclutch::widget::WidgetChildren<
            UpdateAux = Self::UpdateAux,
            GraphicalAux = Self::GraphicalAux,
            DisplayObject = Self::DisplayObject,
        >,
    > {
        let mut children = Vec::with_capacity(1 + self.children.len());
        children.push(&mut self.child as _);
        for child in &mut self.children {
            children.push(child as _);
        }
        children
    }
}
```

(Note: you can switch out the `reclutch::widget::WidgetChildren`s above with your own trait using `#[widget_children_trait(...)]`)

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
            // This simply sets an internal boolean to "true", so don't be afraid to call it multiple times during updating.
            self.command_group.repaint();
        }
    }

    // Draws a nice red rectangle.
    fn draw(&mut self, display: &mut dyn GraphicsDisplay, _aux: &mut ()) {
        let mut builder = DisplayListBuilder::new();
        builder.push_rectangle(
            Rect::new(Point::new(10.0, 10.0), Size::new(30.0, 50.0)),
            GraphicsDisplayPaint::Fill(Color::new(1.0, 0.0, 0.0, 1.0).into()),
            None);

        // Only pushes/modifies the command group if a repaint is needed.
        self.command_group.push(display, &builder.build(), None, true);

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

## License

Reclutch is licensed under either

- [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0)
- [MIT](http://opensource.org/licenses/MIT)

at your choosing.

This license also applies to all "sub-projects" (`event` and `derive`).
