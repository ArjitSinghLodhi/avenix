use crate::{
    events::{Event, EventBuffer, EventReader, EventWriter},
    resources::ParallelResourceAccessor,
    system::{ParallelSystemParam, SystemMeta, SystemParam},
    world::World,
};

/// A thread-safe lock-free, thread-clonable handle that acts as a detached remote input for dispatching events.
///
/// `ParallelEventWriter` can be passed to external or background worker threads, allowing them to
/// concurrently dispatch events outside the main execution path. It can be obtained directly from
/// the world layer via [`.par_event_writer()`].
///
/// Also usable as a system param.
///
/// [`.par_event_writer()`]: crate::world::World::par_event_writer
pub struct ParallelEventWriter<T: Event> {
    pub(crate) event_buffer_accessor: ParallelResourceAccessor<EventBuffer<T>>,
}

impl<T: Event> Clone for ParallelEventWriter<T> {
    fn clone(&self) -> Self {
        Self {
            event_buffer_accessor: self.event_buffer_accessor.clone(),
        }
    }
}

impl<T: Event> ParallelEventWriter<T> {
    /// Creates a temporary [`EventWriter`] context bound to the current scope block.
    ///
    /// This allows external or parallel tasks to safely dispatch events using the engine's
    /// standard event pipeline syntax via an inner closure.
    pub fn scope<F, R>(&self, f: F) -> R
    where
        F: for<'b> FnOnce(EventWriter<'b, T>) -> R,
    {
        self.event_buffer_accessor.scope(|buffer| {
            let writer = EventWriter { buffer };
            f(writer)
        })
    }
}

impl<T: Event> SystemParam for ParallelEventWriter<T> {
    type Item<'world> = ParallelEventWriter<T>;

    fn init_access(_system_meta: &mut SystemMeta) {}

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let event_buffer_accessor = world.par_resource_accessor::<EventBuffer<T>>();
        Some(ParallelEventWriter {
            event_buffer_accessor,
        })
    }
}

unsafe impl<T: Event> Send for ParallelEventWriter<T> {}
unsafe impl<T: Event> Sync for ParallelEventWriter<T> {}
unsafe impl<T: Event> ParallelSystemParam for ParallelEventWriter<T> {}

/// A thread-safe lock-free, thread-clonable handle that acts as a detached remote reader for inspecting events.
///
/// `ParallelEventReader` can be passed to external or background worker threads, allowing them to
/// concurrently inspect dispatched events outside the main execution path. It can be obtained directly
/// from the world layer via [`.par_event_reader()`].
///
/// # Critical Synchronization Warning
///
/// Because events follow the engine's global 3-frame lifecycle, **they are unconditionally cleared on frame 3
/// regardless of system execution.**
///
/// When using this handle on external threads, you must implement manual synchronization with the main ECS
/// loop. If the main application ticks too fast or advances frames before your parallel thread processes
/// the current window, the engine will clear the underlying buffer, causing the handle to miss data entirely.
///
/// Also usable as a system param.
///
/// [`.par_event_reader()`]: crate::world::World::par_event_reader
pub struct ParallelEventReader<T: Event> {
    pub(crate) event_buffer_accessor: ParallelResourceAccessor<EventBuffer<T>>,
}

impl<T: Event> Clone for ParallelEventReader<T> {
    fn clone(&self) -> Self {
        Self {
            event_buffer_accessor: self.event_buffer_accessor.clone(),
        }
    }
}

impl<T: Event> ParallelEventReader<T> {
    /// Creates a temporary [`EventReader`] wrapper context bound to the current scope block.
    ///
    /// This allows external or parallel tasks to safely iterate over events using the engine's
    /// standard event pipeline syntax via an inner closure.
    pub fn scope<F, R>(&self, f: F) -> R
    where
        F: for<'b> FnOnce(EventReader<'b, T>) -> R,
    {
        self.event_buffer_accessor.scope(|buffer| {
            let event_reader = EventReader { buffer };
            f(event_reader)
        })
    }
}

impl<T: Event> SystemParam for ParallelEventReader<T> {
    type Item<'world> = ParallelEventReader<T>;

    fn init_access(_system_meta: &mut SystemMeta) {}

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let event_buffer_accessor = world.par_resource_accessor::<EventBuffer<T>>();
        Some(ParallelEventReader {
            event_buffer_accessor,
        })
    }
}

unsafe impl<T: Event> Send for ParallelEventReader<T> {}
unsafe impl<T: Event> Sync for ParallelEventReader<T> {}
unsafe impl<T: Event> ParallelSystemParam for ParallelEventReader<T> {}
