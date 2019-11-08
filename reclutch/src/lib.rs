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
pub use reclutch_derive::*;

pub mod prelude {
    pub use crate::WidgetChildren as _;
    pub use reclutch_event::prelude::*;
}

use crate::display::{GraphicsDisplay, Rect};

/// Interface to get children of a widget as an array of dynamic widgets.
///
/// Ideally, this wouldn't be implemented directly, but rather with `derive(WidgetChildren)`.
pub trait WidgetChildren {
    fn children(&self) -> Vec<&dyn Widget> {
        Vec::new()
    }

    fn children_mut(&mut self) -> Vec<&mut dyn Widget> {
        Vec::new()
    }
}

/// Simple widget trait with a render boundary and event updating, with a generic auxiliary type.
pub trait Widget<Aux = ()>: WidgetChildren {
    fn bounds(&self) -> Rect;

    fn update(&mut self, _aux: &mut Aux) {}

    fn draw(&mut self, display: &mut dyn GraphicsDisplay);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_derive() {
        use crate as reclutch;
        use reclutch::display::Point;

        #[derive(WidgetChildren)]
        struct ExampleChild(i8);

        impl Widget for ExampleChild {
            fn bounds(&self) -> Rect {
                Rect::new(Point::new(self.0 as _, 0.0), Default::default())
            }
            fn draw(&mut self, _: &mut dyn GraphicsDisplay) {}
        }

        #[derive(WidgetChildren)]
        struct Unnamed(#[widget_child] ExampleChild, #[widget_child] ExampleChild);

        #[derive(WidgetChildren)]
        struct Named {
            #[widget_child]
            a: ExampleChild,
            #[widget_child]
            b: ExampleChild,
        };

        let mut unnamed = Unnamed(ExampleChild(0), ExampleChild(1));
        let mut named = Named {
            a: ExampleChild(2),
            b: ExampleChild(3),
        };

        assert_eq!(unnamed.children()[0].bounds().origin.x, 0.0);
        assert_eq!(unnamed.children_mut()[1].bounds().origin.x, 1.0);

        assert_eq!(named.children_mut()[0].bounds().origin.x, 2.0);
        assert_eq!(named.children()[1].bounds().origin.x, 3.0);
    }
}
