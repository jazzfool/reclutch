/*! Reclutch UI Core

Reclutch is a barebones foundation to build a UI from, with a strong focus
on control.

# `Widgets`

A widget only defines 3 methods; [`bounds`](widget/trait.Widget.html#tymethod.bounds),
[`update`](widget/trait.Widget.html#tymethod.update), and [`draw`](widget/trait.Widget.html#tymethod.draw).
It also defines 3 associated types (`UpdateAux`, `GraphicalAux` and `DisplayObject`), discussed in relevant documentation.

When implementing these methods, child widgets must be considered. Therefore
it is advisable to propagate them;
```ignore
for child in self.children_mut() {
    child.update(aux);
    // or:
    child.draw(display);
}
```
The above example involves the `WidgetChildren` trait.

# `WidgetChildren`

`WidgetChildren` is a supertrait which defines an interface to collate all the
child widgets from fields into a single `Vec`.

Most of the time you don't want to implement `WidgetChildren` manually, instead
you can use the provided `derive` crate to reduce it to a couple extra lines;
```ignore
#[derive(WidgetChildren)]
struct CounterWidget {
    // --snip--

    #[widget_child]
    count_label: LabelWidget,
    #[widget_child]
    count_up: ButtonWidget,
    #[widget_child]
    count_down: ButtonWidget,
}
```
This will resolve down to the following code:
```ignore
impl reclutch::widget::WidgetChildren for CounterWidget {
    fn children(
        &self
    ) -> Vec<
        &dyn reclutch::widget::WidgetChildren<
            UpdateAux = Self::UpdateAux,
            GraphicalAux = Self::GraphicalAux,
            DisplayObject = Self::DisplayObject,
        >
    > {
        vec![&self.count_label, &self.count_up, &self.count_down]
    }

    fn children_mut(
        &mut self
    ) -> Vec<
        &dyn reclutch::widget::WidgetChildren<
            UpdateAux = Self::UpdateAux,
            GraphicalAux = Self::GraphicalAux,
            DisplayObject = Self::DisplayObject,
        >
    > {
        vec![&mut self.count_label, &mut self.count_up, &mut self.count_down]
    }
}
```

It should be noted that `Widget` and `WidgetChildren` aren't in a stable state.

When/if trait specialization become stabilized, `WidgetChildren` can be merged into
`Widget`. Further, when/if associated type defaults become stabilized, the `Widget`
associated types will have defaults.
**/

pub mod display;
pub mod error;

pub use euclid;
pub use font_kit;
pub use palette;

pub use reclutch_event as event;

#[cfg(feature = "reclutch_derive")]
#[allow(unused_imports)]
#[macro_use]
extern crate reclutch_derive;

#[cfg(feature = "reclutch_derive")]
pub use reclutch_derive::WidgetChildren;

pub mod prelude {
    pub use crate::{
        display::GraphicsDisplay,
        widget::{Widget, WidgetChildren},
    };
    pub use reclutch_event::prelude::*;
}

pub mod widget {
    use crate::display::{GraphicsDisplay, Rect};

    /// Simple widget trait with a render boundary, event updating and rendering.
    pub trait Widget {
        type UpdateAux;
        type GraphicalAux;
        type DisplayObject;

        /// The bounds method doesn't necessarily have an internal need within Reclutch,
        /// however widget boundaries is crucial data in every GUI, for things such as
        /// layout, partial redraw, and input.
        fn bounds(&self) -> Rect {
            Rect::default()
        }

        /// Perhaps the most important method, this method gives every widget an opportunity
        /// to process events, emit events and execute all the side effects attached to such.
        /// Event handling is performed through a focused event system (see the event module).
        ///
        /// This is also where the [`UpdateAux`](trait.Widget.html#associatedtype.UpdateAux) associated type comes in.
        /// It allows you to pass mutable data around during updating.
        ///
        /// Here's an example implementation of `update`:
        /// ```ignore
        /// #[derive(WidgetChildren)]
        /// struct Counter { /* fields omitted */ }
        ///
        /// impl Widget for Counter {
        ///     type UpdateAux = GlobalData;
        ///     type GraphicalAux = /* ... */;
        ///     type DisplayObject = /* ... */;
        ///
        ///     fn update(&mut self, aux: &mut GlobalData) {
        ///         // propagate to children
        ///         propagate_update(self, aux);
        ///
        ///         for event in self.count_up_listener.peek() {
        ///             self.count += 1;
        ///             self.command_group.repaint();
        ///         }
        ///
        ///         for event in self.count_down_listener.peek() {
        ///             self.count -= 1;
        ///             self.command_group.repaint();
        ///         }
        ///     }
        ///
        ///     // --snip--
        /// }
        /// ```
        fn update(&mut self, _aux: &mut Self::UpdateAux) {}

        /// Drawing is renderer-agnostic, however this doesn't mean the API is restrictive.
        /// Generally, drawing is performed through [`CommandGroup`](../display/struct.CommandGroup.html).
        /// This is also where [`GraphicalAux`](trait.Widget.html#associatedtype.GraphicalAux) and [`DisplayObject`](trait.Widget.html#associatedtype.DisplayObject) come in handy.
        ///
        /// `GraphicalAux` allows you to pass extra data around while rendering,
        /// much like `UpdateAux`. A use case of this could be, for example,
        /// rendering widgets into smaller displays and compositing them into a
        /// larger display by attaching the larger display as `GraphicalAux`.
        ///
        /// `DisplayObject` is simply the type that is used for `GraphicsDisplay`
        /// (i.e. it's the form in which the widget visually expresses itself).
        /// If you're doing regular graphical rendering, then it is strongly
        /// advised to use `DisplayCommand`, which is the type supported by the
        /// default rendering back-ends. For more information, see [`GraphicsDisplay`](../display/trait.GraphicsDisplay.html).
        ///
        /// A simple example of this can be seen below:
        /// ```ignore
        /// struct MyWidget {
        ///     cmd_group: CommandGroup,
        /// }
        ///
        /// impl Widget for MyWidget {
        ///     type GraphicalAux = ();
        ///     type DisplayObject = DisplayCommand;
        ///
        ///     // --snip--
        ///
        ///     fn draw(&mut self, display: &mut dyn GraphicsDisplay, _aux: &mut ()) {
        ///         // note that the builder is an optional abstraction which stands in
        ///         // place of creating an array of DisplayCommands by hand, which can be
        ///         // cumbersome.
        ///         let mut builder = DisplayListBuilder::new();
        ///
        ///         // push display items to the builder
        ///
        ///         self.cmd_group.push(display, &builder.build(), None);
        ///     }
        /// }
        /// ```
        /// Notice that although `DisplayObject` is defined as `DisplayCommand`,
        /// it needn't be passed to the `display` parameter's type. This is because
        /// `GraphicsDisplay` defaults the generic to `DisplayCommand` already.
        fn draw(
            &mut self,
            _display: &mut dyn GraphicsDisplay<Self::DisplayObject>,
            _aux: &mut Self::GraphicalAux,
        ) {
        }
    }

    /// Interface to get children of a widget as an array of dynamic widgets.
    ///
    /// Ideally, this wouldn't be implemented directly, but rather with `derive(WidgetChildren)`.
    pub trait WidgetChildren: Widget {
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

        fn children_mut(
            &mut self,
        ) -> Vec<
            &mut dyn WidgetChildren<
                UpdateAux = Self::UpdateAux,
                GraphicalAux = Self::GraphicalAux,
                DisplayObject = Self::DisplayObject,
            >,
        > {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "reclutch_derive")]
    #[test]
    fn test_widget_derive() {
        use crate as reclutch;
        use reclutch::{
            display::{Point, Rect},
            prelude::*,
        };

        #[derive(WidgetChildren)]
        struct ExampleChild(i8);

        impl Widget for ExampleChild {
            type UpdateAux = ();
            type GraphicalAux = ();
            type DisplayObject = ();

            fn bounds(&self) -> Rect {
                Rect::new(Point::new(self.0 as _, 0.0), Default::default())
            }
        }

        #[derive(WidgetChildren)]
        struct Unnamed(
            #[widget_child] ExampleChild,
            #[widget_child] ExampleChild,
            #[vec_widget_child] Vec<ExampleChild>,
        );

        impl Widget for Unnamed {
            type UpdateAux = ();
            type GraphicalAux = ();
            type DisplayObject = ();
        }

        #[derive(WidgetChildren)]
        struct Named {
            #[widget_child]
            a: ExampleChild,
            #[widget_child]
            b: ExampleChild,
            #[vec_widget_child]
            c: Vec<ExampleChild>,
        };

        impl Widget for Named {
            type UpdateAux = ();
            type GraphicalAux = ();
            type DisplayObject = ();
        }

        let mut unnamed = Unnamed(ExampleChild(0), ExampleChild(1), vec![ExampleChild(2)]);
        let mut named = Named { a: ExampleChild(2), b: ExampleChild(3), c: vec![ExampleChild(4)] };

        assert_eq!(unnamed.children()[0].bounds().origin.x, 0.0);
        assert_eq!(unnamed.children_mut()[1].bounds().origin.x, 1.0);
        assert_eq!(unnamed.children()[2].bounds().origin.x, 2.0);

        assert_eq!(named.children_mut()[0].bounds().origin.x, 2.0);
        assert_eq!(named.children()[1].bounds().origin.x, 3.0);
        assert_eq!(named.children_mut()[2].bounds().origin.x, 4.0);
    }
}
