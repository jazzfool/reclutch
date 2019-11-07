use crate::{
    cascade::{utils::CleanupIndices, CascadeTrait},
    traits::private::Listen as _,
    *,
};
use crossbeam_channel as chan;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
struct Intern<T> {
    ev: RawEventQueue<T>,
    subscribers: Vec<chan::Sender<T>>,
}

#[derive(Debug)]
pub struct Queue<T>(Arc<RwLock<Intern<T>>>);

impl<T> Clone for Queue<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for Intern<T> {
    #[inline]
    fn default() -> Self {
        Self {
            ev: Default::default(),
            subscribers: Default::default(),
        }
    }
}

impl<T> Default for Queue<T> {
    #[inline]
    fn default() -> Self {
        Self(Arc::new(RwLock::new(Default::default())))
    }
}

impl<T> Queue<T> {
    #[doc(hidden)]
    #[inline]
    fn with_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Intern<T>) -> R,
    {
        let inner = self.0.read().unwrap();
        f(&inner)
    }

    #[doc(hidden)]
    #[inline]
    fn with_inner_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Intern<T>) -> R,
    {
        let mut inner = self.0.write().unwrap();
        f(&mut inner)
    }

    pub fn subscribe(&self) -> chan::Receiver<T> {
        let (tx, rx) = chan::unbounded();
        self.with_inner_mut(move |inner| inner.subscribers.push(tx));
        rx
    }

    pub fn cascade(&self) -> Cascade<T>
    where
        T: Send + 'static,
    {
        Cascade::new(self.subscribe())
    }
}

// TODO: cache SendError.into_inner parts and reuse them

impl<T> GenericQueueInterface<T> for Queue<T>
where
    T: Clone,
{
    fn push(&self, event: T) -> bool {
        self.with_inner_mut(|inner| {
            inner.subscribers.retain(|i| {
                // try to send object, remove channel if unsubscribed
                i.send(event.clone()).is_ok()
            });
            // prevent wasting memory (we perform much lesser clean-ups)
            if !inner.ev.listeners.is_empty() {
                inner.ev.events.push(event);
                true
            } else {
                !inner.subscribers.is_empty()
            }
        })
    }

    fn extend<I>(&self, events: I) -> bool
    where
        I: IntoIterator<Item = T>,
    {
        let events: Vec<T> = events.into_iter().collect();
        self.with_inner_mut(move |inner| {
            inner.subscribers.retain(|i| {
                for elem in events.iter().cloned() {
                    // try to send object
                    if i.send(elem).is_err() {
                        // channel unsubscribed
                        return false;
                    }
                }
                true
            });
            // prevent wasting memory (we perform much lesser clean-ups)
            if !inner.ev.listeners.is_empty() {
                inner.ev.events.extend(events.into_iter());
                true
            } else {
                !inner.subscribers.is_empty()
            }
        })
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.with_inner(|inner| {
            if !inner.ev.events.is_empty() {
                return false;
            }
            for i in inner.subscribers.iter() {
                if !i.is_empty() {
                    return false;
                }
            }
            true
        })
    }
}

impl<T> QueueInterface<T> for Queue<T>
where
    T: Clone,
{
    type Listener = Listener<T>;

    #[inline]
    fn listen(&self) -> Listener<T> {
        Listener::new(self.clone())
    }
}

#[derive(Debug)]
pub struct Listener<T>(ListenerKey, Queue<T>);

impl<T> private::Listen<T> for Listener<T> {
    fn with_inner_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(crate::intern::ListenerKey, &mut RawEventQueue<T>) -> R,
    {
        let mut inner = (self.1).0.write().ok()?;
        Some(f(self.0, &mut inner.ev))
    }
}

impl<T> Drop for Listener<T> {
    fn drop(&mut self) {
        let _ = self.with_inner_mut(|key, ev| ev.remove_listener(key));
    }
}

impl<T> Listener<T> {
    fn new(event: Queue<T>) -> Self {
        let id = event.0.write().unwrap().ev.create_listener();
        Listener(id, event)
    }
}

#[derive(Debug)]
enum CascadeResultIntern<T> {
    ChangeToBlackhole,
    Disconnected,
    Forwarded,
    Kept(T),
}

pub struct Cascade<T> {
    inp: chan::Receiver<T>,
    finalize: crate::cascade::utils::FinalizeContainer<T>,
    outs: Vec<(
        Box<dyn Fn(T, bool) -> CascadeResultIntern<T> + Send + 'static>,
        bool,
    )>,
}

impl<T: Send + 'static> Cascade<T> {
    pub fn new(inp: chan::Receiver<T>) -> Self {
        Self {
            inp,
            finalize: None,
            outs: Vec::new(),
        }
    }
}

impl<T: Send + 'static> crate::cascade::Push<T> for Cascade<T> {
    fn push<O, F>(mut self, ev_out: O, keep_after_disconnect: bool, filter: F) -> Self
    where
        O: GenericQueueInterface<T> + Send + 'static,
        F: Fn(&T) -> bool + Send + 'static,
    {
        self.outs.push((
            Box::new(move |x: T, drop_if_match: bool| -> CascadeResultIntern<T> {
                if !filter(&x) {
                    CascadeResultIntern::Kept(x)
                } else if drop_if_match || ev_out.push(x) {
                    CascadeResultIntern::Forwarded
                } else if keep_after_disconnect {
                    CascadeResultIntern::ChangeToBlackhole
                } else {
                    CascadeResultIntern::Disconnected
                }
            }),
            false,
        ));
        self
    }

    fn push_map<R, O, F>(mut self, ev_out: O, keep_after_disconnect: bool, filtmap: F) -> Self
    where
        R: Send + 'static,
        O: GenericQueueInterface<R> + Send + 'static,
        F: Fn(T) -> Result<R, T> + Send + 'static,
    {
        self.outs.push((
            Box::new(move |x: T, drop_if_match: bool| -> CascadeResultIntern<T> {
                match filtmap(x).map(|i| drop_if_match || ev_out.push(i)) {
                    Err(x) => CascadeResultIntern::Kept(x),
                    Ok(true) => CascadeResultIntern::Forwarded,
                    Ok(false) => {
                        if keep_after_disconnect {
                            CascadeResultIntern::ChangeToBlackhole
                        } else {
                            CascadeResultIntern::Disconnected
                        }
                    }
                }
            }),
            false,
        ));
        self
    }

    fn set_finalize<F>(mut self, f: F) -> Self
    where
        F: Fn(Result<(), T>) -> bool + Send + 'static,
    {
        self.finalize = Some(Box::new(f));
        self
    }
}

impl<T: Send + 'static> CascadeTrait for Cascade<T> {
    fn register_input<'a>(&'a self, sel: &mut chan::Select<'a>) -> usize {
        sel.recv(&self.inp)
    }

    fn try_run<'a>(&self, oper: chan::SelectedOperation<'a>) -> Option<CleanupIndices> {
        let mut clx = CleanupIndices::new();
        let mut finalizer_arg: Result<(), T> = Ok(());

        match self
            .outs
            .iter()
            .enumerate()
            .try_fold(oper.recv(&self.inp).ok()?, |x, (n, i)| {
                match (i.0)(x, i.1) {
                    CascadeResultIntern::Kept(y) => Ok(y),
                    CascadeResultIntern::Forwarded => Err(None),
                    CascadeResultIntern::ChangeToBlackhole => Err(Some((n, true))),
                    CascadeResultIntern::Disconnected => Err(Some((n, false))),
                }
            }) {
            Err(Some((n, x))) => {
                clx.insert(n, x);
            }
            Ok(x) => {
                finalizer_arg = Err(x);
            }
            _ => {}
        }

        if self
            .finalize
            .as_ref()
            .map(|finalize| finalize(finalizer_arg))
            .unwrap_or(false)
        {
            clx.insert(self.outs.len(), false);
        }

        Some(clx)
    }

    fn cleanup(&mut self, clx: CleanupIndices) -> bool {
        crate::cascade::utils::cleanup(&mut self.outs, &mut self.finalize, clx)
    }

    fn is_outs_empty(&self) -> bool {
        self.outs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_event_listener() {
        let event = Queue::new();

        event.push(0i32);

        let subs = event.subscribe();
        let h = std::thread::spawn(move || {
            for i in [1, 2, 3].iter().copied() {
                assert_eq!(subs.recv(), Ok(i));
            }
        });

        event.extend([1, 2, 3].iter().copied());
        h.join().unwrap();
    }

    #[test]
    fn test_event_cleanup() {
        let event = Queue::new();

        let subs1 = event.subscribe();

        event.push(10i32);

        let subs2 = event.subscribe();

        event.push(20i32);

        let h1 = std::thread::spawn(move || {
            assert_eq!(subs1.recv(), Ok(10i32));
            assert_eq!(subs1.recv(), Ok(20i32));
        });
        let h2 = std::thread::spawn(move || {
            assert_eq!(subs2.recv(), Ok(20i32));
            std::thread::sleep(Duration::from_millis(400));
            for _i in 0..10 {
                assert_eq!(subs2.recv(), Ok(30i32));
            }
        });

        std::thread::sleep(Duration::from_millis(200));

        for _i in 0..10 {
            event.push(30i32);
        }

        h1.join().unwrap();
        h2.join().unwrap();
    }

    #[test]
    fn multiple_events() {
        let event1 = Queue::new();
        let event2 = Queue::new();

        let subs1 = event1.subscribe();
        let subs2 = event2.subscribe();

        event1.push(20i32);
        event2.push(10i32);

        chan::select! {
            recv(subs1) -> msg => {
                assert_eq!(msg, Ok(20i32));
            },
            recv(subs2) -> msg => {
                assert_eq!(msg, Ok(10i32));
            },
            default => panic!(),
        }

        chan::select! {
            recv(subs1) -> msg => {
                assert_eq!(msg, Ok(20i32));
            },
            recv(subs2) -> msg => {
                assert_eq!(msg, Ok(10i32));
            },
            default => panic!(),
        }
    }
}
