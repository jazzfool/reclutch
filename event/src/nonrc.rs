use crate::*;
use std::{cell::RefCell, ops::Deref};

#[derive(Debug)]
pub struct Queue<T>(pub RefCell<RawEventQueue<T>>);

impl<T> Queue<T> {
    #[inline]
    pub fn new() -> Self {
        Queue(Default::default())
    }

    #[inline]
    pub fn listen(&self) -> Listener<'_, T> {
        Listener::new(&self.0)
    }
}

impl<T> Default for Queue<T> {
    #[inline]
    fn default() -> Self {
        Queue(Default::default())
    }
}

impl<T> Deref for Queue<T> {
    type Target = RefCell<RawEventQueue<T>>;

    #[inline]
    fn deref(&self) -> &RefCell<RawEventQueue<T>> {
        &self.0
    }
}

#[derive(Debug)]
pub struct Listener<'parent, T>(ListenerKey, &'parent RefCell<RawEventQueue<T>>);

impl<T> EventListen for Listener<'_, T> {
    type Item = T;

    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Self::Item]) -> R,
    {
        self.1.borrow_mut().pull_with(self.0, f)
    }
}

impl<T> Drop for Listener<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.1.borrow_mut().remove_listener(self.0);
    }
}

impl<'a, T> Listener<'a, T> {
    #[inline]
    pub fn new(parent: &'a RefCell<RawEventQueue<T>>) -> Self {
        Listener(parent.borrow_mut().create_listener(), parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::drop;

    #[test]
    fn test_event_listener() {
        let event = Queue::new();

        event.emit_owned(0i32).to_result().unwrap_err();

        let listener = event.listen();

        event.emit_owned(1i32).to_result().unwrap();
        event.emit_owned(2i32).to_result().unwrap();
        event.emit_owned(3i32).to_result().unwrap();

        assert_eq!(listener.peek(), &[1, 2, 3]);

        drop(listener);
    }

    #[test]
    fn test_event_cleanup() {
        let event = Queue::new();

        let listener_1 = event.listen();

        event.emit_owned(10i32).to_result().unwrap();

        assert_eq!(event.borrow().events.len(), 1);

        let listener_2 = event.listen();

        event.emit_owned(20i32).to_result().unwrap();

        assert_eq!(listener_1.peek(), &[10i32, 20i32]);
        assert_eq!(listener_2.peek(), &[20i32]);
        assert_eq!(listener_2.peek(), &[]);
        assert_eq!(listener_2.peek(), &[]);

        assert_eq!(event.borrow().events.len(), 0);

        for _i in 0..10 {
            event.emit_owned(30i32).to_result().unwrap();
        }

        assert_eq!(listener_2.peek(), &[30i32; 10]);

        drop(listener_1);

        assert_eq!(event.borrow().events.len(), 0);
    }
}
