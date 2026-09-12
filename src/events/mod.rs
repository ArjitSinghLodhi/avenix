#![cfg(feature = "events")]

mod parallel_events;

pub use parallel_events::{ParallelEventReader, ParallelEventWriter};

use std::{
    any::{Any, TypeId},
    cell::UnsafeCell,
    sync::Arc,
};

use orx_concurrent_bag::ConcurrentBag;
use parking_lot::{RwLock, RwLockReadGuard};

use crate::extensions::{SystemMeta, SystemParam, World};

#[doc(hidden)]
pub trait Event: Send + Sync + 'static {}

pub(crate) struct TrackedEventsMeta {
    pub(crate) comp_id: TypeId,
    pub(crate) event_id: TypeId,
    pub(crate) clear_events: fn(&mut UnsafeCell<Box<dyn Any>>),
}

pub(crate) static TRACKED_EVENTS: RwLock<Vec<TrackedEventsMeta>> = RwLock::new(Vec::new());

pub(crate) fn register_event<T: Event>() {
    let mut tracked = TRACKED_EVENTS.write();
    if tracked
        .iter()
        .find(|meta| meta.comp_id == TypeId::of::<T>())
        .is_none()
    {
        tracked.push(TrackedEventsMeta {
            comp_id: TypeId::of::<T>(),
            event_id: TypeId::of::<EventBuffer<T>>(),
            clear_events: |raw_unsafecell| {
                let cell = raw_unsafecell.get_mut();
                let event_queue = cell
                    .downcast_mut::<EventBuffer<T>>()
                    .expect("Registered event queue was not found when clearing data");
                let read_queue_gaurd = &mut *event_queue.read_queue.write();
                let write_queue_gaurd = &mut *event_queue.write_queue.write();
                read_queue_gaurd.queue.clear();
                std::mem::swap(read_queue_gaurd, write_queue_gaurd);
            },
        });
    }
}

pub(crate) struct EventQueue<T: Event> {
    pub(crate) queue: ConcurrentBag<T>,
}

pub struct EventBuffer<T: Event> {
    pub(crate) read_queue: Arc<RwLock<EventQueue<T>>>,
    pub(crate) write_queue: Arc<RwLock<EventQueue<T>>>,
}

impl<T: Event> EventBuffer<T> {
    pub(crate) fn new() -> Self {
        EventBuffer {
            read_queue: Arc::new(RwLock::new(EventQueue::new())),
            write_queue: Arc::new(RwLock::new(EventQueue::new())),
        }
    }
}

impl<T: Event> EventQueue<T> {
    pub(crate) fn new() -> Self {
        Self {
            queue: ConcurrentBag::new(),
        }
    }
}

/// A system parameter used to dispatch events of type `T` to the engine's event queue.
///
/// # Architecture & Timing
///
/// Event handling in Avenix is globally double-buffered and strictly time-bound. It operates
/// on the exact same 3-frame lifecycle used by component tracking (`Added` and `Changed` flags):
///
/// * **Frame 1 (Sending):** Events sent via [`.send`] or [`.send_batch`] are safely queued in the active write buffer but remain hidden from readers.
/// * **Frame 2 (Reading Window):** The internal buffers swap globally, making these events visible to event readers.
/// * **Frame 3 (Purge):** The buffer is cleared. Events are unconditionally dropped, regardless of whether any systems read them.
///
/// Because event visibility lasts for exactly one frame window, any system designed to process these events
/// **must run every single frame** to avoid missing data.
///
/// [`.send`]: EventWriter::send
/// [`.send_batch`]: EventWriter::send_batch
pub struct EventWriter<'a, T: Event> {
    pub(crate) write_buffer: RwLockReadGuard<'a, EventQueue<T>>,
}

impl<'a, T: Event> EventWriter<'a, T> {
    /// Queues a single event to be dispatched.
    ///
    /// The event is stored in the write buffer and will become visible to readers in the next frame.
    #[inline]
    pub fn send(&mut self, event: T) {
        self.write_buffer.queue.push(event);
    }

    /// Queues an iterator of multiple events to be dispatched efficiently in a single operation.
    ///
    /// This is significantly more efficient than calling `.send()` multiple times in a loop.
    #[inline]
    pub fn send_batch<I>(&mut self, event_iter: I)
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: Send + 'static + ExactSizeIterator,
    {
        self.write_buffer.queue.extend(event_iter);
    }
}

impl<'a, T: Event> SystemParam for EventWriter<'a, T> {
    fn init_access(_system_meta: &mut SystemMeta) {}

    fn get_param(world: &mut World) -> Self {
        unsafe {
            let buffer_ptr = world.get_resource_mut::<EventBuffer<T>>() as *mut EventBuffer<T>;
            let buffer_ref: &'a EventBuffer<T> = &*buffer_ptr;

            let queue = buffer_ref.write_queue.read();

            Self {
                write_buffer: queue,
            }
        }
    }
}

/// A system parameter used to read dispatched events of type `T` from the engine's event queue.
///
/// # Architecture & Timing
///
/// Event handling in Avenix is globally double-buffered and strictly time-bound. It operates
/// on the exact same 3-frame lifecycle used by component tracking (`Added` and `Changed` flags):
///
/// * **Frame 1 (Sending):** Events are written via an `EventWriter` but remain hidden in the active write buffer.
/// * **Frame 2 (Reading Window):** The internal buffers swap globally, making these events visible to this reader via [`.iter()`].
/// * **Frame 3 (Purge):** The buffer is cleared. Events are unconditionally dropped, regardless of whether any systems read them.
///
/// Because event visibility lasts for exactly one frame window, any system designed to process these events
/// **must run every single frame** to avoid missing data.
///
/// [`.iter()`]: EventReader::iter
pub struct EventReader<'w, T: Event> {
    pub(crate) read_buffer: RwLockReadGuard<'w, EventQueue<T>>,
}

impl<'w, T: Event> EventReader<'w, T> {
    /// Returns an iterator over all events dispatched during the previous frame.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        unsafe { self.read_buffer.queue.iter() }
    }
}

impl<'w, T: Event> SystemParam for EventReader<'w, T> {
    fn init_access(_system_meta: &mut SystemMeta) {}

    fn get_param(world: &mut World) -> Self {
        let event_buffer_ref = world.get_resource::<EventBuffer<T>>() as *const EventBuffer<T>;
        let queue_ref = unsafe { &*event_buffer_ref };
        let queue = queue_ref.read_queue.read();

        Self { read_buffer: queue }
    }
}
