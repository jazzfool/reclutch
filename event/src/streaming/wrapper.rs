use super::*;

#[derive(Clone, Debug, Default)]
pub struct QueueWrapper<IQ> {
    inner: IQ,
    wakers: Arc<Mutex<Vec<Waker>>>,
}

pub struct ListenerWrapper<IL: Listen> {
    inner: IL,
    buf: RefCell<Vec<<IL as Listen>::Item>>,
    wakers: std::sync::Weak<Mutex<Vec<Waker>>>,
}

impl<IQ> QueueWrapper<IQ> {
    pub fn new(inner: IQ) -> Self {
        Self { inner, wakers: Arc::new(Mutex::new(Vec::new())) }
    }

    pub fn into_inner(self) -> IQ {
        let QueueWrapper { inner, wakers } = self;
        wake_all(&wakers);
        inner
    }

    /// # Safety
    /// This method is marked unsafe because it might allow
    /// type contract-breaking behavior.
    /// It is forbidden to call [`emit`](crate::traits::Emitter::emit)
    /// or similar functions on the returned reference.
    pub unsafe fn inner_mut(&mut self) -> &mut IQ {
        &mut self.inner
    }
}

impl<IQ: QueueInterfaceCommon> QueueInterfaceCommon for QueueWrapper<IQ> {
    type Item = <IQ as QueueInterfaceCommon>::Item;

    fn buffer_is_empty(&self) -> bool {
        self.inner.buffer_is_empty()
    }
}

impl<IQ: Emitter> Emitter for QueueWrapper<IQ>
where
    IQ: Emitter,
    <IQ as QueueInterfaceCommon>::Item: Clone,
{
    fn emit<'a>(&self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        let ret = self.inner.emit(event);
        wake_all(&self.wakers);
        ret
    }
}

impl<IQ> QueueInterfaceListable for QueueWrapper<IQ>
where
    IQ: QueueInterfaceListable,
    <IQ as QueueInterfaceCommon>::Item: Clone + Unpin,
{
    type Listener = ListenerWrapper<<IQ as QueueInterfaceListable>::Listener>;

    fn listen(&self) -> Self::Listener {
        ListenerWrapper {
            inner: self.inner.listen(),
            buf: RefCell::new(Vec::new()),
            wakers: Arc::downgrade(&self.wakers),
        }
    }
}

impl<IL> Listen for ListenerWrapper<IL>
where
    IL: Listen,
    <IL as Listen>::Item: Clone,
{
    type Item = <IL as Listen>::Item;

    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Self::Item]) -> R,
    {
        // TODO: optimize this
        let mut buf = self.buf.borrow_mut();
        buf.extend(self.inner.peek().into_iter());
        let ret = f(&buf[..]);
        buf.clear();
        ret
    }
}

impl<IL> Stream for ListenerWrapper<IL>
where
    IL: Listen + Unpin,
    <IL as Listen>::Item: Clone + Unpin,
{
    type Item = <IL as Listen>::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // TODO: check if pinning projections might be appropriate
        // (to get rid of the 'IL: Unpin' requirement)
        let this = Pin::into_inner(self);
        let buf = this.buf.get_mut();

        // fetch new elements if needed
        if buf.is_empty() {
            *buf = this.inner.peek();
        }

        if !buf.is_empty() {
            return Poll::Ready(Some(buf.remove(0)));
        }
        match this.wakers.upgrade() {
            None => Poll::Ready(None),
            Some(wakers) => {
                wakers.lock().unwrap().push(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.buf.borrow().len(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::EmitterExt;

    #[test]
    fn futuristic() {
        let eq = QueueWrapper::new(crate::ts::Queue::default());
        let mut eql = eq.listen();
        let h = std::thread::spawn(move || {
            futures_executor::block_on(async {
                use futures_util::stream::StreamExt;
                let mut tmp = Vec::<u32>::new();
                while let Some(dat) = eql.next().await {
                    tmp.push(dat);
                }
                assert_eq!(tmp, [1, 2]);
            })
        });
        eq.emit_owned(1u32).into_result().unwrap();
        eq.emit_owned(2).into_result().unwrap();
        std::mem::drop(eq);
        h.join().unwrap();
    }
}
