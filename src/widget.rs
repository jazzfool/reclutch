use crate::display::{GraphicsDisplay, Rect};

pub trait Widget<E> {
    fn children(&self) -> Vec<&dyn Widget<E>> {
        Vec::new()
    }

    fn children_mut(&mut self) -> Vec<&mut dyn Widget<E>> {
        Vec::new()
    }

    fn bounds(&self) -> Rect;

    fn update(&mut self, _global: &mut Event<E>) {}

    fn draw(&mut self, display: &mut dyn GraphicsDisplay);
}

#[derive(Debug)]
pub struct EventListener<T>(u64, std::marker::PhantomData<T>);

impl<T> Clone for EventListener<T> {
    fn clone(&self) -> Self {
        EventListener(self.0, std::marker::PhantomData)
    }
}

impl<T> Copy for EventListener<T> {}

pub struct Event<T> {
    listeners: std::collections::HashMap<u64, usize>,
    next_listener_id: u64,
    events: Vec<T>,
}

impl<T> Event<T> {
    /// Creates a new event
    pub fn new() -> Self {
        Self {
            listeners: Default::default(),
            next_listener_id: 0,
            events: Vec::new(),
        }
    }

    /// Pushes/emits an event
    pub fn push(&mut self, event: T) {
        self.events.push(event);
    }

    /// Returns a list of new events since last `peek`
    pub fn peek(&mut self, listener: EventListener<T>) -> &[T] {
        assert!(self.listeners.contains_key(&listener.0));

        let idx = *self.listeners.get(&listener.0).unwrap();

        if self.events.len() == idx {
            &[]
        } else {
            self.listeners.insert(listener.0, self.events.len());

            &self.events[idx..]
        }
    }

    /// Returns a handle to a new listener
    pub fn new_listener(&mut self) -> EventListener<T> {
        let id = self.next_listener_id;
        self.next_listener_id += 1;

        self.listeners.insert(id, self.events.len());

        EventListener(id, std::marker::PhantomData)
    }

    /// Removes a listener and invokes `cleanup`
    pub fn remove_listener(&mut self, listener: EventListener<T>) {
        self.listeners.remove(&listener.0);
        self.cleanup();
    }

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

    /// Returns the number of listeners
    pub fn listener_len(&self) -> usize {
        self.listeners.len()
    }

    /// Returns the number of even
    pub fn event_len(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_listener() {
        let mut event = Event::new();

        event.push(0i32);

        let listener = event.new_listener();

        event.push(1i32);
        event.push(2i32);
        event.push(3i32);

        assert_eq!(event.peek(listener), &[1, 2, 3]);

        event.remove_listener(listener);
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

        assert_eq!(event.peek(listener_1), &[10i32, 20i32]);
        assert_eq!(event.peek(listener_2), &[20i32]);

        event.cleanup();
        assert_eq!(event.event_len(), 0);

        for _i in 0..10 {
            event.push(30i32);
        }

        assert_eq!(event.peek(listener_2), &[30i32; 10]);

        event.remove_listener(listener_1);

        event.cleanup();
        assert_eq!(event.event_len(), 0);
    }
}
