use std::{cell::RefCell, rc::Rc};

use crate::{intern::EventIntern, traits::private::EventListen as _};
use crate::*;

#[derive(Debug)]
pub struct Event<T>(Rc<RefCell<EventIntern<T>>>);

impl<T> Clone for Event<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for Event<T> {
    #[inline]
    fn default() -> Self {
        Self(Rc::new(RefCell::new(EventIntern::new())))
    }
}

impl<T> private::EventInterface<T> for Event<T> {
    #[inline]
    fn with_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&EventIntern<T>) -> R,
    {
        let inner = self.0.borrow();
        f(&inner)
    }

    #[inline]
    fn with_inner_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut EventIntern<T>) -> R,
    {
        let mut inner = self.0.borrow_mut();
        f(&mut inner)
    }
}

impl<T> EventInterface<T> for Event<T> {
    type Listener = EventListener<T>;

    #[inline]
    fn listen(&self) -> EventListener<T> {
        EventListener::new(self.clone())
    }
}

/// You should wrap this inside of an Rc if you want
/// multiple references to the same listener
#[derive(Debug)]
pub struct EventListener<T>(ListenerKey, Event<T>);

impl<T> private::EventListen<T> for EventListener<T> {
    fn with_inner_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(crate::intern::ListenerKey, &mut EventIntern<T>) -> R
    {
        let mut inner = (self.1).0.borrow_mut();
        Some(f(self.0, &mut inner))
    }
}

impl<T> Drop for EventListener<T> {
    fn drop(&mut self) {
        let _ = self.with_inner_mut(|key, ev| ev.remove_listener(key));
    }
}

impl<T> EventListener<T> {
    fn new(event: Event<T>) -> Self {
        let id = event.0.borrow_mut().create_listener();
        EventListener(id, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::drop;

    #[test]
    fn test_event_listener() {
        let event = Event::new();

        event.push(0i32);

        let listener = event.listen();

        event.push(1i32);
        event.push(2i32);
        event.push(3i32);

        assert_eq!(listener.peek(), &[1, 2, 3]);

        drop(listener);
    }

    #[test]
    fn test_event_cleanup() {
        let event = Event::new();

        let listener_1 = event.listen();

        event.push(10i32);

        assert_eq!(event.event_len(), 1);

        let listener_2 = event.listen();

        event.push(20i32);

        assert_eq!(listener_1.peek(), &[10i32, 20i32]);
        assert_eq!(listener_2.peek(), &[20i32]);
        assert_eq!(listener_2.peek(), &[]);
        assert_eq!(listener_2.peek(), &[]);

        assert_eq!(event.event_len(), 0);

        for _i in 0..10 {
            event.push(30i32);
        }

        assert_eq!(listener_2.peek(), &[30i32; 10]);

        drop(listener_1);

        assert_eq!(event.event_len(), 0);
    }
}
