// async future-based API

use crate::traits::{EmitResult, Emitter, Listen, QueueInterfaceCommon, QueueInterfaceListable};
use futures_core::{
    stream::Stream,
    task::{Context, Waker},
};
use std::{
    borrow::Cow,
    cell::RefCell,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
};

mod direct;
mod wrapper;

pub use self::{direct::*, wrapper::*};

fn wake_all(wakers: &Mutex<Vec<Waker>>) {
    let mut lock = wakers.lock().unwrap();
    for i in std::mem::replace(&mut *lock, Vec::new()).into_iter() {
        i.wake();
    }
}

pub struct WakerWrapper<T> {
    waker: Option<Waker>,
    _phantom: PhantomData<T>,
}

impl<T> WakerWrapper<T> {
    #[inline]
    pub fn new(cx: Context<'_>) -> Self {
        Self {
            waker: Some(cx.waker().clone()),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn wake(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

impl<T> QueueInterfaceCommon for WakerWrapper<T> {
    type Item = T;
}

impl<T: Clone> crate::traits::EmitterMut for WakerWrapper<T> {
    #[inline]
    fn emit<'a>(&mut self, event: Cow<'a, T>) -> EmitResult<'a, T> {
        self.wake();
        Err(event)
    }
}
