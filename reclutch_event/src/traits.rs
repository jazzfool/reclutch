/// Internal glue code to avoid boilerplate and repetition
pub mod private {
    pub use crate::intern::EventIntern;

    pub trait EventInterface<T>: Clone + Default {
        fn with_inner<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&EventIntern<T>) -> R;

        fn with_inner_mut<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut EventIntern<T>) -> R;
    }

    pub trait EventListen<T> {
        fn with_inner_mut<F, R>(&self, f: F) -> Option<R>
        where
            F: FnOnce(crate::intern::ListenerKey, &mut EventIntern<T>) -> R;
    }
}

pub trait GenericEventInterface<T>: Clone + Default {
    /// Pushes/emits an event
    fn push(&self, event: T);

    /// Emits multiple events in one go
    fn extend<I>(&self, events: I)
    where
        I: IntoIterator<Item = T>;

    /// Returns the number of listeners
    fn listener_len(&self) -> usize;

    /// Returns the number of events
    fn event_len(&self) -> usize;
}

impl<T, EI> GenericEventInterface<T> for EI
where
    EI: private::EventInterface<T>,
{
    #[inline]
    fn push(&self, event: T) {
        self.with_inner_mut(|inner| inner.events.push(event));
    }

    #[inline]
    fn extend<I>(&self, events: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.with_inner_mut(|inner| inner.events.extend(events));
    }

    #[inline]
    fn listener_len(&self) -> usize {
        self.with_inner(|inner| inner.listeners.len())
    }

    #[inline]
    fn event_len(&self) -> usize {
        self.with_inner(|inner| inner.events.len())
    }
}

pub trait EventInterface<T>: GenericEventInterface<T> {
    type Listener: EventListen<T>;

    /// Creates a new event
    fn new() -> Self {
        Default::default()
    }

    /// Returns a handle to a new listener
    fn listen(&self) -> Self::Listener;
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

impl<T, EL> EventListen<T> for EL
where
    EL: private::EventListen<T>,
{
    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        self.with_inner_mut(|key, ev| ev.pull_with(key, f)).unwrap()
    }
}
