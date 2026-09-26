use crate::ecs::Component;
use crate::query::ThreadSafe;
use crate::world::archetypes::ComponentColumnRead;
#[cfg(not(feature = "reactivity"))]
use crate::world::archetypes::ComponentColumnWrite;
#[cfg(feature = "reactivity")]
use crate::world::archetypes::ComponentColumnWrite;
use crate::{entity::Entity, query::QueryData, system::AccessVec, world::archetypes::Archetype};
#[cfg(feature = "reactivity")]
use crate::{
    reactivity::{ChangedMarker, Mut},
    world::storage::CurrentBufferIdx,
};

use std::{any::TypeId, marker::PhantomData};

impl<'a, T: Component> QueryData for &'a T {
    type Item<'w> = &'w T;
    type ReadOnlyItem<'w> = &'w T;
    type Fetch = ComponentColumnRead<'a, T>;

    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }
    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        _writes: &mut AccessVec<std::any::TypeId>,
    ) {
        reads.push(TypeId::of::<T>());
    }
    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        archetype.get_column::<T>()
    }
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe { &*fetch.as_ptr().add(index) }
    }
    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.as_ptr().add(index) }
    }
}

#[cfg(not(feature = "reactivity"))]
impl<'a, T: Component> QueryData for &'a mut T {
    type Item<'w> = &'w mut T;
    type ReadOnlyItem<'w> = &'w T;
    type Fetch = ComponentColumnWrite<'a, T>;

    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        archetype.get_column_mut::<T>()
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
        let data_ptr = fetch.as_ptr() as *mut T;
        unsafe { &mut *data_ptr.add(index) }
    }

    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.as_ptr().add(index) }
    }

    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
    }
}

#[cfg(feature = "reactivity")]
impl<'a, T: Component> QueryData for &'a mut T {
    type Item<'w> = Mut<'w, T>;
    type ReadOnlyItem<'w> = &'w T;
    type Fetch = ThreadSafe<(
        ComponentColumnWrite<'a, T>,
        Option<ComponentColumnWrite<'a, ChangedMarker<T>>>,
        *mut ChangedMarker<T>,
        u8,
        bool,
    )>;

    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        let data_writer = archetype.get_column_mut::<T>();
        let marker_writer_opt = archetype.get_column_mut_opt::<ChangedMarker<T>>();
        let current_write_idx = CurrentBufferIdx::current_write_idx();

        if let Some(mut marker_writer) = marker_writer_opt {
            let marker_ptr = marker_writer.as_mut_ptr();
            ThreadSafe {
                value: (
                    data_writer,
                    Some(marker_writer),
                    marker_ptr,
                    current_write_idx,
                    true,
                ),
            }
        } else {
            ThreadSafe {
                value: (
                    data_writer,
                    None,
                    std::ptr::null_mut(),
                    current_write_idx,
                    false,
                ),
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
        let fetch = &fetch.value;
        let val_ptr = fetch.0.as_ptr() as *mut T;
        unsafe {
            Mut {
                value: val_ptr.add(index),
                marker: fetch.2.wrapping_add(index),
                current_write_idx: fetch.3,
                should_modify: fetch.4,
                _marker: std::marker::PhantomData,
            }
        }
    }

    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.value.0.as_ptr().add(index) }
    }

    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
        writes.push(TypeId::of::<ChangedMarker<T>>());
    }
}

impl QueryData for Entity {
    type Item<'w> = &'w Entity;
    type ReadOnlyItem<'w> = &'w Entity;
    type Fetch = ThreadSafe<*const Entity>;

    fn matches(_archetype: &Archetype) -> bool {
        true
    }
    fn collect_access(_reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {}
    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        ThreadSafe {
            value: archetype.entities.as_ptr(),
        }
    }
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe { &*fetch.value.add(index) }
    }
    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.value.add(index) }
    }
}

impl<'a, T: Component> QueryData for Option<&'a T> {
    type Item<'w> = Option<&'w T>;
    type ReadOnlyItem<'w> = Option<&'w T>;
    type Fetch = Option<ComponentColumnWrite<'a, T>>;

    fn matches(_archetype: &Archetype) -> bool {
        true
    }

    fn collect_access(reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {
        reads.push(TypeId::of::<T>());
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        if archetype.types.contains(&TypeId::of::<T>()) {
            Some(archetype.get_column_mut::<T>())
        } else {
            None
        }
    }
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
        if let Some(fetch) = fetch {
            let val_ptr = fetch.as_ptr() as *mut T;
            unsafe { Some(&*val_ptr.add(index)) }
        } else {
            None
        }
    }

    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        if let Some(fetch) = fetch {
            unsafe { Some(&*fetch.as_ptr().add(index)) }
        } else {
            None
        }
    }
}

#[cfg(not(feature = "reactivity"))]
impl<'a, T: Component> QueryData for Option<&'a mut T> {
    type Item<'w> = Option<&'w mut T>;
    type ReadOnlyItem<'w> = Option<&'w T>;
    type Fetch = Option<ComponentColumnWrite<'a, T>>;

    fn matches(_archetype: &Archetype) -> bool {
        true
    }
    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        archetype.get_column_mut_opt::<T>()
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe {
            if let Some(data_head) = fetch {
                let data_ptr = data_head.as_ptr() as *mut T;
                Some(&mut *data_ptr.add(index))
            } else {
                None
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe {
            if let Some(data_head) = fetch {
                Some(&*data_head.as_ptr().add(index))
            } else {
                None
            }
        }
    }
}

#[cfg(feature = "reactivity")]
impl<'a, T: Component> QueryData for Option<&'a mut T> {
    type Item<'w> = Option<Mut<'w, T>>;
    type ReadOnlyItem<'w> = Option<&'w T>;
    type Fetch = Option<
        ThreadSafe<(
            ComponentColumnWrite<'a, T>,
            Option<ComponentColumnWrite<'a, ChangedMarker<T>>>,
            *mut ChangedMarker<T>,
            u8,
            bool,
        )>,
    >;

    fn matches(_archetype: &Archetype) -> bool {
        true
    }

    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
        writes.push(TypeId::of::<ChangedMarker<T>>());
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        let marker_writer_opt = archetype.get_column_mut_opt::<ChangedMarker<T>>();
        let current_write_idx = CurrentBufferIdx::current_write_idx();
        let data_writer_opt = archetype.get_column_mut_opt::<T>();
        if let Some(data_writer) = data_writer_opt {
            if let Some(mut marker_writer) = marker_writer_opt {
                let marker_ptr = marker_writer.as_mut_ptr();
                Some(ThreadSafe {
                    value: (
                        data_writer,
                        Some(marker_writer),
                        marker_ptr,
                        current_write_idx,
                        true,
                    ),
                })
            } else {
                Some(ThreadSafe {
                    value: (
                        data_writer,
                        None,
                        std::ptr::null_mut(),
                        current_write_idx,
                        false,
                    ),
                })
            }
        } else {
            None
        }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe {
            if let Some(fetch) = fetch {
                let fetch = &fetch.value;
                let data_ptr = fetch.0.as_ptr() as *mut T;
                Some(Mut {
                    value: data_ptr.add(index),
                    marker: fetch.2.wrapping_add(index),
                    current_write_idx: fetch.3,
                    should_modify: fetch.4,
                    _marker: std::marker::PhantomData,
                })
            } else {
                None
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe {
            if let Some(data_head) = fetch {
                Some(&*data_head.value.0.as_ptr().add(index))
            } else {
                None
            }
        }
    }
}

pub struct Has<T: Component>(PhantomData<T>);

impl<T: Component> QueryData for Has<T> {
    type Item<'w> = bool;
    type ReadOnlyItem<'w> = bool;
    type Fetch = bool;
    fn collect_access(
        _reads: &mut AccessVec<std::any::TypeId>,
        _writes: &mut AccessVec<std::any::TypeId>,
    ) {
    }

    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        archetype.get_column_opt::<T>().is_some()
    }

    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, _index: usize) -> Self::Item<'w> {
        *fetch
    }

    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, _index: usize) -> Self::ReadOnlyItem<'w> {
        *fetch
    }
}

macro_rules! impl_world_query_tuple {
    ($($name:ident -> $idx:tt),*) => {
        impl<$($name: QueryData),*> QueryData for ($($name,)*) {
            type Item<'w> = ($($name::Item<'w>,)*);
            type ReadOnlyItem<'w> = ($($name::ReadOnlyItem<'w>,)*);
            type Fetch = ($($name::Fetch,)*);

            fn matches(archetype: &Archetype) -> bool { $($name::matches(archetype))&&* }
            unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch { unsafe { ($($name::init_fetch(archetype),)*) } }
            unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w> {
                unsafe {($($name::fetch_mut(&fetch.$idx, index),)*)}
            }

            unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
                unsafe { ($($name::fetch_read_only(&fetch.$idx, index),)*) }
            }

            fn collect_access(reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
                $( $name::collect_access(reads, writes); )*
            }
        }
    };
}

impl_world_query_tuple!(A -> 0);
impl_world_query_tuple!(A -> 0, B -> 1);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7, I -> 8);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7, I -> 8, J -> 9);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7, I -> 8, J -> 9, K -> 10);
