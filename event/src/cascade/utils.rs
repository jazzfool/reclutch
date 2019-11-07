/// maps cascade_out index to
/// * `true`: set destination = black hole
/// * `false`: drop filter
pub type CleanupIndices = std::collections::HashMap<usize, bool>;

pub type FinalizeContainer<T> = Option<Box<dyn Fn(Result<(), T>) -> bool + Send + 'static>>;

/// This function is a wrapper around the [`crossbeam-channel::unbounded`]
/// function, and returns a `(:GenericQueueInterface, :CascadeTrait)` tuple
pub fn unbounded<T: Send + 'static>() -> (crossbeam_channel::Sender<T>, crate::dchans::Cascade<T>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    (tx, crate::dchans::Cascade::new(rx))
}

/// This helper function is designed to be called inside of implementations
/// of [`CascadeTrait::cleanup`](super::CascadeTrait::cleanup)
pub fn cleanup<F, I>(
    outs: &mut Vec<(F, bool)>,
    finalize: &mut FinalizeContainer<I>,
    mut clx: CleanupIndices,
) -> bool {
    if clx.remove(&outs.len()).is_some() {
        // the finalizer is invalidated
        *finalize = None;
    }

    let mut any_active_out = finalize.is_some();
    *outs = std::mem::replace(outs, Vec::new())
        .into_iter()
        .enumerate()
        .filter_map(|(n, mut i)| {
            match clx.get(&n) {
                Some(&false) => return None,
                Some(&true) => i.1 = true,
                _ => {}
            }
            any_active_out |= !i.1;
            Some(i)
        })
        .collect();

    any_active_out
}
