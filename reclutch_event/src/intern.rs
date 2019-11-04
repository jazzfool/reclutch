pub(crate) type ListenerKey = slotmap::DefaultKey;

#[derive(Debug)]
#[doc(hidden)]
pub struct EventIntern<T> {
    pub(crate) listeners: slotmap::SlotMap<ListenerKey, usize>,
    pub(crate) events: Vec<T>,
}

impl<T> Default for EventIntern<T> {
    fn default() -> Self {
        Self {
            listeners: Default::default(),
            events: Vec::new(),
        }
    }
}

impl<T> EventIntern<T> {
    /// Create a new event queue
    pub fn new() -> Self {
        Default::default()
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

impl<A> std::iter::Extend<A> for EventIntern<A> {
    #[inline]
    fn extend<T>(&mut self, iter: T)
    where
        T: std::iter::IntoIterator<Item = A>,
    {
        self.events.extend(iter)
    }
}
