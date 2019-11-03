use crate::display::{GraphicsDisplay, Rect};
use std::{
    collections::hash_map,
    fmt,
    sync::{Arc, Mutex},
};

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

struct EventIntern<T> {
    listeners: hash_map::HashMap<usize, usize>,
    next_listener_id: usize,
    events: Vec<T>,
}

impl<T> EventIntern<T> {
    /// Removes all events that have been already seen by all listeners.
    ///
    /// Call this ocassionally to free up memory
    pub fn cleanup(&mut self) {
        if !self.listeners.is_empty() {
            let min_idx = *self.listeners.values().min().unwrap();

            if min_idx != 0 {
                self.events.drain(0..min_idx);

                for idx in self.listeners.values_mut() {
                    *idx -= min_idx;
                }
            }
        }
    }
}

pub struct Event<T>(Arc<Mutex<EventIntern<T>>>);

impl<T> Clone for Event<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for Event<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Event<T> {
    /// Creates a new event
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(EventIntern {
            listeners: Default::default(),
            next_listener_id: 0,
            events: Vec::new(),
        })))
    }

    /// Pushes/emits an event
    pub fn push(&mut self, event: T) {
        (*self.0.lock().unwrap()).events.push(event);
    }

    /// Returns a handle to a new listener
    pub fn new_listener(&self) -> EventListener<T> {
        EventListener::new(Event(Arc::clone(&self.0)))
    }

    /// Removes all events that have been already seen by all listeners.
    ///
    /// Call this ocassionally to free up memory
    pub fn cleanup(&mut self) {
        (*self.0.lock().unwrap()).cleanup();
    }

    /// Returns the number of listeners
    pub fn listener_len(&self) -> usize {
        self.0.lock().unwrap().listeners.len()
    }

    /// Returns the number of even
    pub fn event_len(&self) -> usize {
        self.0.lock().unwrap().events.len()
    }
}

/// You should wrap this inside of an Rc or Arc if you want
/// multiple references to the same listener
pub struct EventListener<T>(usize, Event<T>);

impl<T> fmt::Debug for EventListener<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EventListener({}, _Event)", self.0)
    }
}

impl<T> Drop for EventListener<T> {
    fn drop(&mut self) {
        if let Ok(ref mut inner) = (self.1).0.lock() {
            inner.listeners.remove(&self.0);
            inner.cleanup();
        }
    }
}

impl<T> EventListener<T> {
    fn new(event: Event<T>) -> Self {
        let id = {
            let mut inner = event.0.lock().unwrap();
            let id = inner.next_listener_id;
            let maxidx = inner.events.len();
            inner.next_listener_id += 1;
            inner.listeners.insert(id, maxidx);
            id
        };
        EventListener(id, event)
    }

    /// Applies a function to the list of new events since last `peek`
    /// without cloning T
    ///
    /// This function is faster than calling peek() and iterating over the result.
    ///
    /// It holds a lock on the event while called, which means that recursive
    /// calls of `with` or calls to `Event` methods aren't allowed and
    /// will deadlock or panic.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        let mut inner = (self.1).0.lock().unwrap();
        let maxidx = inner.events.len();
        let idx = if let hash_map::Entry::Occupied(mut entry) = inner.listeners.entry(self.0) {
            std::mem::replace(entry.get_mut(), maxidx)
        } else {
            unreachable!();
        };
        f(&inner.events[idx..])
    }

    /// Returns a list of new events since last `peek`
    pub fn peek(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.with(<[T]>::to_vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::drop;

    #[test]
    fn test_event_listener() {
        let mut event = Event::new();

        event.push(0i32);

        let listener = event.new_listener();

        event.push(1i32);
        event.push(2i32);
        event.push(3i32);

        assert_eq!(listener.peek(), &[1, 2, 3]);

        drop(listener);
    }

    #[test]
    fn test_event_cleanup() {
        let mut event = Event::new();

        let listener_1 = event.new_listener();

        event.push(10i32);

        event.cleanup();
        assert_eq!(event.event_len(), 1);

        let listener_2 = event.new_listener();

        event.push(20i32);

        assert_eq!(listener_1.peek(), &[10i32, 20i32]);
        assert_eq!(listener_2.peek(), &[20i32]);
        assert_eq!(listener_2.peek(), &[]);
        assert_eq!(listener_2.peek(), &[]);

        event.cleanup();
        assert_eq!(event.event_len(), 0);

        for _i in 0..10 {
            event.push(30i32);
        }

        assert_eq!(listener_2.peek(), &[30i32; 10]);

        drop(listener_1);

        event.cleanup();
        assert_eq!(event.event_len(), 0);
    }
}
