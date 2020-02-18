use crate::*;
use std::{cell::RefCell, ops::Deref, rc::Rc};

type Intern<T> = Rc<RefCell<RawEventQueue<T>>>;

/// Non-thread-safe queue; a step above [`nonrc`](crate::nonrc).
#[derive(Debug)]
pub struct Queue<T>(pub Intern<T>);

impl<T> Queue<T> {
    #[inline]
    pub fn new() -> Self {
        Queue(Default::default())
    }
}

impl<T> Default for Queue<T> {
    #[inline]
    fn default() -> Self {
        Queue(Default::default())
    }
}

impl<T> Deref for Queue<T> {
    type Target = Intern<T>;

    #[inline]
    fn deref(&self) -> &Intern<T> {
        &self.0
    }
}

impl<T: Clone> QueueInterfaceListable for Intern<T> {
    type Listener = Listener<T>;

    #[inline]
    fn listen(&self) -> Listener<T> {
        Listener::new(self.clone())
    }
}

#[derive(Debug)]
pub struct Listener<T>(ListenerKey, Intern<T>);

impl<T> EventListen for Listener<T> {
    type Item = T;

    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Self::Item]) -> R,
    {
        self.1.borrow_mut().pull_with(self.0, f)
    }
}

impl<T> Drop for Listener<T> {
    fn drop(&mut self) {
        self.1.borrow_mut().remove_listener(self.0)
    }
}

impl<T> Listener<T> {
    fn new(event: Intern<T>) -> Self {
        let id = event.borrow_mut().create_listener();
        Listener(id, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::drop;

    #[test]
    fn test_event_listener() {
        let event = Queue::default();

        event.emit_owned(0i32).into_result().unwrap_err();

        let listener = event.listen();

        event.emit_owned(1i32).into_result().unwrap();
        event.emit_owned(2i32).into_result().unwrap();
        event.emit_owned(3i32).into_result().unwrap();

        assert_eq!(listener.peek(), &[1, 2, 3]);

        drop(listener);
    }

    #[test]
    fn test_event_cleanup() {
        let event = Queue::default();

        let listener_1 = event.listen();

        event.emit_owned(10i32).into_result().unwrap();

        assert_eq!(event.borrow().events.len(), 1);

        let listener_2 = event.listen();

        event.emit_owned(20i32).into_result().unwrap();

        assert_eq!(listener_1.peek(), &[10i32, 20i32]);
        assert_eq!(listener_2.peek(), &[20i32]);
        assert_eq!(listener_2.peek(), &[]);
        assert_eq!(listener_2.peek(), &[]);

        assert_eq!(event.borrow().events.len(), 0);

        for _i in 0..10 {
            event.emit_owned(30i32).into_result().unwrap();
        }

        assert_eq!(listener_2.peek(), &[30i32; 10]);

        drop(listener_1);

        assert_eq!(event.borrow().events.len(), 0);
    }
}
