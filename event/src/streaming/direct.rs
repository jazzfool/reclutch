use super::*;

#[derive(Debug)]
pub struct Queue<T> {
    eq: crate::ts::Queue<T>,
    wakers: Arc<Mutex<Vec<Waker>>>,
}

#[derive(Debug)]
pub struct Listener<T> {
    inner: Arc<crate::ts::Listener<T>>,
    wakers: std::sync::Weak<Mutex<Vec<Waker>>>,
}

#[derive(Debug)]
pub struct IndirectRef<T>(Arc<crate::ts::Listener<T>>);

#[derive(Debug)]
pub struct Ref<'parent, T> {
    eq: std::sync::RwLockReadGuard<'parent, crate::RawEventQueue<T>>,
    key: crate::intern::ListenerKey,
}

impl<T> Clone for Queue<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self { eq: Arc::clone(&self.eq), wakers: Arc::clone(&self.wakers) }
    }
}

impl<T> Default for Queue<T> {
    #[inline]
    fn default() -> Self {
        Self { eq: Default::default(), wakers: Arc::new(Mutex::new(Vec::new())) }
    }
}

impl<T> QueueInterfaceCommon for Queue<T> {
    type Item = T;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.eq.buffer_is_empty()
    }
}

impl<T: Clone> Emitter for Queue<T> {
    fn emit<'a>(&self, event: Cow<'a, T>) -> EmitResult<'a, T> {
        let ret = self.eq.emit(event);
        wake_all(&self.wakers);
        ret
    }
}

impl<T: Clone> QueueInterfaceListable for Queue<T> {
    type Listener = Listener<T>;

    fn listen(&self) -> Self::Listener {
        Listener { inner: Arc::new(self.eq.listen()), wakers: Arc::downgrade(&self.wakers) }
    }
}

impl<T> Listen for Listener<T> {
    type Item = T;

    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        self.inner.with(f)
    }
}

impl<T: Unpin> Stream for Listener<T> {
    type Item = IndirectRef<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<IndirectRef<T>>> {
        let this = Pin::into_inner(self);
        let inner = &this.inner;

        match inner.eq.read().ok() {
            Some(eq) if eq.peek_get(inner.key).is_some() => {
                Poll::Ready(Some(IndirectRef(Arc::clone(inner))))
            }
            _ => {
                if let Some(wakers) = this.wakers.upgrade() {
                    wakers.lock().unwrap().push(cx.waker().clone());
                    Poll::Pending
                } else {
                    Poll::Ready(None)
                }
            }
        }
    }
}

impl<T> IndirectRef<T> {
    pub fn lock(
        &self,
    ) -> Result<
        Ref<'_, T>,
        std::sync::PoisonError<std::sync::RwLockReadGuard<'_, crate::RawEventQueue<T>>>,
    > {
        self.0.eq.read().map(|eq| Ref { eq, key: self.0.key })
    }
}

impl<T> std::ops::Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.eq.peek_get(self.key).unwrap()
    }
}

impl<T> Drop for IndirectRef<T> {
    #[inline]
    fn drop(&mut self) {
        if let Ok(mut eq) = self.0.eq.write() {
            eq.peek_finish(self.0.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::EmitterExt;

    #[test]
    fn futuristic() {
        let eq = Queue::new();
        let mut eql = eq.listen();
        let h = std::thread::spawn(move || {
            futures_executor::block_on(async {
                use futures_util::stream::StreamExt;
                let mut tmp = Vec::<u32>::new();
                while let Some(dat) = eql.next().await {
                    tmp.push(*dat.lock().unwrap());
                }
                assert_eq!(tmp, [1, 2]);
            })
        });
        eq.emit_owned(1u32).to_result().unwrap();
        eq.emit_owned(2).to_result().unwrap();
        std::mem::drop(eq);
        h.join().unwrap();
    }
}
