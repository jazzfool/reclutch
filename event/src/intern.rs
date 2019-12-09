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

    /// Get the start index of new events since last `pull`
    fn pull(&mut self, key: ListenerKey) -> usize {
        let maxidx = self.events.len();
        std::mem::replace(self.listeners.get_mut(key).unwrap(), maxidx)
    }

    /// Applies a function to the list of new events since last `pull`
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

    /// Get the next event since last `pull`
    #[inline]
    pub fn peek_get(&self, key: ListenerKey) -> Option<&T> {
        self.events.get(*self.listeners.get(key)?)
    }

    /// Finish with this peek, go to next event
    #[inline]
    pub fn peek_finish(&mut self, key: ListenerKey) {
        let maxidx = self.events.len();
        let was_blocker = self
            .listeners
            .get_mut(key)
            .map(|idx| {
                if *idx < maxidx {
                    // only increment the idx if it is in bounds
                    *idx += 1;
                    *idx == 1
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if was_blocker {
            // this was a blocker
            self.cleanup();
        }
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn events_len(&self) -> usize {
        self.events.len()
    }
}

impl<T> crate::traits::QueueInterfaceCommon for Queue<T> {
    type Item = T;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl<T: Clone> crate::traits::EmitterMut for Queue<T> {
    #[inline]
    fn emit<'a>(&mut self, event: std::borrow::Cow<'a, T>) -> crate::traits::EmitResult<'a, T> {
        if !self.listeners.is_empty() {
            self.events.push(event.into_owned());
            crate::traits::EmitResult::Delivered
        } else {
            crate::traits::EmitResult::Undelivered(event)
        }
    }
}

impl<A> std::iter::Extend<A> for Queue<A> {
    #[inline]
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = A>,
    {
        if !self.listeners.is_empty() {
            self.events.extend(iter)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Queue;
    use crate::traits::EmitterMutExt;

    #[test]
    fn test_event_listener() {
        let mut event = Queue::new();

        event.emit_owned(0).into_result().unwrap_err();

        let listener = event.create_listener();

        event.emit_owned(1).into_result().unwrap();
        event.emit_owned(2).into_result().unwrap();
        event.emit_owned(3).into_result().unwrap();

        event.pull_with(listener, |x| assert_eq!(x, &[1, 2, 3]));

        event.remove_listener(listener);
    }

    #[test]
    fn test_event_cleanup() {
        let mut event = Queue::new();

        let listener_1 = event.create_listener();

        event.emit_owned(10).into_result().unwrap();

        assert_eq!(event.events_len(), 1);

        let listener_2 = event.create_listener();

        event.emit_owned(20).into_result().unwrap();

        event.pull_with(listener_1, |x| assert_eq!(x, &[10, 20]));
        event.pull_with(listener_2, |x| assert_eq!(x, &[20]));
        event.pull_with(listener_2, |x| assert_eq!(x, &[]));
        event.pull_with(listener_2, |x| assert_eq!(x, &[]));

        assert_eq!(event.events_len(), 0);

        for _i in 0..10 {
            event.emit_owned(30).into_result().unwrap();
        }

        event.pull_with(listener_2, |x| assert_eq!(x, &[30; 10]));

        event.remove_listener(listener_1);

        assert_eq!(event.events_len(), 0);
    }
}
