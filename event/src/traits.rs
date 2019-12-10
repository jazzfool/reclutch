use std::borrow::Cow;

/// `EmitResult` indicates the success or failure of an `event emit`.
/// * `Delivered` means the event was emitted with possible listeners present.
/// * `Undelivered` means replaced with `()` along the way
///   and contains the unconsumed `Cow` argument.
///   Take note that some emit methods might return an always owned
///   event instead.
#[derive(Debug, Clone)]
pub enum EmitResult<'a, T: Clone> {
    Delivered,
    Undelivered(Cow<'a, T>),
}

impl<'a, T: Clone> EmitResult<'a, T> {
    /// Returns true if the result is `Delivered`, otherwise false.
    pub fn was_delivered(&self) -> bool {
        match self {
            EmitResult::Delivered => true,
            EmitResult::Undelivered(_) => false,
        }
    }

    /// Returns true if the result is `Undelivered`, otherwise false.
    pub fn was_undelivered(&self) -> bool {
        match self {
            EmitResult::Delivered => false,
            EmitResult::Undelivered(_) => true,
        }
    }

    /// Converts this `EmitResult` into `std::result::Result`.
    pub fn to_result(self) -> Result<(), Cow<'a, T>> {
        self.into()
    }
}

impl<'a, T: Clone> From<Result<(), Cow<'a, T>>> for EmitResult<'a, T> {
    fn from(result: Result<(), Cow<'a, T>>) -> Self {
        match result {
            Result::Ok(_) => EmitResult::Delivered,
            Result::Err(x) => EmitResult::Undelivered(x),
        }
    }
}

impl<'a, T: Clone> Into<Result<(), Cow<'a, T>>> for EmitResult<'a, T> {
    fn into(self) -> Result<(), Cow<'a, T>> {
        match self {
            EmitResult::Delivered => Result::Ok(()),
            EmitResult::Undelivered(x) => Result::Err(x),
        }
    }
}

pub trait QueueInterfaceCommon {
    type Item;

    /// Checks if any events are currently buffered
    ///
    /// # Return value
    /// * `false` if events are currently buffered
    /// * `true` if no events are currently buffered or the event queue doesn't
    ///   support querying this information
    fn buffer_is_empty(&self) -> bool {
        true
    }
}

/// Every supported event queue implements this trait, mutable variant
///
/// More types implement this trait (including all types which implement
/// [`Emitter`]), but can't be fully accessed via `Rc`/`Arc`.
///
/// This trait should be used as a trait bound if possible
/// (instead of the non-mut variant).
pub trait EmitterMut: QueueInterfaceCommon
where
    Self::Item: Clone,
{
    /// Pushes/emits an event
    fn emit<'a>(&mut self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item>;
}

/// Every supported indirect event queue implements this trait
///
/// One should always prefer implementing `Emitter` over
/// [`EmitterMut`] because this trait allows access via `Rc`/`Arc`
/// and because implementing `Emitter` automatically
/// provides one with a implementation of `EmitterMut` thanks
/// to the provided blanket implementation.
pub trait Emitter: QueueInterfaceCommon
where
    Self::Item: Clone,
{
    /// Pushes/emits an event
    fn emit<'a>(&self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item>;
}

/// Event queues with the ability to create new listeners implement this trait
pub trait QueueInterfaceListable: QueueInterfaceCommon
where
    Self::Item: Clone,
{
    type Listener: Listen<Item = Self::Item>;

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

pub trait EmitterMutExt: EmitterMut
where
    Self::Item: Clone,
{
    /// Pushs/emits an event, performing conversion from owned
    #[inline]
    fn emit_owned(&mut self, event: Self::Item) -> EmitResult<'static, Self::Item> {
        self.emit(Cow::Owned(event))
    }

    /// Pushs/emits an event, performing conversion from borrowed
    #[inline]
    fn emit_borrowed<'a>(&mut self, event: &'a Self::Item) -> EmitResult<'a, Self::Item> {
        self.emit(Cow::Borrowed(event))
    }
}

impl<Q: EmitterMut> EmitterMutExt for Q where Self::Item: Clone {}

pub trait EmitterExt: Emitter
where
    Self::Item: Clone,
{
    /// Pushs/emits an event, performing conversion from owned (convenience method)
    #[inline]
    fn emit_owned(&self, event: Self::Item) -> EmitResult<'static, Self::Item> {
        self.emit(Cow::Owned(event))
    }

    /// Pushs/emits an event, performing conversion from borrowed (convenience method)
    #[inline]
    fn emit_borrowed<'a>(&self, event: &'a Self::Item) -> EmitResult<'a, Self::Item> {
        self.emit(Cow::Borrowed(event))
    }
}

impl<Q: Emitter> EmitterExt for Q where Self::Item: Clone {}

pub trait Listen {
    type Item;

    /// Applies a function to the list of new events since last `with` or `peek`
    /// without cloning T
    ///
    /// This function is faster than calling [`peek`](Listen::peek)
    /// and iterating over the result.
    ///
    /// It holds a lock on the event while called, which means that recursive
    /// calls to [`QueueInterfaceListable`] methods aren't allowed and will deadlock or panic.
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Self::Item]) -> R;

    /// Applies a function to each new event since last `with` or `peek`
    /// without cloning T
    ///
    /// This function is sometimes faster than calling [`with`](Listen::with).
    ///
    /// It holds a lock on the event while called, which means that recursive
    /// calls to [`QueueInterfaceListable`] methods aren't allowed and will deadlock or panic.
    #[inline]
    fn map<F, R>(&self, mut f: F) -> Vec<R>
    where
        F: FnMut(&Self::Item) -> R,
    {
        self.with(|slc| slc.iter().map(|i| f(i)).collect())
    }

    /// Returns a list of new events since last `peek`
    #[inline]
    fn peek(&self) -> Vec<Self::Item>
    where
        Self::Item: Clone,
    {
        self.with(<[Self::Item]>::to_vec)
    }
}
