use crate::traits::{self, EmitResult};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, rc::Rc};

struct InnerRef<'parent, Tin, Tout> {
    inq: &'parent mut VecDeque<Tin>,
    outq: &'parent mut VecDeque<Tout>,
}

/// Non-thread-safe, reference-counted,
/// bidirectional event queue,
/// designed for `1:1` communication,
/// thus, it doesn't support multi-casting.
///
/// The first type parameter describes the
/// events which the primary peer receives,
/// the second type parameter describes the
/// events which the secondary peer receives.
#[derive(Clone, Debug)]
pub struct Queue<Tp, Ts>(pub(crate) Rc<RefCell<(VecDeque<Tp>, VecDeque<Ts>)>>);

/// The "other" end of the bidirectional [`Queue`](crate::bidir::Queue)
#[derive(Clone, Debug)]
pub struct Secondary<Tp, Ts>(Queue<Tp, Ts>);

impl<Tp, Ts> Default for Queue<Tp, Ts> {
    fn default() -> Self {
        Queue(Rc::new(RefCell::new((VecDeque::new(), VecDeque::new()))))
    }
}

impl<Tp, Ts> Queue<Tp, Ts> {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// This function returns the "other" end of the bidirectional `Queue`
    ///
    /// NOTE: multiple calls to this method on the same queue
    /// return wrapped references to the same [`Secondary`](crate::bidir::Secondary).
    #[inline]
    pub fn secondary(&self) -> Secondary<Tp, Ts> {
        Secondary(Queue(Rc::clone(&self.0)))
    }

    fn on_queues_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(InnerRef<Tp, Ts>) -> R,
    {
        let inner = &mut *self.0.borrow_mut();
        f(InnerRef { inq: &mut inner.0, outq: &mut inner.1 })
    }
}

impl<Tp, Ts> Secondary<Tp, Ts> {
    fn on_queues_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(InnerRef<Ts, Tp>) -> R,
    {
        let inner = &mut *(self.0).0.borrow_mut();
        f(InnerRef { inq: &mut inner.1, outq: &mut inner.0 })
    }
}

macro_rules! impl_queue_part {
    ($strucn:ident, $tp1:ident, $tp2:ident, $tin:ident, $tout:ident) => {
        impl<$tp1, $tp2> $strucn<$tp1, $tp2> {
            /// Function which iterates over the input event queue
            /// and optionally schedules items to be put into the
            /// outgoing event queue
            pub fn bounce<F>(&self, f: F)
            where
                F: FnMut($tin) -> Option<$tout>,
            {
                self.on_queues_mut(|x| x.outq.extend(std::mem::replace(x.inq, VecDeque::new()).into_iter().flat_map(f)))
            }

            /// This function retrieves the newest event from
            /// the event queue and drops the rest.
            pub fn retrieve_newest(&self) -> Option<$tin> {
                self.on_queues_mut(|x| x.inq.drain(..).last())
            }
        }

        impl<$tp1, $tp2> traits::QueueInterfaceCommon for $strucn<$tp1, $tp2> {
            type Item = $tout;

            #[inline]
            fn buffer_is_empty(&self) -> bool {
                self.on_queues_mut(|x| x.outq.is_empty())
            }
        }

        impl<$tin, $tout: Clone> traits::Emitter for $strucn<$tp1, $tp2> {
            #[inline]
            fn emit<'a>(&self, event: Cow<'a, $tout>) -> EmitResult<'a, $tout> {
                self.on_queues_mut(|x| x.outq.push_back(event.into_owned()));
                EmitResult::Delivered
            }
        }

        impl<$tin: Clone, $tout> traits::Listen for $strucn<$tp1, $tp2> {
            type Item = $tin;

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
                self.on_queues_mut(|x| std::mem::replace(x.inq, VecDeque::new()).iter().map(f).collect())
            }

            #[inline]
            fn peek(&self) -> Vec<Self::Item> {
                self.on_queues_mut(|x| std::mem::replace(x.inq, VecDeque::new()).into_iter().collect())
            }
        }
    }
}

impl_queue_part!(Queue, Tp, Ts, Tp, Ts);
impl_queue_part!(Secondary, Tp, Ts, Ts, Tp);

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
