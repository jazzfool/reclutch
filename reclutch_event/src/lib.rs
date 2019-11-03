pub(crate) type ListenerKey = slotmap::DefaultKey;

pub mod nonts;
pub mod ts;

pub use nonts::{Event as RcEvent, EventListener as RcEventListener};
pub use ts::{Event as ArcEvent, EventListener as ArcEventListener};

#[derive(Debug)]
pub(crate) struct EventIntern<T> {
    listeners: slotmap::SlotMap<ListenerKey, usize>,
    events: Vec<T>,
}

impl<T> EventIntern<T> {
    /// Create a new event queue
    pub fn new() -> Self {
        Self {
            listeners: Default::default(),
            events: Vec::new(),
        }
    }

    /// Removes all events that have been already seen by all listeners
    fn cleanup(&mut self) {
        let min_idx = *self.listeners.values().min().unwrap_or(&0);
        if min_idx == 0 {
            return;
        }

        for idx in self.listeners.values_mut() {
            *idx -= min_idx;
        }

        self.events.drain(0..min_idx);
    }

    /// Creates a subscription
    pub fn create_listener(&mut self) -> ListenerKey {
        let maxidx = self.events.len();
        self.listeners.insert(maxidx)
    }

    /// Removes a subscription
    pub fn remove_listener(&mut self, key: ListenerKey) {
        // oldidx != 0 --> this is not a blocker
        if self.listeners.remove(key) == Some(0) {
            self.cleanup();
        }
    }

    /// Get a the start index of new events since last `pull`
    fn pull(&mut self, key: ListenerKey) -> usize {
        let maxidx = self.events.len();
        std::mem::replace(self.listeners.get_mut(key).unwrap(), maxidx)
    }

    /// Applies a function to the list of new events since last `pull`/`pull_with`
    #[inline]
    pub fn pull_with<F, R>(&mut self, key: ListenerKey, f: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        let idx = self.pull(key);
        let ret = f(&self.events[idx..]);
        if idx == 0 {
            // this was a blocker
            self.cleanup();
        }
        ret
    }
}

pub trait EventInterface<T>: Clone + Default {
    type Listener: EventListen<T>;

    /// Creates a new event
    fn new() -> Self;

    /// Pushes/emits an event
    fn push(&self, event: T);

    /// Returns a handle to a new listener
    fn listen(&self) -> Self::Listener;

    /// Returns the number of listeners
    fn listener_len(&self) -> usize;

    /// Returns the number of events
    fn event_len(&self) -> usize;
}

pub trait EventListen<T> {
    /// Applies a function to the list of new events since last `with` or `peek`
    /// without cloning T
    ///
    /// This function is faster than calling [`peek`](EventListen::peek)
    /// and iterating over the result.
    ///
    /// It holds a lock on the event while called, which means that recursive
    /// calls to [`EventInterface`] methods aren't allowed and will deadlock or panic.
    ///
    /// This function calls [`cleanup`](EventInterface::cleanup)
    /// when the callback returns (implicit via a call to `pull_with`).
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R;

    /// Returns a list of new events since last `peek`
    #[inline]
    fn peek(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.with(<[T]>::to_vec)
    }
}
