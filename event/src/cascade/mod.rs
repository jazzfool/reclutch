use crate::traits::GenericQueueInterface;
use crossbeam_channel as chan;

pub mod utils;

pub trait CascadeTrait: 'static + Send {
    /// Register the cascade input `Receiver`.
    /// This function must register exactly one `Receiver` and
    /// return the index (as returned from [Select::recv](crossbeam_channel::Select::recv)).
    fn register_input<'a>(&'a self, sel: &mut chan::Select<'a>) -> usize;

    /// Try to forward data incoming via the registered input `Receiver`.
    /// This function must call [`oper.recv`](crossbeam_channel::SelectedOperation::recv).
    ///
    /// # Return values
    /// * `Some([])`: nothing to do
    /// * `None`: channel closed -> drop this cascade
    /// * `Some([...])`: call [`cleanup`](CascadeTrait::cleanup) later with the data
    ///
    /// # Design
    /// The processing of incoming events is splitted into `try_run` and `cleanup`
    /// to bypass conflicting borrows between `self` and `oper`, because both
    /// hold read-only references to the same underlying memory (`Self`).
    /// Thus, `self` can't be a mutable reference because it would conflict with `'a`.
    fn try_run<'a>(&self, oper: chan::SelectedOperation<'a>) -> Option<utils::CleanupIndices>;

    /// This function is expected to be called with the unwrapped return value of
    /// [`try_run`](CascadeTrait::try_run) if it returned a non-empty value.
    fn cleanup(&mut self, clx: utils::CleanupIndices) -> bool;

    /// Returns if the cascade output filter count is null
    fn is_outs_empty(&self) -> bool;
}

pub trait Push<T>: CascadeTrait + Sized {
    /// Append a cascade output filter.
    /// Each event is forwarded (to `ev_out`) if `filter(&event) == true`.
    /// Processing of the event stops after the first matching filter.
    ///
    /// `keep_after_disconnect` specifies the behavoir of this `Cascade` item
    /// after `ev_out` signals that it won't accept new events.
    /// * `true`: the filter is left in the cascade, but will drop matching events
    ///   instead of forwarding
    /// * `false`: the filter is removed from the cascade, which is equivalent to
    ///   "`filter` which matches no events"
    fn push<O, F>(self, ev_out: O, keep_after_disconnect: bool, filter: F) -> Self
    where
        O: GenericQueueInterface<T> + Send + 'static,
        F: Fn(&T) -> bool + Send + 'static;

    /// This function extends the functionality of [`push`](Push::push)
    /// with the ability to cascade event queues with different
    /// underlying types.
    ///
    /// `filtmap` is expected to either return:
    /// * `Ok` to forward the event item to `ev_out`
    /// * `Err` to keep it in the cascade chain and continue with the next filter
    fn push_map<R, O, F>(self, ev_out: O, keep_after_disconnect: bool, filtmap: F) -> Self
    where
        R: Send + 'static,
        O: GenericQueueInterface<R> + Send + 'static,
        F: Fn(T) -> Result<R, T> + Send + 'static;

    /// Append a cascade output as notification queue.
    /// Every event which isn't matched by any preceding filters is cloned and
    /// pushed into the specified event output queue.
    fn push_notify<O>(self, ev_out: O) -> Self
    where
        T: Clone,
        O: GenericQueueInterface<T> + Clone + Send + Sync + 'static,
    {
        // we need a clonable O to perform the automatic clenaup
        self.push(ev_out.clone(), false, move |event| {
            ev_out.push((*event).clone());
            false
        })
    }

    /// Append a cascade output as notification queue.
    /// For each event which isn't matched by any preceding filters,
    /// an empty tuple `()` is pushed into the output queue.
    /// This is useful to wake up threads which aren't listening on the actual,
    /// but need to be notified when events pass through the cascade.
    /// Important note: The events which triggered the emission of the tokens
    /// possibily aren't available yet when the token is consumed.
    fn push_notify_via_token<O>(self, ev_out: O) -> Self
    where
        O: GenericQueueInterface<()> + Clone + Send + Sync + 'static,
    {
        // we need a clonable O to perform the automatic clenaup
        self.push_map(ev_out.clone(), false, move |event| {
            ev_out.push(());
            Err(event)
        })
    }

    /// Register a finalization function.
    /// The registered function will run after the event is either
    /// * dropped or forwarded --> argument will have value `Ok(())`
    /// * or completed the cascade with no matching filters
    ///   --> argument will have value `Err(event)`
    /// The order of the events is unspecified.
    ///
    /// The finalizer gets removed from the cascade if it returns false.
    fn set_finalize<F>(self, f: F) -> Self
    where
        F: Fn(Result<(), T>) -> bool + Send + 'static;

    /// Wraps the current instance into a `Box`
    fn wrap(self) -> Box<dyn CascadeTrait> {
        assert!(!self.is_outs_empty());
        Box::new(self)
    }
}

/// This function runs a cascade routing worker.
/// It expected an control channel (first argument, receiving end)
/// and an initial set of cascades (or `Vec::new()`) as arguments.
///
/// # Example
/// ```rust
/// let (ctrl_tx, ctrl_rx) = crossbeam_channel::bounded(0);
/// let h = std::thread::spawn(
///     move || reclutch_event::cascade::run_worker(ctrl_rx, Vec::new())
/// );
/// // do stuff, e.g. push new cascades via ctrl_tx
/// // teardown
/// std::mem::drop(ctrl_tx);
/// h.join();
/// ```
pub fn run_worker(
    ctrl: chan::Receiver<Box<dyn CascadeTrait>>,
    mut cascades: Vec<Box<dyn CascadeTrait>>,
) {
    loop {
        let mut sel = chan::Select::new();
        sel.recv(&ctrl);

        for i in cascades.iter() {
            i.register_input(&mut sel);
        }

        if let Some((real_idx, clx)) = loop {
            let oper = sel.select();
            let idx = oper.index();
            break if 0 == idx {
                match oper.recv(&ctrl) {
                    Err(_) => {
                        // stop signal
                        return;
                    }
                    Ok(x) => {
                        // new cascade
                        cascades.push(x);
                        None
                    }
                }
            } else {
                Some((
                    idx - 1,
                    match cascades.get(idx - 1).unwrap().try_run(oper) {
                        // channel closed
                        None => utils::CleanupIndices::new(),
                        // nothing to do
                        Some(ref x) if x.is_empty() => continue,
                        // cleanup needed
                        Some(x) => x,
                    },
                ))
            };
        } {
            // cleanup part
            if clx.is_empty() || cascades.get_mut(real_idx).unwrap().cleanup(clx) {
                cascades.remove(real_idx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{chans, dchans, traits::*};

    #[test]
    fn cascades_chan() {
        let ev1 = chans::Queue::new();
        let ev2 = chans::Queue::new();
        let ev3 = chans::Queue::new();
        let (stop_tx, stop_rx) = chan::bounded(0);
        let mut cascades = Vec::new();
        cascades.push(
            ev1.cascade()
                .push(ev2.clone(), false, |i| i % 2 == 1)
                .push(ev3.clone(), false, |_| true)
                .wrap(),
        );
        crossbeam_utils::thread::scope(move |s| {
            s.spawn(move |_| run_worker(stop_rx, cascades));
            let sub = ev2.listen_and_subscribe();
            let sub2 = ev3.listen_and_subscribe();
            ev1.push(2);
            ev1.push(1);
            assert_eq!(sub.notifier.recv(), Ok(()));
            assert_eq!(sub.listener.peek(), &[1]);
            assert_eq!(sub2.notifier.recv(), Ok(()));
            assert_eq!(sub2.listener.peek(), &[2]);
            std::mem::drop(stop_tx);
        })
        .unwrap();
    }

    #[test]
    fn cascades_dchan() {
        let ev1 = dchans::Queue::new();
        let ev2 = dchans::Queue::new();
        let ev3 = dchans::Queue::new();
        let (stop_tx, stop_rx) = chan::bounded(0);
        let mut cascades = Vec::new();
        cascades.push(
            ev1.cascade()
                .push(ev2.clone(), false, |i| i % 2 == 1)
                .push(ev3.clone(), false, |_| true)
                .wrap(),
        );
        crossbeam_utils::thread::scope(move |s| {
            s.spawn(move |_| run_worker(stop_rx, cascades));
            let sub = ev2.subscribe();
            let sub2 = ev3.subscribe();
            ev1.push(2);
            ev1.push(1);
            assert_eq!(sub.recv(), Ok(1));
            assert_eq!(sub2.recv(), Ok(2));
            std::mem::drop(stop_tx);
        })
        .unwrap();
    }

    #[test]
    fn runtime_cascade() {
        let ev1 = dchans::Queue::new();
        let ev2 = dchans::Queue::new();
        let ev3 = dchans::Queue::new();
        let (ctrl_tx, ctrl_rx) = chan::bounded(0);
        crossbeam_utils::thread::scope(move |s| {
            s.spawn(move |_| run_worker(ctrl_rx, Vec::new()));
            ctrl_tx
                .send(
                    ev1.cascade()
                        .push(ev2.clone(), false, |i| i % 2 == 1)
                        .push(ev3.clone(), false, |_| true)
                        .wrap(),
                )
                .unwrap();
            let sub = ev2.subscribe();
            let sub2 = ev3.subscribe();
            ev1.push(2);
            ev1.push(1);
            assert_eq!(sub.recv(), Ok(1));
            assert_eq!(sub2.recv(), Ok(2));
        })
        .unwrap();
    }

    #[test]
    fn cascade_map() {
        let ev1 = dchans::Queue::new();
        let ev2 = dchans::Queue::new();
        let ev3 = dchans::Queue::new();
        let (stop_tx, stop_rx) = chan::bounded(0);
        let mut cascades = Vec::new();
        cascades.push(
            ev1.cascade()
                .push(ev2.clone(), false, |i| i % 2 == 1)
                .push_map(ev3.clone(), false, |_| Ok(true))
                .wrap(),
        );
        crossbeam_utils::thread::scope(move |s| {
            s.spawn(move |_| run_worker(stop_rx, cascades));
            let sub = ev2.subscribe();
            let sub2 = ev3.subscribe();
            ev1.push(2);
            ev1.push(1);
            assert_eq!(sub.recv(), Ok(1));
            assert_eq!(sub2.recv(), Ok(true));
            std::mem::drop(stop_tx);
        })
        .unwrap();
    }

    #[test]
    fn cascade_internal_routing_low() {
        let ev1 = dchans::Queue::new();
        let (ev2_tx, ev2_rx) = chan::unbounded();
        let (evi_tx, evi_rx) = super::utils::unbounded(); // evi_rx is a cascade
        let mut cascades = Vec::new();
        cascades.push(ev1.cascade().push(evi_tx, false, |_| true).wrap());
        cascades.push(evi_rx.push(ev2_tx, false, |_| true).wrap());
        crossbeam_utils::thread::scope(move |s| {
            let (_stop_tx, stop_rx) = chan::bounded(0);
            s.spawn(move |_| run_worker(stop_rx, cascades));
            ev1.push(2);
            ev1.push(1);
            std::mem::drop(ev1);
            assert_eq!(ev2_rx.recv(), Ok(2));
            assert_eq!(ev2_rx.recv(), Ok(1));
            // the following only works when the 'evi' cascade is dropped
            assert_eq!(ev2_rx.recv(), Err(chan::RecvError));
        })
        .unwrap();
    }

    #[test]
    fn cascade_internal_routing_high() {
        let ev1 = dchans::Queue::new();
        let ev2 = dchans::Queue::new();
        let evi = dchans::Queue::new();
        let mut cascades = Vec::new();
        cascades.push(ev1.cascade().push(evi.clone(), false, |_| true).wrap());
        cascades.push(evi.cascade().push(ev2.clone(), false, |_| true).wrap());
        std::mem::drop(evi);
        crossbeam_utils::thread::scope(move |s| {
            let (_stop_tx, stop_rx) = chan::bounded(0);
            s.spawn(move |_| run_worker(stop_rx, cascades));
            let sub = ev2.subscribe();
            ev1.push(2);
            ev1.push(1);
            std::mem::drop(ev1);
            std::mem::drop(ev2);
            assert_eq!(sub.recv(), Ok(2));
            assert_eq!(sub.recv(), Ok(1));
            // the following only works when the 'evi' cascade is dropped
            assert_eq!(sub.recv(), Err(chan::RecvError));
        })
        .unwrap();
    }
}
