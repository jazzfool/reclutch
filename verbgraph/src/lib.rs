use {
    reclutch_core::event::prelude::*,
    std::{cell::RefCell, collections::HashMap, ops::Deref, rc::Rc},
};

pub use paste;

/// An object which contains an `OptionVerbGraph` that can be accessed mutably.
pub trait HasVerbGraph: reclutch_core::widget::Widget + Sized + 'static {
    fn verb_graph(&mut self) -> &mut OptionVerbGraph<Self, Self::UpdateAux>;
}

/// An object which performs updates on it's own internal `VerbGraph`.
pub trait OperatesVerbGraph: reclutch_core::widget::Widget {
    fn update_all(&mut self, additional: &mut Self::UpdateAux);
    fn require_update(&mut self, additional: &mut Self::UpdateAux, tag: &'static str);
}

/// Helper type alias; `VerbGraph` is commonly stored in an `Option` to allow
/// referencing it's outer widget without violating borrow rules.
pub type OptionVerbGraph<T, A> = Option<VerbGraph<T, A>>;

/// Event which returns a string corresponding to the current event variant.
pub trait Event: Clone {
    fn get_key(&self) -> &'static str;
}

/// A queue handler not bound to any specific event queue.
#[derive(Clone)]
pub struct UnboundQueueHandler<T, A: 'static, E: Event> {
    handlers: HashMap<&'static str, Rc<RefCell<dyn FnMut(&mut T, &mut A, E)>>>,
}

impl<T, A, E: Event> Default for UnboundQueueHandler<T, A, E> {
    fn default() -> Self {
        UnboundQueueHandler { handlers: Default::default() }
    }
}

impl<T, A, E: Event> UnboundQueueHandler<T, A, E> {
    /// Creates a new, unbound queue handler.
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds a closure to be executed when an event of a specific key is matched.
    ///
    /// Also see [`event_key`](struct.Event.html#structmethod.get_key).
    pub fn on<'a>(
        &'a mut self,
        ev: &'static str,
        handler: impl FnMut(&mut T, &mut A, E) + 'static,
    ) -> &'a mut Self {
        self.handlers.insert(ev, Rc::new(RefCell::new(handler)));
        self
    }

    /// Binds the queue handler to a given event queue, thereby returning a regular, bound queue handler.
    pub fn bind<D: QueueInterfaceListable<Item = E, Listener = L>, L: EventListen<Item = E>>(
        self,
        queue: &impl Deref<Target = D>,
    ) -> QueueHandler<T, A, E, L> {
        QueueHandler { handlers: self.handlers, listener: queue.listen() }
    }
}

/// A queue handler containing a map of event keys to closures, bound to an event.
pub struct QueueHandler<T, A: 'static, E: Event, L: EventListen<Item = E>> {
    handlers: HashMap<&'static str, Rc<RefCell<dyn FnMut(&mut T, &mut A, E)>>>,
    listener: L,
}

impl<T, A, E: Event, L: EventListen<Item = E>> QueueHandler<T, A, E, L> {
    /// Creates a new queue handler, listening to a given event queue.
    pub fn new<D: QueueInterfaceListable<Item = E, Listener = L>>(
        queue: &impl Deref<Target = D>,
    ) -> Self {
        QueueHandler { handlers: HashMap::new(), listener: queue.listen() }
    }

    /// Adds a closure to be executed when an event of a specific key is matched.
    ///
    /// Also see [`event_key`](struct.Event.html#structmethod.get_key).
    pub fn on<'a>(
        &'a mut self,
        ev: &'static str,
        handler: impl FnMut(&mut T, &mut A, E) + 'static,
    ) -> &'a mut Self {
        self.handlers.insert(ev, Rc::new(RefCell::new(handler)));
        self
    }

    /// Same as [`on`](QueueHandler::on), however `self` is consumed and returned.
    pub fn and_on(
        mut self,
        ev: &'static str,
        handler: impl FnMut(&mut T, &mut A, E) + 'static,
    ) -> Self {
        self.on(ev, handler);
        self
    }
}

/// Implemented by queue handlers to execute the inner closures regardless of surrounding types.
pub trait DynQueueHandler<T, A> {
    /// Invokes the queue handler to peek events and match them.
    fn update(&mut self, obj: &mut T, additional: &mut A);
    /// Almost identical to `update`, however only the first `n` events are handled.
    fn update_n(&mut self, n: usize, obj: &mut T, additional: &mut A);
}

impl<T, A, E: Event, L: EventListen<Item = E>> DynQueueHandler<T, A> for QueueHandler<T, A, E, L> {
    fn update(&mut self, obj: &mut T, additional: &mut A) {
        let handlers = &mut self.handlers;
        self.listener.with(|events| {
            for event in events {
                if let Some(handler) = handlers.get_mut(event.get_key()) {
                    use std::ops::DerefMut;
                    let mut handler = handler.as_ref().borrow_mut();
                    (handler.deref_mut())(obj, additional, event.clone());
                }
            }
        });
    }

    fn update_n(&mut self, n: usize, obj: &mut T, additional: &mut A) {
        let handlers = &mut self.handlers;
        self.listener.with_n(n, |events| {
            for event in events {
                if let Some(handler) = handlers.get_mut(event.get_key()) {
                    use std::ops::DerefMut;
                    let mut handler = handler.as_ref().borrow_mut();
                    (handler.deref_mut())(obj, additional, event.clone());
                }
            }
        });
    }
}

/// Stores a list of queue handlers mapped to tags.
/// The tags facilitate jumping to specifc sections of other `VerbGraph`s, hence allowing for non-linear queue handling.
pub struct VerbGraph<T: 'static, A: 'static> {
    handlers: HashMap<&'static str, Vec<Box<dyn DynQueueHandler<T, A>>>>,
}

impl<T: 'static, A: 'static> Default for VerbGraph<T, A> {
    fn default() -> Self {
        VerbGraph { handlers: Default::default() }
    }
}

impl<T: 'static, A: 'static> VerbGraph<T, A> {
    /// Creates a new, empty verb graph.
    /// Synonymous to `Default::default()`.
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds a queue handler, associated with a tag.
    pub fn add<'a, E: Event + 'static, L: EventListen<Item = E> + 'static>(
        &'a mut self,
        tag: &'static str,
        handler: QueueHandler<T, A, E, L>,
    ) -> &'a mut Self {
        self.handlers.entry(tag).or_default().push(Box::new(handler));
        self
    }

    /// Same as [`add`](VerbGraph::add), however `self` is consumed and returned.
    pub fn and_add<E: Event + 'static, L: EventListen<Item = E> + 'static>(
        mut self,
        tag: &'static str,
        handler: QueueHandler<T, A, E, L>,
    ) -> Self {
        self.add(tag, handler);
        self
    }

    fn update_handlers(
        handlers: &mut [Box<dyn DynQueueHandler<T, A>>],
        obj: &mut T,
        additional: &mut A,
    ) {
        for handler in handlers {
            handler.update(obj, additional);
        }
    }

    /// Invokes all the queue handlers in a linear fashion, however non-linear jumping between verb graphs is still supported.
    pub fn update_all(&mut self, obj: &mut T, additional: &mut A) {
        for handler_list in self.handlers.values_mut() {
            VerbGraph::update_handlers(handler_list, obj, additional)
        }
    }

    /// Invokes the queue handlers for a specific tag.
    #[inline]
    pub fn update_tag(&mut self, obj: &mut T, additional: &mut A, tag: &'static str) {
        if let Some(handlers) = self.handlers.get_mut(tag) {
            VerbGraph::update_handlers(handlers, obj, additional)
        }
    }
}

fn update_obj_with<T, A, F>(obj: &mut T, additional: &mut A, f: F)
where
    T: HasVerbGraph<UpdateAux = A>,
    A: 'static,
    F: FnOnce(&mut VerbGraph<T, A>, &mut T, &mut A),
{
    if let Some(mut graph) = obj.verb_graph().take() {
        f(&mut graph, obj, additional);
        *obj.verb_graph() = Some(graph);
    }
}

/// Invokes the queue handler for a specific tag on a given object containing a verb graph.
#[inline]
pub fn require_update<T, A>(obj: &mut T, additional: &mut A, tag: &'static str)
where
    T: HasVerbGraph<UpdateAux = A>,
    A: 'static,
{
    update_obj_with(obj, additional, |graph, obj, additional| {
        graph.update_tag(obj, additional, tag)
    });
}

/// Invokes the queue handler for all tags on a given object containing a verb graph.
#[inline]
pub fn update_all<T, A>(obj: &mut T, additional: &mut A)
where
    T: HasVerbGraph<UpdateAux = A>,
    A: 'static,
{
    update_obj_with(obj, additional, VerbGraph::update_all);
}

/// Simplifies the syntax of creating a verb graph.
/// Example usage:
/// ```rust,ignore
/// reclutch_verbgraph::verbgraph! {
///     SomeObject as obj,
///     Aux as aux,
///
///     "tag" => event in &event_queue => {
///         event_key => {
///             println!("Handling 'event_key' for event `event_queue`, under the tag `tag`");
///         }
///     }
/// }
/// ```
/// Expands to:
/// ```rust,ignore
/// VerbGraph::new().add(
///     "tag",
///     QueueHandler::new(&event_queue).on(
///         "event_key",
///         |obj: &mut SomeObject, aux: &mut Aux, event| {
///             let event = event.unwrap_as_event_key();
///             {
///                 println!("Handling 'event_key' for event in 'event_queue' under the tag 'tag'");
///             }
///         },
///     ),
/// )
/// ```
#[macro_export]
macro_rules! verbgraph {
    ($ot:ty as $obj:ident,$at:ty as $add:ident, $($tag:expr => $eo:ident in $eq:expr=> {$($ev:tt => $body:block)*})*) => {{
        let mut graph = $crate::VerbGraph::new();
        $(
            let mut qh = $crate::QueueHandler::new($eq);
            $(
                qh.on(
                    std::stringify!($ev),
                    |$obj: &mut $ot, $add: &mut $at, #[allow(unused_variables)] $eo| {
                        #[allow(unused_variables)]
                        $crate::paste::expr!{
                            let $eo = $eo.[<unwrap_as_ $ev>]().unwrap();
                            $body
                        }
                    });
            )*
            graph.add($tag, qh);
        )*
        graph
    }};
}

/// Simplifies the syntax of creating an unbound queue handler.
///
/// # Example
/// ```ignore
/// unbound_queue_handler! {
///     SomeObject as obj,
///     Aux as aux,
///     EventType as event,
///
///     event_key => {
///         println!("Handling 'event_key' for an unknown event queue");
///     }
/// }
/// ```
/// Expands to:
/// ```ignore
/// UnboundQueueHandler::new().on(
///     |obj: &mut SomeObject, aux: &mut Aux, event: EventType, ctxt| {
///         let event = event.unwrap_as_event_key().unwrap();
///         {
///             println!("Handling 'event_key' for an unknown event queue");
///         }
///     },
/// )
/// ```
#[macro_export]
macro_rules! unbound_queue_handler {
    ($ot:ty as $obj:ident,$at:ty as $add:ident,$et:ty as $eo:ident,$($ev:tt => $body:block)*) => {{
        let mut qh = $crate::UnboundQueueHandler::new();
        $(
            qh.on(
                std::stringify!($ev),
                |$obj: &mut $ot, #[allow(unused_variables)] $add: &mut $at, $eo: $et| {
                    #[allow(unused_variables)]
                    $crate::paste::expr!{
                        let $eo = $eo.[<unwrap_as_ $ev>]().unwrap();
                        $body
                    }
                });
        )*
        qh
    }}
}

#[cfg(test)]
mod tests {
    use {super::*, reclutch_core::event::RcEventQueue};

    #[test]
    fn test_jumping() {
        #[derive(Clone)]
        struct EmptyEvent;

        impl Event for EmptyEvent {
            fn get_key(&self) -> &'static str {
                "empty"
            }
        }

        impl EmptyEvent {
            fn unwrap_as_empty(self) -> Option<()> {
                Some(())
            }
        }

        #[derive(Default)]
        struct Dependency {
            a: i32,
            b: i32,
            q: RcEventQueue<EmptyEvent>,
            g: OptionVerbGraph<Self, ()>,
        }

        impl reclutch_core::widget::Widget for Dependency {
            type UpdateAux = ();
            type GraphicalAux = ();
            type DisplayObject = ();
        }

        impl HasVerbGraph for Dependency {
            fn verb_graph(&mut self) -> &mut OptionVerbGraph<Self, ()> {
                &mut self.g
            }
        }

        #[derive(Default)]
        struct Root {
            dep: Dependency,
            q: RcEventQueue<EmptyEvent>,
        }

        let mut root = Root::default();

        let mut root_graph = verbgraph! {
            Root as obj,
            () as aux,
            "_" => event in &root.q => {
                empty => {
                    obj.dep.a += 1;
                    obj.dep.q.emit_owned(EmptyEvent);
                    require_update(&mut obj.dep, aux, "copy");
                }
            }
        };

        root.dep.g = verbgraph! {
            Dependency as obj,
            () as _aux,
            "copy" => event in &root.dep.q => {
                empty => {
                    obj.b = obj.a;
                }
            }
        }
        .into();

        for _ in 0..7 {
            root.q.emit_owned(EmptyEvent);
        }

        root_graph.update_all(&mut root, &mut ());

        // without ever explicitly updating `root.dep.g`, `obj.a` should still be copied to `obj.b`.

        assert_eq!(root.dep.a, root.dep.b);
        assert_eq!(root.dep.b, 7);
    }
}
