/*! Reclutch UI Core

Reclutch is a barebones foundation to build a UI from, with a strong focus
on control.

# `Widget`

A widget only defines 3 methods; [`bounds`], [`update`], and [`draw`].
It also defines 3 associated types ([`UpdateAux`], [`GraphicalAux`] and [`DisplayObject`]), discussed in relevant documentation.

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

[`WidgetChildren`] is a supertrait which defines an interface to collate all the
child widgets from fields into a single [`Vec`].

Most of the time you don't want to implement [`WidgetChildren`] manually, instead
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

[`bounds`]: widget::Widget::bounds
[`update`]: widget::Widget::update
[`draw`]: widget::Widget::draw
[`UpdateAux`]: widget::Widget::UpdateAux
[`GraphicalAux`]: widget::Widget::GraphicalAux
[`DisplayObject`]: widget::Widget::DisplayObject
[`WidgetChildren`]: widget::WidgetChildren
**/

#[cfg(feature = "reclutch_derive")]
#[allow(unused_imports)]
#[macro_use]
extern crate reclutch_derive;

#[cfg(feature = "reclutch_derive")]
pub use reclutch_derive::{Event, OperatesVerbGraph, WidgetChildren};

pub use reclutch_verbgraph as verbgraph;

pub use reclutch_core::*;

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
