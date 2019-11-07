//! If you can, you should probably use `crossbeam_channel` instead
//! of `std::sync::mpsc`, because it's faster.
//! (enable the support for `crossbeam-channel` via the feature flag)

use crate::traits::GenericQueueInterface;

impl<T> GenericQueueInterface<T> for crate::BlackHole<T> {
    #[inline]
    fn push(&self, _event: T) -> bool {
        false
    }
}

impl<T> GenericQueueInterface<T> for std::sync::mpsc::Sender<T> {
    #[inline]
    fn push(&self, event: T) -> bool {
        self.send(event).is_ok()
    }
}

impl<T> GenericQueueInterface<T> for std::sync::mpsc::SyncSender<T> {
    #[inline]
    fn push(&self, event: T) -> bool {
        self.send(event).is_ok()
    }
}

#[cfg(feature = "crossbeam-channel")]
impl<T> GenericQueueInterface<T> for crossbeam_channel::Sender<T> {
    #[inline]
    fn push(&self, event: T) -> bool {
        self.send(event).is_ok()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        crossbeam_channel::Sender::is_empty(self)
    }
}

#[cfg(feature = "winit")]
impl<T> GenericQueueInterface<T> for winit::event_loop::EventLoopProxy<T> {
    #[inline]
    fn push(&self, event: T) -> bool {
        self.send_event(event).is_ok()
    }
}

#[cfg(all(test, feature = "crossbeam-channel", feature = "winit"))]
mod tests {
    #[allow(dead_code)]
    fn winit_cascade() {
        use crate::cascade::Push;
        let (_tx, casc) = crate::cascade::utils::unbounded();
        let eloop = winit::event_loop::EventLoop::<u32>::with_user_event();
        let proxy = eloop.create_proxy();
        casc.push(proxy, false, |_| true);
    }
}
