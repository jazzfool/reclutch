use crate::{
    traits::private::{Listen as _, QueueInterface as _},
    *,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct Queue<T>(Rc<RefCell<RawEventQueue<T>>>);

impl<T> Clone for Queue<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for Queue<T> {
    #[inline]
    fn default() -> Self {
        Self(Rc::new(RefCell::new(RawEventQueue::new())))
    }
}

impl<T> private::QueueInterface<T> for Queue<T> {
    #[inline]
    fn with_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&RawEventQueue<T>) -> R,
    {
        let inner = self.0.borrow();
        f(&inner)
    }

    #[inline]
    fn with_inner_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut RawEventQueue<T>) -> R,
    {
        let mut inner = self.0.borrow_mut();
        f(&mut inner)
    }
}

impl<T> GenericQueueInterface<T> for Queue<T> {
    #[inline]
    fn push(&self, event: T) -> bool {
        self.with_inner_mut(|inner| inner.push(event))
    }

    #[inline]
    fn extend<I>(&self, events: I) -> bool
    where
        I: IntoIterator<Item = T>,
    {
        crate::traits::private::extend(self, events)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.with_inner(|inner| inner.events.is_empty())
    }
}

impl<T> QueueInterface<T> for Queue<T> {
    type Listener = Listener<T>;

    #[inline]
    fn listen(&self) -> Listener<T> {
        Listener::new(self.clone())
    }
}

impl<T> Queue<T> {
    #[cfg(test)]
    #[inline]
    fn event_len(&self) -> usize {
        self.with_inner(|inner| inner.events.len())
    }
}

#[derive(Debug)]
pub struct Listener<T>(ListenerKey, Queue<T>);

impl<T> private::Listen<T> for Listener<T> {
    fn with_inner_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(crate::intern::ListenerKey, &mut RawEventQueue<T>) -> R,
    {
        let mut inner = (self.1).0.borrow_mut();
        Some(f(self.0, &mut inner))
    }
}

impl<T> Drop for Listener<T> {
    fn drop(&mut self) {
        let _ = self.with_inner_mut(|key, ev| ev.remove_listener(key));
    }
}

impl<T> Listener<T> {
    fn new(event: Queue<T>) -> Self {
        let id = event.0.borrow_mut().create_listener();
        Listener(id, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::drop;

    #[test]
    fn test_event_listener() {
        let event = Queue::new();

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
        let event = Queue::new();

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
