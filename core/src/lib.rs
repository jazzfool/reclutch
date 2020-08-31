//! Core components of Reclutch, such as the Widget types and the display module.

pub mod display;
pub mod error;

pub use euclid;
pub use font_kit;
pub use palette;
pub use skia_safe as skia;

/// Intricate event queues.
pub use reclutch_event as event;

pub mod prelude {
    pub use crate::{
        display::GraphicsDisplay,
        widget::{Widget, WidgetChildren},
    };
    pub use reclutch_event::prelude::*;
}

/// Widget systems in which Reclutch is built around.
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
        /// This is also where the [`UpdateAux`] associated type comes in.
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
        ///
        /// [`UpdateAux`]: Widget::UpdateAux
        fn update(&mut self, _aux: &mut Self::UpdateAux) {}

        /// Drawing is renderer-agnostic, however this doesn't mean the API is restrictive.
        /// Generally, drawing is performed through [`CommandGroup`].
        /// This is also where [`GraphicalAux`] and [`DisplayObject`] come in handy.
        ///
        /// [`GraphicalAux`] allows you to pass extra data around while rendering,
        /// much like [`UpdateAux`]. A use case of this could be, for example,
        /// rendering widgets into smaller displays and compositing them into a
        /// larger display by attaching the larger display as [`GraphicalAux`].
        ///
        /// [`DisplayObject`] is simply the type that is used for [`GraphicsDisplay`]
        /// (i.e. it's the form in which the widget visually expresses itself).
        /// If you're doing regular graphical rendering, then it is strongly
        /// advised to use [`DisplayCommand`], which is the type supported by the
        /// default rendering back-ends. For more information, see [`GraphicsDisplay`].
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
        /// Notice that although [`DisplayObject`] is defined as [`DisplayCommand`],
        /// it needn't be passed to the `display` parameter's type. This is because
        /// [`GraphicsDisplay`] defaults the generic to [`DisplayCommand`] already.
        ///
        /// [`CommandGroup`]: crate::display::CommandGroup
        /// [`GraphicalAux`]: Widget::GraphicalAux
        /// [`DisplayObject`]: Widget::DisplayObject
        /// [`UpdateAux`]: Widget::UpdateAux
        /// [`DisplayCommand`]: crate::display::DisplayCommand
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
        /// Returns all the children as immutable dynamic references.
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

        /// Returns all the children as mutable dynamic references.
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
