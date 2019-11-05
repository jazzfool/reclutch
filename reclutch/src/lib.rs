pub mod display;

pub use euclid;
pub use font_kit;
pub use palette;

pub use reclutch_event as event;

pub mod prelude {
    pub use crate::event::EventInterface as _;
    pub use crate::event::EventListen as _;
    pub use crate::event::GenericEventInterface as _;
}

use crate::display::{GraphicsDisplay, Rect};

pub trait Widget {
    fn children(&self) -> Vec<&dyn Widget> {
        Vec::new()
    }

    fn children_mut(&mut self) -> Vec<&mut dyn Widget> {
        Vec::new()
    }

    fn bounds(&self) -> Rect;

    fn update(&mut self) {}

    fn draw(&mut self, display: &mut dyn GraphicsDisplay);
}
