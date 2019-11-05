#[macro_use]
extern crate criterion;

use criterion::Criterion;
use reclutch_event::*;
use std::mem::drop;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("event-listener-peek", move |b| {
        b.iter(|| {
            let event = RcEvent::new();

            event.push(0i32);

            let listener = event.listen();

            event.push(1i32);
            event.push(2i32);
            event.push(3i32);

            assert_eq!(listener.peek(), &[1, 2, 3]);
        })
    });

    c.bench_function("event-listener-with", move |b| {
        b.iter(|| {
            let event = RcEvent::new();

            event.push(0i32);

            let listener = event.listen();

            event.push(1i32);
            event.push(2i32);
            event.push(3i32);

            listener.with(|events| {
                assert_eq!(events, &[1i32, 2i32, 3i32]);
            });
        })
    });

    c.bench_function("event-cleanup", move |b| {
        b.iter(|| {
            let event = RcEvent::new();

            let listener_1 = event.listen();

            event.push(10i32);

            assert_eq!(event.event_len(), 1);

            let listener_2 = event.listen();

            event.push(20i32);

            assert_eq!(listener_1.peek(), &[10i32, 20i32]);
            assert_eq!(listener_2.peek(), &[20i32]);
            let empty_peeked: &[i32] = &[];
            assert_eq!(listener_2.peek(), empty_peeked);
            assert_eq!(listener_2.peek(), empty_peeked);

            assert_eq!(event.event_len(), 0);

            event.extend([30i32; 10].iter().copied());

            assert_eq!(listener_2.peek(), &[30i32; 10]);

            drop(listener_1);

            assert_eq!(event.event_len(), 0);
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
