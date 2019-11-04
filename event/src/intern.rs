pub(crate) type ListenerKey = slotmap::DefaultKey;

/// Non-thread-safe, non-reference-counted API
#[derive(Debug)]
pub struct Queue<T> {
    pub(crate) listeners: slotmap::SlotMap<ListenerKey, usize>,
    pub(crate) events: Vec<T>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            listeners: Default::default(),
            events: Vec::new(),
        }
    }
}

impl<T> Queue<T> {
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

    pub fn push(&mut self, x: T) -> bool {
        if self.listeners.is_empty() {
            return false;
        }
        self.events.push(x);
        true
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

    #[cfg(test)]
    #[inline]
    pub(crate) fn event_len(&self) -> usize {
        self.events.len()
    }
}

impl<A> std::iter::Extend<A> for Queue<A> {
    #[inline]
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = A>,
    {
        self.events.extend(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::Queue;

    #[test]
    fn test_event_listener() {
        let mut event = Queue::new();

        event.push(0i32);

        let listener = event.create_listener();

        event.push(1i32);
        event.push(2i32);
        event.push(3i32);

        event.pull_with(listener, |x| assert_eq!(x, &[1, 2, 3]));

        event.remove_listener(listener);
    }

    #[test]
    fn test_event_cleanup() {
        let mut event = Queue::new();

        let listener_1 = event.create_listener();

        event.push(10i32);

        assert_eq!(event.event_len(), 1);

        let listener_2 = event.create_listener();

        event.push(20i32);

        event.pull_with(listener_1, |x| assert_eq!(x, &[10i32, 20i32]));
        event.pull_with(listener_2, |x| assert_eq!(x, &[20i32]));
        event.pull_with(listener_2, |x| assert_eq!(x, &[]));
        event.pull_with(listener_2, |x| assert_eq!(x, &[]));

        assert_eq!(event.event_len(), 0);

        for _i in 0..10 {
            event.push(30i32);
        }

        event.pull_with(listener_2, |x| assert_eq!(x, &[30i32; 10]));

        event.remove_listener(listener_1);

        assert_eq!(event.event_len(), 0);
    }
}
