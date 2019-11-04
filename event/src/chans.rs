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
    subscribers: Vec<chan::Sender<()>>,
}

#[derive(Debug)]
pub struct CombinedListener<T> {
    pub listener: Listener<T>,
    pub notifier: chan::Receiver<()>,
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

    fn create_channel() -> (chan::Sender<()>, chan::Receiver<()>) {
        chan::bounded(1)
    }

    pub fn subscribe(&self) -> chan::Receiver<()> {
        let (tx, rx) = Self::create_channel();
        self.with_inner_mut(move |inner| inner.subscribers.push(tx));
        rx
    }

    pub fn listen_and_subscribe(&self) -> CombinedListener<T> {
        let (tx, rx) = Self::create_channel();
        let id = self.with_inner_mut(move |inner| {
            inner.subscribers.push(tx);
            inner.ev.create_listener()
        });
        CombinedListener {
            listener: Listener(id, self.clone()),
            notifier: rx,
        }
    }

    pub fn cascade(&self) -> Cascade<T>
    where
        T: Send + 'static,
    {
        let CombinedListener { listener, notifier } = self.listen_and_subscribe();
        Cascade {
            listener,
            notifier,
            finalize: None,
            outs: Vec::new(),
        }
    }
}

impl<T> Intern<T> {
    fn notify(&mut self) {
        self.subscribers.retain(|i| {
            // try to send token
            if let Err(chan::TrySendError::Disconnected(())) = i.try_send(()) {
                // channel unsubscribed
                false
            } else {
                // channel works
                true
            }
        });
    }
}

impl<T> GenericQueueInterface<T> for Queue<T> {
    #[inline]
    fn push(&self, event: T) -> bool {
        self.with_inner_mut(|inner| {
            if inner.ev.listeners.is_empty() {
                return false;
            }
            inner.ev.events.push(event);
            inner.notify();
            true
        })
    }

    #[inline]
    fn extend<I>(&self, events: I) -> bool
    where
        I: IntoIterator<Item = T>,
    {
        self.with_inner_mut(|inner| {
            if inner.ev.listeners.is_empty() {
                return false;
            }
            inner.ev.events.extend(events);
            inner.notify();
            true
        })
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.with_inner(|inner| inner.ev.events.is_empty())
    }
}

impl<T> QueueInterface<T> for Queue<T> {
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

pub struct Cascade<T> {
    listener: Listener<T>,
    notifier: chan::Receiver<()>,
    finalize: crate::cascade::utils::FinalizeContainer<T>,
    outs: Vec<(
        Box<dyn Fn(&mut Vec<T>, bool) -> Result<(), bool> + Send + 'static>,
        bool,
    )>,
}

impl<T: Clone + Send + Sync + 'static> crate::cascade::Push<T> for Cascade<T> {
    fn push<O, F>(mut self, ev_out: O, keep_after_disconnect: bool, filter: F) -> Self
    where
        O: GenericQueueInterface<T> + Send + 'static,
        F: Fn(&T) -> bool + Send + 'static,
    {
        self.outs.push((
            Box::new(
                move |x: &mut Vec<T>, drop_if_match: bool| -> Result<(), bool> {
                    let (forward, keep) = std::mem::replace(x, Vec::new())
                        .into_iter()
                        .partition(|x| filter(x));
                    *x = keep;
                    if drop_if_match || ev_out.extend(forward) {
                        Ok(())
                    } else {
                        Err(keep_after_disconnect)
                    }
                },
            ),
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
            Box::new(
                move |x: &mut Vec<T>, drop_if_match: bool| -> Result<(), bool> {
                    let mut keep = Vec::<T>::new();
                    let mut forward = Vec::<R>::new();

                    for i in std::mem::replace(x, Vec::new()) {
                        match filtmap(i) {
                            Ok(x) => forward.push(x),
                            Err(x) => keep.push(x),
                        }
                    }

                    *x = keep;

                    if drop_if_match || ev_out.extend(forward) {
                        Ok(())
                    } else {
                        Err(keep_after_disconnect)
                    }
                },
            ),
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

impl<T: Clone + Send + Sync + 'static> CascadeTrait for Cascade<T> {
    fn register_input<'a>(&'a self, sel: &mut chan::Select<'a>) -> usize {
        sel.recv(&self.notifier)
    }

    fn try_run<'a>(&self, oper: chan::SelectedOperation<'a>) -> Option<CleanupIndices> {
        oper.recv(&self.notifier).ok()?;

        // check this here to make sure that
        // we have no race condition between `try_fold` and `strong_count`
        let is_last_ref = Arc::strong_count(&(self.listener.1).0) == 1;
        let mut clx = CleanupIndices::new();

        let events = self.listener.peek();
        let eventcnt = events.len();
        let rest = self
            .outs
            .iter()
            .enumerate()
            .try_fold(events, |mut x, (n, i)| {
                if let Err(y) = (i.0)(&mut x, i.1) {
                    clx.insert(n, y);
                }
                if x.is_empty() {
                    None
                } else {
                    Some(x)
                }
            });

        if let Some(ref finalize) = &self.finalize {
            let restlen = rest.as_ref().map(|x| x.len()).unwrap_or(0);

            // first process all unmatched events
            let mut finalize_fail = false;
            if let Some(rest) = rest {
                finalize_fail = rest
                    .into_iter()
                    .try_fold((), |(), i| if finalize(Err(i)) { Some(()) } else { None })
                    .is_some();
            }

            if !finalize_fail {
                // then process all matched events
                for _i in restlen..eventcnt {
                    if !finalize(Ok(())) {
                        finalize_fail = true;
                        break;
                    }
                }
            }
            if finalize_fail {
                clx.insert(self.outs.len(), false);
            }
        }

        if is_last_ref {
            // we are the only reference to this event
            // the event queue is now dead
            None
        } else {
            Some(clx)
        }
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

        let suls = event.listen_and_subscribe();
        let h = std::thread::spawn(move || {
            assert_eq!(suls.notifier.recv(), Ok(()));
            assert_eq!(suls.listener.peek(), &[1, 2, 3]);
        });

        event.extend([1, 2, 3].iter().copied());
        h.join().unwrap();
    }

    #[test]
    fn test_event_cleanup() {
        let event = Queue::new();

        let suls1 = event.listen_and_subscribe();

        event.push(10i32);

        assert!(!event.is_empty());

        let suls2 = event.listen_and_subscribe();

        event.push(20i32);

        let h1 = std::thread::spawn(move || {
            assert_eq!(suls1.notifier.recv(), Ok(()));
            assert_eq!(suls1.listener.peek(), &[10i32, 20i32]);
        });
        let h2 = std::thread::spawn(move || {
            assert_eq!(suls2.notifier.recv(), Ok(()));
            assert_eq!(suls2.listener.peek(), &[20i32]);
            assert_eq!(suls2.listener.peek(), &[]);
            assert_eq!(suls2.listener.peek(), &[]);
            std::thread::sleep(Duration::from_millis(400));
            assert_eq!(suls2.notifier.recv(), Ok(()));
            assert_eq!(suls2.listener.peek(), &[30i32; 10]);
        });

        std::thread::sleep(Duration::from_millis(200));
        assert!(event.is_empty());

        for _i in 0..10 {
            event.push(30i32);
        }

        h1.join().unwrap();
        h2.join().unwrap();

        assert!(event.is_empty());
    }

    #[test]
    fn multiple_events() {
        let event1 = Queue::new();
        let event2 = Queue::new();

        let suls1 = event1.listen_and_subscribe();
        let suls2 = event2.listen_and_subscribe();

        event1.push(20i32);
        event2.push(10i32);

        chan::select! {
            recv(suls1.notifier) -> _msg => {
                assert_eq!(suls1.listener.peek(), &[20i32]);
            },
            recv(suls2.notifier) -> _msg => {
                assert_eq!(suls2.listener.peek(), &[10i32]);
            },
            default => panic!(),
        }

        chan::select! {
            recv(suls1.notifier) -> _msg => {
                assert_eq!(suls1.listener.peek(), &[20i32]);
            },
            recv(suls2.notifier) -> _msg => {
                assert_eq!(suls2.listener.peek(), &[10i32]);
            },
            default => panic!(),
        }
    }
}
