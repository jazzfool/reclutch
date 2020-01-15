/*!

```text
╭─── RawEventQueue
│
│ ╔═════════════════╦════════════════════╦══════════════════╗
│ ║  sending halves ║     forwarders     ║ receiving halves ║
│ ╟─────────────────╫────────────────────╫──────────────────╢
│ ║ ╭───────────────╫────────────────────╫────────────╮     ║
│ ║ │               ║                    ║  (push_*)  ↑     ║
│ ║ │               ║ ╭───── cascade::* ─╫──┬filter───╯     ║
↕ ║ ↓               ║ ↑                  ║  ╰*...─────╯     ║
│ ║ │               ║ │                  ║                  ║
╰─╫─┼─ Emitter[Mut]─╫─┴─ if Listable ─>>─╫──┬─ Listen       ║
  ╠═╪═══════════════╩════════════════════╬══╪═════════════╤═╩═══════════════╗
  ║ ├─ nested Emitters...                ║  ├─ merge::*   │  multiplexing/  ║
  ║ │   (broadcasting)                   ║  │  (muxing)   │  broadcasting   ║
  ╚═╪════════════════════════════════════╩══╪═════════════╧═════════════════╝
    ↑                                       ↓
    ╰─ emit(_Event)                         ╰─ with(_f(&[_Event]))
```

# `Emitter`'s

`Emitter`'s (data types which implement the [`EventEmitter`](crate::prelude::EventEmitter) trait)
represent an event queue instance including the event sending capability
and often (when they implement the `QueueInterfaceListable`)
have the ability to construct new `Listener`s.

# `Listener`'s

`Listener`'s (data types which implement the `EventListen` trait)
represent the receiving end of an event queue.
A `Listener` should be wrapped inside of an Rc or Arc
if you want multiple references to the same listener

# `crossbeam-channel` support

This crate offers a simple non-blocking API by default. But this isn't enough in multi-threaded
scenarios, because often polling/busy-waiting inside of non-main-threads isn't wanted and wastes
resources. Thus, this crate offers an blocking API if the `crossbeam-channel` feature is enabled,
which utilizes `crossbeam-channel` to support "blocking until an event arrives".

In this documentation, all items marked with `channels` are only available if the
`crossbeam-channel` feature is enabled.

## Cascades

Sometimes, it is necessary to route events between multiple threads and event queues.
When the `crossbeam-channel` feature is enabled, this crate offers the `cascade` API,
which supports filtered event forwarding.

*/

#![cfg_attr(feature = "docs", feature(doc_cfg))]

mod intern;
#[macro_use]
mod macros;
mod traits;

/// Contains an bidirectional `1:1`, non-thread-safe, reference-counted API
pub mod bidir;

/// Like `bidir`, but each direction can only save one event at a time
pub mod bidir_single;

channels_api! {
    /// Contains a thread-safe event-cascading API based upon the
    /// subscribable thread-safe APIs.
    pub mod cascade;

    /// Contains the subscribable thread-safe API
    /// using tokens sent via crossbeam channels
    ///
    /// This event queue wrapper is slower than `dchans`,
    /// but uses lesser memory.
    pub mod chans;

    /// Contains the subscribable thread-safe API
    /// using direct clones of T sent via crossbeam channels
    ///
    /// This event queue wrapper is faster than `chans`,
    /// but uses more memory, because event items are cloned
    /// before being sent via crossbeam channels.
    pub mod dchans;
}

/// Contains an asynchronous, thread-safe API
/// and wrapper types
#[cfg(feature = "futures")]
#[cfg_attr(feature = "docs", doc(cfg(futures)))]
pub mod streaming;

/// Contains an Event queue merger
pub mod merge;

/// Contains the non-thread-safe, non-reference-counted API
pub mod nonrc;

/// Contains the non-thread-safe, reference-counted API
pub mod nonts;

/// Contains the thread-safe, reference-counted API
pub mod ts;

// implementation of traits for 3rd party types
#[doc(hidden)]
pub mod thirdparty;

/// An event queue which drops any incoming item
/// and is always closed.
pub type BlackHole<T> = std::marker::PhantomData<T>;

use intern::ListenerKey;

/// Exports the most important traits
pub mod prelude {
    pub use crate::traits::{
        Emitter as EventEmitter, EmitterExt as EventEmitterExt, EmitterMut as EventEmitterMut,
        EmitterMutExt as EventEmitterMutExt, Listen as EventListen, QueueInterfaceCommon,
        QueueInterfaceListable,
    };
}

pub use {
    intern::Queue as RawEventQueue,
    nonrc::{Listener as NonRcEventListener, Queue as NonRcEventQueue},
    nonts::{Listener as RcEventListener, Queue as RcEventQueue},
    prelude::*,
    traits::EmitResult,
};
