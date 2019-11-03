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
