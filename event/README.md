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

There are no closures when it comes to callbacks, as it would be too much work to have it fit in safe Rust (and it definitely wouldn't be ergonomic to use).

Instead, Reclutch uses a simple event queue with listener primitives.

Essentially, the event has a list of all events emitted, stored as a vector, and a list of all listeners, stored as a map. The key of the map is what the listener-facing API stores.
The value of the map is simply an index. This index keeps track of the last time the event queue was peeked for a specific listener.
This also allows for simple cleanup of event data not needed (i.e. event data seen by all the listeners); An implicitly called cleanup function looks for the lowest listener index and removes everything before it.
Further, it's not a "standalone" event system, it only works in a widget environment (or any environment with persistent updates).
The memory cleanup system resembles an extremely simple garbage collector.

Thanks to zserik, the events have been made more ergonomic to use and now use the RAII pattern to automatically cleanup when listeners go out of scope.
The event system is still a work-in-progress and we're looking to find the right balance between performance and ease-of-use.

Here's an example of it's usage outside a widget (with manual updating);

```rust
let mut event: RcEventQueue<i32> = RcEventQueue::new();

event.push(10); // no listeners, so this event won't be received by anyone.

let listener = event.listen();

event.push(1);
event.push(2);

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
