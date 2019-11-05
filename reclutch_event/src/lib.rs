mod intern;

/// This module contains the non-thread-safe API
pub mod nonts;

mod traits;

/// This module contains the thread-safe API
pub mod ts;

pub(crate) use crate::intern::ListenerKey;
pub use crate::{
    nonts::{Event as RcEvent, EventListener as RcEventListener},
    traits::*,
    traits::private,
    ts::{Event as ArcEvent, EventListener as ArcEventListener},
};
