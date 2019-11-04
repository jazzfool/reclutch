//! # `Listener`'s
//! `Listener`'s should be wrapped inside of an Rc or Arc
//! if you want multiple references to the same listener

mod intern;
mod traits;

/// Contains the non-thread-safe, non-reference-counted API
pub mod nonrc;

/// Contains the non-thread-safe, reference-counted API
pub mod nonts;

/// Contains an Event queue merger
pub mod merge;

#[cfg(feature = "crossbeam-channel")]
/// Contains a thread-safe event-cascading API based upon the
/// subscribable thread-safe APIs.
pub mod cascade;

#[cfg(feature = "crossbeam-channel")]
/// Contains the subscribable thread-safe API
/// using tokens sent via crossbeam channels
///
/// This event queue wrapper is slower than `dchans`,
/// but uses lesser memory.
pub mod chans;

#[cfg(feature = "crossbeam-channel")]
/// Contains the subscribable thread-safe API
/// using direct clones of T sent via crossbeam channels
///
/// This event queue wrapper is faster than `chans`,
/// but uses more memory, because event items are cloned
/// before being sent via crossbeam channels.
pub mod dchans;

/// Contains the subscribable thread-safe API
/// using direct clones of T sent via std channels
pub mod schans;

pub(crate) use crate::intern::ListenerKey;

pub use crate::{
    intern::Queue as RawEventQueue,
    nonrc::{Listener as NonRcEventListener, Queue as NonRcEventQueue},
    nonts::{Listener as RcEventListener, Queue as RcEventQueue},
    traits::*,
};

// implementation of traits for 3rd party types
#[doc(hidden)]
pub mod thirdparty;

pub mod prelude {
    pub use crate::traits::{GenericQueueInterface as _, Listen as _, QueueInterface as _};
}
