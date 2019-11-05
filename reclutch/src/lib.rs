pub mod display;

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
    pub use crate::event::EventInterface as _;
    pub use crate::event::EventListen as _;
    pub use crate::event::GenericEventInterface as _;
    pub use crate::WidgetChildren as _;
}

use crate::display::{GraphicsDisplay, Rect};

pub trait WidgetChildren {
    fn children(&self) -> Vec<&dyn Widget> {
        Vec::new()
    }

    fn children_mut(&mut self) -> Vec<&mut dyn Widget> {
        Vec::new()
    }
}

pub trait Widget: WidgetChildren {
    fn bounds(&self) -> Rect;

    fn update(&mut self) {}

    fn draw(&mut self, display: &mut dyn GraphicsDisplay);
}
