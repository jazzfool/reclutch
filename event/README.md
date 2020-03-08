# `reclutch_event`

## Overview

```
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

## Callbacks

At this level, there are no closures when it comes to callbacks. However, the `verbgraph` event queue abstraction does provide closure-based event handling.

Instead, the event module uses a simple event queue with listener primitives.

At it's very core, the event system works like this;
- An event queue store a list of events (we'll call this `Vec<E>`) and a dictionary of listeners (we'll call this `Map<L, Idx>`).
- `E` is defined to be a generic type; the event type itself.
- `L` is defined to be a listener ID that is locally unique within the event queue (not globally unique; e.g. not unique across event queues).
- `Idx` represents an index in `Vec<E>`. The event at this index holds the latest event the listener (`L`) has "seen".
- As long as the owning scope of some listener `L` has an immutable reference to the associated event queue, then the events can be processed. We will call the events processed `Vec<Ep>`.
- The contents of `Vec<Ep>` is defined to be a slice of `Vec<E>`, `[Idx..]`.
- Thus, `Vec<Ep>` only contains events that have not been seen by `L` yet, or simply, the events past `Idx`.
- Once `Vec<Ep>` is returned, `Idx` is set to `Vec<E>.len()`.

This, however, is only the first part of the event system. It can be determined that there is some lower bound, `Idx`, which signifies the lowest common listener index; the index at which all the preceding events have been "seen" by all the listeners. Therefore, a cleanup process that removes all events preceding `Idx` can be implemented.
- For most implementations, the following steps are invoked by a listener going out of scope.
- Let `Vec<E>` be the list of events and `Idx` be the lowest common listener index.
- The slice `[..Idx]` is removed from `Vec<E>`.
- However, recall that `Map<L, Idx>` stores an index in `Vec<E>` much like a pointer.
- Consequent to the removal of `[..Idx`], `Vec<E>` has been offset by `-Idx` such that all indices of a given listener `L` have been invalidated.
- To solve this invalidation, each index of all `L` in `Map<L, Idx>` is offset by `-Idx`.
- Hence, all events that have been seen by all existing listeners and thereby will never be seen by any listener again (i.e. completely obsolete), are removed.

This cleanup process is not perfectly efficient, however. The lowest common listener index can easily be "held back" by a stale listener that hasn't peeked new events for a while.

Thanks to zserik and his excellent contributions, the events have been made more ergonomic to use and now utilize the RAII pattern to automatically cleanup when listeners go out of scope.
This module's API is reaching a stable point and is capable for usage outside of a UI.

Here's an example of it's usage outside a widget (with manual updating);

```rust
let mut event: RcEventQueue<i32> = RcEventQueue::new();

event.emit_owned(10); // no listeners, so this event won't be received by anyone.

let listener = event.listen();

event.emit_owned(1);
event.emit_owned(2);

// here is how listeners respond to events.
for num in listener.peek() {
    print!("{} ", num);
} // prints: "1 2 "

std::mem::drop(listener); // explicitly called to illustrate cleanup; this removes the listener and therefore doesn't hold back the cleanup process.
```

## Advanced features

For more advanced use cases, this crate has some feature flags, all disabled by default.
* `crossbeam-channel`
* `winit`

## `crossbeam-channel` support

This crate offers a simple non-blocking API by default. But this isn't enough in multi-threaded
scenarios, because often polling/busy-waiting inside of non-main-threads isn't wanted and wastes
resources. Thus, this crate offers an blocking API if the `crossbeam-channel` feature is enabled,
which utilizes `crossbeam-channel` to support "blocking until an event arrives".

### Cascades

Sometimes, it is necessary to route events between multiple threads and event queues.
When the `crossbeam-channel` feature is enabled, this crate offers the `cascade` API,
which supports filtered event forwarding.

## `winit` support

This feature is in particular useful if combined with the `crossbeam-channel` feature,
because it allows the `cascade` API to deliver events back into the main `winit` event queue.
