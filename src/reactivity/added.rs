use parking_lot::RwLock;
use std::{
    any::{TypeId, type_name},
    marker::PhantomData,
};

use crate::{
    ecs::component::Component,
    query::{QueryData, QueryFilter},
    reactivity::{TRACKED_COMPONENTS, TrackedComponentMeta, verify_app_not_built_yet},
    system::AccessVec,
    world::archetypes::{Archetype, ComponentColumn, ComponentColumnRead},
};

pub(crate) fn register_added_tracked_component<T: Component>() {
    let mut tracked = TRACKED_COMPONENTS.lock();
    let component_id = TypeId::of::<T>();
    let marker_id = TypeId::of::<AddedState<T>>();

    let meta_list = tracked.entry(component_id).or_default();

    if meta_list.iter().any(|m| m.marker_id == marker_id) {
        return;
    }

    let _lock = verify_app_not_built_yet(type_name::<AddedState<T>>());
    meta_list.push(TrackedComponentMeta {
        marker_id,
        create_marker_column: || ComponentColumn {
            data: RwLock::new(Box::new(Vec::<AddedState<T>>::new())),
        },
        push_default_marker: |column| {
            let mut guard = column.data.write();
            let raw_any = guard.as_any_mut();
            let vec = raw_any.downcast_mut::<Vec<AddedState<T>>>().unwrap();
            vec.push(AddedState {
                read: false,
                write: true,
                phantom: PhantomData,
            });
        },
        clear_column_markers: |raw_any| {
            let vec = raw_any.downcast_mut::<Vec<AddedState<T>>>().unwrap();
            vec.iter_mut().for_each(|state| {
                state.read = state.write;
                state.write = false;
            });
        },
    });
}

#[derive(Clone, Copy, Debug)]
pub struct AddedState<T: Component> {
    read: bool,
    write: bool,
    phantom: PhantomData<T>,
}

impl<T: Component> AddedState<T> {
    pub const fn new(read: bool, write: bool) -> Self {
        Self {
            read,
            write,
            phantom: PhantomData,
        }
    }

    pub const fn read(&self) -> bool {
        self.read
    }

    pub fn set_read(&mut self, read: bool) {
        self.read = read;
    }

    pub const fn write(&self) -> bool {
        self.write
    }

    pub fn set_write(&mut self, write: bool) {
        self.write = write;
    }
}

impl<T: Component> Component for AddedState<T> {}

/// A query filter that matches components of type `T` that were newly added during the previous frame.
///
/// # Architecture & Timing
///
/// Component addition tracking in Avenix is globally double-buffered and strictly time-bound.
/// It operates on a strict 3-frame lifecycle:
///
/// * **Frame 1 (Addition):** The component is structurally added or spawned. The tracking bool is set internally but is **not** yet visible to queries.
/// * **Frame 2 (Detection Window):** The addition becomes globally visible. Any system running in this frame filtering for `Added<T>` will detect it.
/// * **Frame 3 (Purge):** The addition state is unconditionally cleared and resets to `false`.
///
/// Regardless of whether a system ran or read the data, the detection flag will never last for more than exactly one frame.
///
/// # Structural Mutations & Spawning
///
/// Spawning a new entity with a component or inserting a component via commands acts as the initial addition event.
/// This structural modification triggers addition tracking cleanly through the standard timeline:
///
/// 1. **Command Queuing:** You call `.spawn()` or `.insert()` to queue a component modification command.
/// 2. **Immediate Application:** The command is flushed and applied to the world during that same frame's execution phase.
/// 3. **Next-Frame Visibility:** The tracking markers are flagged immediately following application, making `Added<T>` evaluate to `true` across the entirety of the next frame.
pub struct Added<T: Component>(std::marker::PhantomData<T>);

impl<T: Component> QueryFilter for Added<T> {
    type FilterData<'w> = ComponentColumnRead<'w, AddedState<T>>;
    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
        archetype.column::<AddedState<T>>()
    }
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }
    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool {
        unsafe { (*filter_data.as_ptr().add(row_idx)).read }
    }
    fn collect_filter(
        withs: &mut AccessVec<std::any::TypeId>,
        _withouts: &mut AccessVec<std::any::TypeId>,
    ) {
        withs.push(TypeId::of::<AddedState<T>>());
        register_added_tracked_component::<T>();
    }

    fn collect_filter_access(reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {
        reads.push(TypeId::of::<AddedState<T>>());
    }
}

/// A query data wrapper that allows systems to inspect the addition state of an individual component instance.
///
/// Unlike the [`Added<T>`] filter, which excludes non-added entities from the query entirely,
/// `AddedTracker<T>` allows the entity to match the query while exposing the [`.is_added()`] method
/// to check its status conditionally.
///
/// # Architecture & Timing
///
/// This tracking relies on the exact same double-buffered, frame-locked system as the filter.
/// For a detailed breakdown of the 3-frame lifecycle timing, command application phases, and
/// initial spawn behavior, see the documentation for [`Added<T>`].
///
/// [`.is_added()`]: AddedTracker::is_added
pub struct AddedTracker<T: Component> {
    added: bool,
    _phantom: PhantomData<T>,
}

impl<T: Component> AddedTracker<T> {
    #[inline(always)]
    pub fn is_added(&self) -> bool {
        self.added
    }
}

impl<T: Component> QueryData for AddedTracker<T> {
    type Item<'w> = AddedTracker<T>;
    type ReadOnlyItem<'w> = AddedTracker<T>;
    type Fetch<'w> = ComponentColumnRead<'w, AddedState<T>>;
    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        _writes: &mut AccessVec<std::any::TypeId>,
    ) {
        reads.push(TypeId::of::<AddedState<T>>());
        register_added_tracked_component::<T>();
    }
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }
    unsafe fn init_fetch<'w>(archetype: &'w Archetype) -> Self::Fetch<'w> {
        archetype.column::<AddedState<T>>()
    }

    unsafe fn fetch_mut<'w, 'b>(fetch: &Self::Fetch<'b>, index: usize) -> Self::Item<'w> {
        let marker_ref = unsafe { &*fetch.as_ptr().add(index) };
        let added = marker_ref.read;
        AddedTracker {
            added,
            _phantom: PhantomData,
        }
    }

    unsafe fn fetch_read_only<'w, 'b>(
        fetch: &Self::Fetch<'b>,
        index: usize,
    ) -> Self::ReadOnlyItem<'w> {
        let marker_ref = unsafe { &(*fetch.as_ptr().add(index)) };
        let added = marker_ref.read;
        AddedTracker {
            added,
            _phantom: PhantomData,
        }
    }
}
