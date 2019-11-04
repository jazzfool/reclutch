mod private_ {
    pub trait SealedListen {}

    #[cfg(feature = "crossbeam-channel")]
    impl<T> SealedListen for crate::chans::Listener<T> {}
    #[cfg(feature = "crossbeam-channel")]
    impl<T> SealedListen for crate::dchans::Listener<T> {}
    impl<T> SealedListen for crate::schans::Listener<T> {}

    impl<T> SealedListen for crate::nonrc::Listener<'_, T> {}
    impl<T> SealedListen for crate::nonts::Listener<T> {}
}

/// Internal glue code to avoid boilerplate and repetition
pub mod private {
    use crate::RawEventQueue;

    pub trait QueueInterface<T>: Default {
        fn with_inner<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&RawEventQueue<T>) -> R;

        fn with_inner_mut<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut RawEventQueue<T>) -> R;
    }

    pub trait Listen<T> {
        fn with_inner_mut<F, R>(&self, f: F) -> Option<R>
        where
            F: FnOnce(crate::intern::ListenerKey, &mut RawEventQueue<T>) -> R;
    }

    #[inline]
    pub(crate) fn extend<T, EQ, I>(equeue: &EQ, events: I) -> bool
    where
        EQ: QueueInterface<T>,
        I: IntoIterator<Item = T>,
    {
        equeue.with_inner_mut(|inner| {
            if inner.listeners.is_empty() {
                return false;
            }
            inner.events.extend(events);
            true
        })
    }
}

pub trait GenericQueueInterface<T> {
    /// Pushes/emits an event
    fn push(&self, event: T) -> bool;

    /// Emits multiple events in one go
    fn extend<I>(&self, events: I) -> bool
    where
        I: IntoIterator<Item = T>,
    {
        for i in events.into_iter() {
            if !self.push(i) {
                return false;
            }
        }
        true
    }

    /// Checks if any events are currently buffered
    ///
    /// # Return value
    /// * `false` if events are currently buffered
    /// * `true` if no events are currently buffered or the event queue doesn't
    ///   support querying this information
    fn is_empty(&self) -> bool {
        true
    }
}

pub trait QueueInterface<T>: GenericQueueInterface<T> {
    type Listener: Listen<T>;

    /// Creates a new event
    fn new() -> Self
    where
        Self: Default,
    {
        Default::default()
    }

    /// Returns a handle to a new listener
    fn listen(&self) -> Self::Listener;
}

pub trait Listen<T> {
    /// Applies a function to the list of new events since last `with` or `peek`
    /// without cloning T
    ///
    /// This function is faster than calling [`peek`](Listen::peek)
    /// and iterating over the result.
    ///
    /// It holds a lock on the event while called, which means that recursive
    /// calls to [`QueueInterface`] methods aren't allowed and will deadlock or panic.
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R;

    /// Applies a function to each new event since last `with` or `peek`
    /// without cloning T
    ///
    /// This function is sometimes faster than calling [`with`](Listen::with).
    ///
    /// It holds a lock on the event while called, which means that recursive
    /// calls to [`QueueInterface`] methods aren't allowed and will deadlock or panic.
    #[inline]
    fn map<F, R>(&self, mut f: F) -> Vec<R>
    where
        F: FnMut(&T) -> R,
    {
        self.with(|slc| slc.iter().map(|i| f(i)).collect())
    }

    /// Returns a list of new events since last `peek`
    #[inline]
    fn peek(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.with(<[T]>::to_vec)
    }
}

impl<T, EL> Listen<T> for EL
where
    EL: private::Listen<T> + private_::SealedListen,
{
    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        self.with_inner_mut(|key, ev| ev.pull_with(key, f)).unwrap()
    }
}
