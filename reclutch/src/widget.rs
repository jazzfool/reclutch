use crate::display::{GraphicsDisplay, Rect};

pub trait Widget<E> {
    fn children(&self) -> Vec<&dyn Widget<E>> {
        Vec::new()
    }

    fn children_mut(&mut self) -> Vec<&mut dyn Widget<E>> {
        Vec::new()
    }

    fn bounds(&self) -> Rect;

    #[cfg(feature = "event")]
    fn update(&mut self) {}

    #[cfg(not(feature = "event"))]
    fn update(&mut self, _global: &mut super::event::Event<E>) {}

    fn draw(&mut self, display: &mut dyn GraphicsDisplay);
}
