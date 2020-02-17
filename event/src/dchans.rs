use crate::{
    cascade::{utils::CleanupIndices, CascadeTrait},
    traits::{Emitter, EmitterExt},
};
use crossbeam_channel as chan;

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

impl<T: Clone + Send + 'static> Cascade<T> {
    pub fn new(inp: chan::Receiver<T>) -> Self {
        Self {
            inp,
            finalize: None,
            outs: Vec::new(),
        }
    }
}

impl<T: Clone + Send + 'static> crate::cascade::Push for Cascade<T> {
    type Item = T;

    fn push<O, F>(mut self, ev_out: O, keep_after_disconnect: bool, filter: F) -> Self
    where
        O: Emitter<Item = T> + Send + 'static,
        F: Fn(&T) -> bool + Send + 'static,
    {
        self.outs.push((
            Box::new(move |x: T, drop_if_match: bool| -> CascadeResultIntern<T> {
                if !filter(&x) {
                    CascadeResultIntern::Kept(x)
                } else if drop_if_match || ev_out.emit_owned(x).was_delivered() {
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
        R: Clone + Send + 'static,
        O: Emitter<Item = R> + Send + 'static,
        F: Fn(T) -> Result<R, T> + Send + 'static,
    {
        self.outs.push((
            Box::new(move |x: T, drop_if_match: bool| -> CascadeResultIntern<T> {
                match filtmap(x).map(|i| drop_if_match || ev_out.emit_owned(i).was_delivered()) {
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
    use crate::traits::EmitterMutExt;
    use std::time::Duration;

    #[test]
    fn test_event_listener() {
        let (event, subs) = chan::unbounded();

        let h = std::thread::spawn(move || {
            for i in [1, 2, 3].iter().copied() {
                assert_eq!(subs.recv(), Ok(i));
            }
        });

        event.emit_owned(1).into_result().unwrap();
        event.emit_owned(2).into_result().unwrap();
        event.emit_owned(3).into_result().unwrap();
        h.join().unwrap();
    }

    #[test]
    fn test_event_cleanup() {
        let mut event = Vec::new();

        let (sender, subs1) = chan::unbounded();
        event.push(sender);

        event.emit_owned(10i32).into_result().unwrap();

        let (sender, subs2) = chan::unbounded();
        event.push(sender);

        event.emit_owned(20i32).into_result().unwrap();

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
            event.emit_owned(30i32).into_result().unwrap();
        }

        h1.join().unwrap();
        h2.join().unwrap();
    }

    #[test]
    fn multiple_events() {
        let (event1, subs1) = chan::unbounded();
        let (event2, subs2) = chan::unbounded();

        event1.emit_owned(20i32).into_result().unwrap();
        event2.emit_owned(10i32).into_result().unwrap();

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
