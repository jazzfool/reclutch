use {
    reclutch_event::prelude::*,
    std::{cell::RefCell, collections::HashMap, ops::Deref, rc::Rc},
};

pub use paste;

/// An object which contains an `OptionVerbGraph` that can be accessed immutably and mutably.
pub trait HasVerbGraph<A: 'static>: Sized + 'static {
    fn verb_graph(&self) -> &OptionVerbGraph<Self, A>;
    fn verb_graph_mut(&mut self) -> &mut OptionVerbGraph<Self, A>;
}

pub type OptionVerbGraph<T, A> = Option<VerbGraph<T, A>>;

/// Event which returns a string corresponding to the current event variant.
pub trait Event: Clone {
    fn get_key(&self) -> &'static str;
}

/// A queue handler not bound to any specific event queue.
#[derive(Clone)]
pub struct UnboundQueueHandler<T, A: 'static, E: Event> {
    handlers:
        HashMap<&'static str, Rc<RefCell<dyn FnMut(&mut T, &mut A, E, &mut VerbGraphContext<A>)>>>,
}

impl<T, A, E: Event> UnboundQueueHandler<T, A, E> {
    /// Creates a new, unbound queue handler.
    pub fn new() -> Self {
        UnboundQueueHandler { handlers: HashMap::new() }
    }

    /// Adds a closure to be executed when an event of a specific key is matched.
    ///
    /// Also see [`event_key`](struct.Event.html#structmethod.get_key).
    pub fn on(
        mut self,
        ev: &'static str,
        handler: impl FnMut(&mut T, &mut A, E, &mut VerbGraphContext<A>) + 'static,
    ) -> Self {
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
    handlers:
        HashMap<&'static str, Rc<RefCell<dyn FnMut(&mut T, &mut A, E, &mut VerbGraphContext<A>)>>>,
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
    pub fn on(
        mut self,
        ev: &'static str,
        handler: impl FnMut(&mut T, &mut A, E, &mut VerbGraphContext<A>) + 'static,
    ) -> Self {
        self.handlers.insert(ev, Rc::new(RefCell::new(handler)));
        self
    }
}

/// Implemented by queue handlers to execute the inner closures regardless of surrounding types.
trait DynQueueHandler<T, A> {
    /// Invokes the queue handler to peek events and match them.
    fn update(&mut self, obj: &mut T, additional: &mut A, context: &mut VerbGraphContext<A>);
}

impl<T, A, E: Event, L: EventListen<Item = E>> DynQueueHandler<T, A> for QueueHandler<T, A, E, L> {
    fn update(&mut self, obj: &mut T, additional: &mut A, context: &mut VerbGraphContext<A>) {
        for event in self.listener.peek() {
            if let Some(handler) = self.handlers.get_mut(event.get_key()) {
                use std::ops::DerefMut;
                let mut handler = handler.as_ref().borrow_mut();
                (handler.deref_mut())(obj, additional, event.clone(), context);
            }
        }
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
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds a queue handler, associated with a tag.
    pub fn add<E: Event + 'static, L: EventListen<Item = E> + 'static>(
        mut self,
        tag: &'static str,
        handler: QueueHandler<T, A, E, L>,
    ) -> Self {
        self.handlers.entry(tag).or_default().push(Box::new(handler));
        self
    }

    /// Invokes all the queue handlers in a linear fashion, however non-linear jumping between verb graphs is still supported.
    pub fn update_all(&mut self, obj: &mut T, additional: &mut A) {
        let mut ctxt = VerbGraphContext::<A>(Default::default());
        for (_, handler_list) in &mut self.handlers {
            for handler in handler_list {
                handler.update(obj, additional, &mut ctxt);
            }
        }
    }

    /// Invokes the queue handlers for a specific tag.
    #[inline]
    pub fn update_tag(&mut self, obj: &mut T, additional: &mut A, tag: &'static str) {
        self.update_tag_in_ctxt(
            obj,
            additional,
            tag,
            &mut VerbGraphContext::<A>(Default::default()),
        );
    }

    fn update_tag_in_ctxt(
        &mut self,
        obj: &mut T,
        additional: &mut A,
        tag: &'static str,
        ctxt: &mut VerbGraphContext<A>,
    ) {
        if let Some(handlers) = self.handlers.get_mut(tag) {
            for handler in handlers {
                handler.update(obj, additional, ctxt);
            }
        }
    }
}

/// Performs verb graph traversal during execution.
pub struct VerbGraphContext<A: 'static>(std::marker::PhantomData<A>);

impl<A: 'static> VerbGraphContext<A> {
    /// Invokes the queue handler for a specific tag on a given object containing a verb graph.
    pub fn require_update<T: HasVerbGraph<A>>(
        &mut self,
        obj: &mut T,
        additional: &mut A,
        tag: &'static str,
    ) {
        let mut graph = obj.verb_graph_mut().take().unwrap();
        graph.update_tag_in_ctxt(obj, additional, tag, self);
        *obj.verb_graph_mut() = Some(graph);
    }
}

/// Simplifies the syntax of creating a verb graph.
///
/// # Example
/// ```ignore
/// verbgraph! {
///     SomeObject as obj,
///     Aux as aux,
///     GraphContext as ctxt,
///
///     "tag" => event in &event_queue => {
///         event_key => {
///             println!("Handling 'event_key' for event `event_queue`, under the tag `tag`");   
///         }
///     }
/// }
/// ```
/// Expands to:
/// ```ignore
/// VerbGraph::new().add(
///     "tag",
///     QueueHandler::new(&event_queue).on(
///         "event_key",
///         |obj: &mut SomeObject, aux: &mut Aux, event, ctxt| {
///             let event = event.unwrap_as_event_key().unwrap();
///             {
///                 println!("Handling 'event_key' for event in 'event_queue' under the tag 'tag'");
///             }
///         },
///     ),
/// )
/// ```
#[macro_export]
macro_rules! verbgraph {
    ($ot:ty as $obj:ident,$at:ty as $add:ident,GraphContext as $ctxt:ident,$($tag:expr => $eo:ident in $eq:expr=>{$($ev:tt $body:block)*})*) => {{
        let mut graph = $crate::VerbGraph::new();
        $(
            let mut qh = $crate::QueueHandler::new($eq);
            $(
                qh = qh.on(
                    std::stringify!($ev),
                    |$obj: &mut $ot, $add: &mut $at, #[allow(unused_variables)] $eo, $ctxt| {
                        #[allow(unused_variables)]
                        $crate::paste::expr!{
                            let $eo = $eo.[<unwrap_as_ $ev>]().unwrap();
                            $body
                        }
                    });
            )*
            graph = graph.add($tag, qh);
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
///     GraphContext as ctxt,
///
///     event_key {
///         println!("Handling 'event_key' for an unknown event queue");
///     }
/// }
/// ```
/// Expands to:
/// ```ignore
/// UnboundQueueHandler::new().on(
///     "event_key",
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
    ($ot:ty as $obj:ident,$at:ty as $add:ident,$et:ty as $eo:ident,GraphContext as $ctxt:ident,$($ev:tt $body:block)*) => {{
        let mut qh = $crate::QueueHandler::new();
        $(
            qh = qh.on(
                std::stringify!($ev),
                |$obj: &mut $ot, #[allow(unused_variables)] $add: &mut $at, $eo: $et, $ctxt| {
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
