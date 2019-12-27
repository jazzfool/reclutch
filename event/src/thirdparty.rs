//! If you can, you should probably use `crossbeam_channel` instead
//! of `std::sync::mpsc`, because it's faster.
//! (enable the support for `crossbeam-channel` via the feature flag)

use crate::{
    channels_api,
    traits::{EmitResult, Emitter, EmitterMut, EmitterMutExt, QueueInterfaceCommon},
};
use retain_mut::RetainMut;
use std::{
    borrow::Cow,
    cell::RefCell,
    ops::{Deref, DerefMut},
    sync::{mpsc, Arc, RwLock},
};

impl<Q> EmitterMut for Q
where
    Q: Emitter,
    Self::Item: Clone,
{
    #[inline(always)]
    fn emit<'a>(&mut self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        Emitter::emit(&*self, event)
    }
}

impl<T> QueueInterfaceCommon for std::marker::PhantomData<T> {
    type Item = T;
}

impl<T: Clone> Emitter for std::marker::PhantomData<T> {
    #[inline(always)]
    fn emit<'a>(&self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        EmitResult::Undelivered(event)
    }
}

impl<Q: QueueInterfaceCommon> QueueInterfaceCommon for [Q] {
    type Item = <Q as QueueInterfaceCommon>::Item;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.iter().all(|i| i.buffer_is_empty())
    }
}

impl<Q> EmitterMut for [Q]
where
    Q: EmitterMut,
    Self::Item: Clone,
{
    fn emit<'a>(&mut self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        if self.len() == 1 {
            self.first_mut().unwrap().emit(event)
        } else if self.iter_mut().any(|i| i.emit_borrowed(&*event).was_delivered()) {
            EmitResult::Delivered
        } else {
            EmitResult::Undelivered(event)
        }
    }
}

impl<Q: QueueInterfaceCommon> QueueInterfaceCommon for Vec<Q> {
    type Item = <Q as QueueInterfaceCommon>::Item;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.iter().all(|i| i.buffer_is_empty())
    }
}

impl<Q> EmitterMut for Vec<Q>
where
    Q: EmitterMut,
    Self::Item: Clone,
{
    fn emit<'a>(&mut self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        self.retain_mut(|i| i.emit_borrowed(&*event).was_delivered());
        if self.is_empty() {
            EmitResult::Undelivered(event)
        } else {
            EmitResult::Delivered
        }
    }
}

impl<T: Clone> QueueInterfaceCommon for Box<dyn EmitterMut<Item = T>> {
    type Item = T;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.deref().buffer_is_empty()
    }
}

impl<T: Clone> EmitterMut for Box<dyn EmitterMut<Item = T>> {
    #[inline]
    fn emit<'a>(&mut self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        self.deref_mut().emit(event)
    }
}

impl<Q: QueueInterfaceCommon> QueueInterfaceCommon for std::rc::Rc<Q> {
    type Item = <Q as QueueInterfaceCommon>::Item;

    #[inline(always)]
    fn buffer_is_empty(&self) -> bool {
        self.deref().buffer_is_empty()
    }
}

impl<Q> Emitter for std::rc::Rc<Q>
where
    Q: Emitter,
    Self::Item: Clone,
{
    #[inline(always)]
    fn emit<'a>(&self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        self.deref().emit(event)
    }
}

impl<Q: QueueInterfaceCommon> QueueInterfaceCommon for Arc<Q> {
    type Item = <Q as QueueInterfaceCommon>::Item;

    #[inline(always)]
    fn buffer_is_empty(&self) -> bool {
        self.deref().buffer_is_empty()
    }
}

impl<Q> Emitter for Arc<Q>
where
    Q: Emitter,
    Self::Item: Clone,
{
    #[inline(always)]
    fn emit<'a>(&self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        self.deref().emit(event)
    }
}

impl<Q: QueueInterfaceCommon> QueueInterfaceCommon for RefCell<Q> {
    type Item = <Q as QueueInterfaceCommon>::Item;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.borrow().buffer_is_empty()
    }
}

impl<Q> Emitter for RefCell<Q>
where
    Q: EmitterMut,
    Self::Item: Clone,
{
    #[inline]
    fn emit<'a>(&self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        self.borrow_mut().emit(event)
    }
}

impl<Q: QueueInterfaceCommon> QueueInterfaceCommon for RwLock<Q> {
    type Item = <Q as QueueInterfaceCommon>::Item;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.read().map(|i| i.buffer_is_empty()).unwrap_or(true)
    }
}

impl<Q> Emitter for RwLock<Q>
where
    Q: EmitterMut,
    Self::Item: Clone,
{
    #[inline]
    fn emit<'a>(&self, event: Cow<'a, Self::Item>) -> EmitResult<'a, Self::Item> {
        if let Ok(mut i) = self.write() {
            i.emit(event)
        } else {
            EmitResult::Undelivered(event)
        }
    }
}

impl<T> QueueInterfaceCommon for mpsc::Sender<T> {
    type Item = T;
}

impl<T: Clone> Emitter for mpsc::Sender<T> {
    #[inline]
    fn emit<'a>(&self, event: Cow<'a, T>) -> EmitResult<'a, T> {
        self.send(event.into_owned()).map_err(|mpsc::SendError(x)| Cow::Owned(x)).into()
    }
}

impl<T> QueueInterfaceCommon for mpsc::SyncSender<T> {
    type Item = T;
}

impl<T: Clone> Emitter for mpsc::SyncSender<T> {
    #[inline]
    fn emit<'a>(&self, event: Cow<'a, T>) -> EmitResult<'a, T> {
        self.send(event.into_owned()).map_err(|mpsc::SendError(x)| Cow::Owned(x)).into()
    }
}

channels_api! {
    impl<T> QueueInterfaceCommon for crossbeam_channel::Sender<T> {
        type Item = T;

        #[inline]
        fn buffer_is_empty(&self) -> bool {
            crossbeam_channel::Sender::is_empty(self)
        }
    }

    impl<T: Clone> Emitter for crossbeam_channel::Sender<T> {
        #[inline]
        fn emit<'a>(&self, event: Cow<'a, T>) -> EmitResult<'a, T> {
            self.send(event.into_owned()).map_err(|crossbeam_channel::SendError(x)| Cow::Owned(x)).into()
        }
    }
}

#[cfg(feature = "winit")]
impl<T> QueueInterfaceCommon for winit::event_loop::EventLoopProxy<T> {
    type Item = T;
}

#[cfg(feature = "winit")]
impl<T: Clone> Emitter for winit::event_loop::EventLoopProxy<T> {
    #[inline]
    fn emit<'a>(&self, event: Cow<'a, T>) -> EmitResult<'a, T> {
        self.send_event(event.into_owned())
            .map_err(|winit::event_loop::EventLoopClosed(x)| Cow::Owned(x))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::traits::EmitterMutExt;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn test_event_listener() {
        let mut event = Vec::new();

        event.emit_owned(0i32).to_result().unwrap_err();

        let (sender, receiver) = mpsc::channel();
        event.push(sender);

        let data = &[1, 2, 3];

        let h = std::thread::spawn(move || {
            for i in data {
                assert_eq!(receiver.recv(), Ok(*i));
            }
        });

        for i in data {
            event.emit_borrowed(i).to_result().unwrap();
        }
        h.join().unwrap();
    }

    #[test]
    fn test_event_cleanup() {
        let mut event = Vec::new();

        let (sender, subs1) = mpsc::channel();
        event.push(sender);

        event.emit_owned(10i32).to_result().unwrap();

        let (sender, subs2) = mpsc::channel();
        event.push(sender);

        event.emit_owned(20i32).to_result().unwrap();

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
            event.emit_owned(30i32).to_result().unwrap();
        }

        h1.join().unwrap();
        h2.join().unwrap();
    }

    #[cfg(all(feature = "crossbeam-channel", feature = "winit"))]
    #[allow(dead_code)]
    fn winit_cascade() {
        use crate::cascade::Push;
        let (_tx, casc) = crate::cascade::utils::unbounded();
        let eloop = winit::event_loop::EventLoop::<u32>::with_user_event();
        let proxy = eloop.create_proxy();
        casc.push(proxy, false, |_| true);
    }
}
