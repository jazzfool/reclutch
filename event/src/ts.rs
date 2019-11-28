use crate::*;
use std::sync::{Arc, RwLock};

pub type Queue<T> = Arc<RwLock<RawEventQueue<T>>>;

impl<T: Clone> QueueInterfaceListable for Queue<T> {
    type Listener = Listener<T>;

    #[inline]
    fn listen(&self) -> Listener<T> {
        Listener::new(Arc::clone(&self))
    }
}

#[derive(Debug)]
pub struct Listener<T> {
    pub(crate) key: ListenerKey,
    pub(crate) eq: Arc<RwLock<RawEventQueue<T>>>,
}

impl<T> EventListen for Listener<T> {
    type Item = T;

    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Self::Item]) -> R,
    {
        self.eq.write().ok().unwrap().pull_with(self.key, f)
    }
}

impl<T> Drop for Listener<T> {
    fn drop(&mut self) {
        if let Ok(mut eq) = self.eq.write() {
            eq.remove_listener(self.key);
        }
    }
}

impl<T> Listener<T> {
    pub(crate) fn new(eq: Arc<RwLock<RawEventQueue<T>>>) -> Self {
        let key = eq.write().unwrap().create_listener();
        Listener { key, eq }
    }
}
