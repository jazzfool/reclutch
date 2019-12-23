use crate::traits::{self, EmitResult};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, rc::Rc};

/// Non-thread-safe, reference-counted,
/// bidirectional event queue,
/// designed for `1:1` communication,
/// thus, it doesn't support multicasting.
///
/// The first type parameter describes the
/// events which the primary peer receives,
/// the second type parameter describes the
/// events which the secondary peer receives.
#[derive(Clone, Debug)]
pub struct Queue<Tp, Ts>(pub(crate) Rc<RefCell<(VecDeque<Tp>, VecDeque<Ts>)>>);

/// The "other" end of the bidirectional [`Queue`]
#[derive(Clone, Debug)]
pub struct Secondary<Tp, Ts>(Queue<Tp, Ts>);

impl<Tp, Ts> Default for Queue<Tp, Ts> {
    fn default() -> Self {
        Queue(Rc::new(RefCell::new((VecDeque::new(), VecDeque::new()))))
    }
}

// TODO(zserik): introduce some kind of QueueProxy which transparently
// swaps Tp & Ts to avoid code duplication between [`Queue`] and [`Secondary`].

impl<Tp, Ts> Queue<Tp, Ts> {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// This function returns the "other" end of the bidirectional `Queue`
    ///
    /// NOTE: multiple calls to this method on the same queue
    /// return wrapped references to the same [`Secondary`].
    #[inline]
    pub fn secondary(&self) -> Secondary<Tp, Ts> {
        Secondary(Queue(Rc::clone(&self.0)))
    }

    /// This function iterates over the input event queue
    /// and optionally schedules items to be put into the
    /// outgoing event queue
    pub fn bounce<F>(&self, f: F)
    where
        F: FnMut(Tp) -> Option<Ts>,
    {
        let mut inner = self.0.borrow_mut();
        let inner = &mut *inner;
        let (inevq, outevq) = (&mut inner.0, &mut inner.1);
        outevq.extend(std::mem::replace(inevq, VecDeque::new()).into_iter().flat_map(f))
    }

    /// This function retrieves the newest event from
    /// the event queue and drops the rest.
    pub fn retrieve_newest(&self) -> Option<Tp> {
        self.0.borrow_mut().0.drain(..).last()
    }
}

impl<Tp, Ts> Secondary<Tp, Ts> {
    /// Function which iterates over the input event queue
    /// and optionally schedules items to be put into the
    /// outgoing event queue
    pub fn bounce<F>(&self, f: F)
    where
        F: FnMut(Ts) -> Option<Tp>,
    {
        let mut inner = (self.0).0.borrow_mut();
        let inner = &mut *inner;
        let (inevq, outevq) = (&mut inner.1, &mut inner.0);
        outevq.extend(std::mem::replace(inevq, VecDeque::new()).into_iter().flat_map(f))
    }

    /// This function retrieves the newest event from
    /// the event queue and drops the rest.
    pub fn retrieve_newest(&self) -> Option<Ts> {
        (self.0).0.borrow_mut().1.drain(..).last()
    }
}

impl<Tp, Ts> traits::QueueInterfaceCommon for Queue<Tp, Ts> {
    type Item = Ts;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        self.0.borrow().1.is_empty()
    }
}

impl<Tp, Ts> traits::QueueInterfaceCommon for Secondary<Tp, Ts> {
    type Item = Tp;

    #[inline]
    fn buffer_is_empty(&self) -> bool {
        (self.0).0.borrow().0.is_empty()
    }
}

impl<Tp, Ts: Clone> traits::Emitter for Queue<Tp, Ts> {
    #[inline]
    fn emit<'a>(&self, event: Cow<'a, Ts>) -> EmitResult<'a, Ts> {
        self.0.borrow_mut().1.push_back(event.into_owned());
        EmitResult::Delivered
    }
}

impl<Tp: Clone, Ts> traits::Emitter for Secondary<Tp, Ts> {
    #[inline]
    fn emit<'a>(&self, event: Cow<'a, Tp>) -> EmitResult<'a, Tp> {
        (self.0).0.borrow_mut().0.push_back(event.into_owned());
        EmitResult::Delivered
    }
}

impl<Tp: Clone, Ts> traits::Listen for Queue<Tp, Ts> {
    type Item = Tp;

    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Self::Item]) -> R,
    {
        f(&self.peek()[..])
    }

    #[inline]
    fn map<F, R>(&self, f: F) -> Vec<R>
    where
        F: FnMut(&Self::Item) -> R,
    {
        std::mem::replace(&mut self.0.borrow_mut().0, VecDeque::new()).iter().map(f).collect()
    }

    #[inline]
    fn peek(&self) -> Vec<Self::Item> {
        std::mem::replace(&mut self.0.borrow_mut().0, VecDeque::new()).into_iter().collect()
    }
}

impl<Tp, Ts: Clone> traits::Listen for Secondary<Tp, Ts> {
    type Item = Ts;

    #[inline]
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Self::Item]) -> R,
    {
        f(&self.peek()[..])
    }

    #[inline]
    fn map<F, R>(&self, f: F) -> Vec<R>
    where
        F: FnMut(&Self::Item) -> R,
    {
        std::mem::replace(&mut (self.0).0.borrow_mut().1, VecDeque::new()).iter().map(f).collect()
    }

    #[inline]
    fn peek(&self) -> Vec<Self::Item> {
        std::mem::replace(&mut (self.0).0.borrow_mut().1, VecDeque::new()).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn test_bidir_evq() {
        let primary = super::Queue::new();
        let secondary = primary.secondary();

        primary.emit_owned(1);
        assert_eq!(secondary.peek(), &[1]);
        primary.emit_owned(2);
        primary.emit_owned(3);
        assert_eq!(secondary.peek(), &[2, 3]);

        secondary.emit_owned(4);
        secondary.emit_owned(5);
        secondary.emit_owned(6);

        primary.bounce(|x| Some(x + 1));
        assert_eq!(secondary.peek(), &[5, 6, 7]);
    }
}
