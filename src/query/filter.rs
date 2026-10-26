use std::{any::TypeId, marker::PhantomData};

use crate::{ecs::component::Component, system::AccessVec, world::archetypes::Archetype};

pub trait QueryFilter {
    type FilterData<'w>: Sync;
    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w>;
    fn matches(archetype: &Archetype) -> bool;
    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool;
    fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>);
    fn collect_filter_access(reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>);
}

#[derive(Debug)]
pub struct With<T: Component>(PhantomData<T>);
impl<T: Component> QueryFilter for With<T> {
    type FilterData<'w> = bool;
    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
        archetype.has_column::<T>()
    }
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }
    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, _row_idx: usize) -> bool {
        *filter_data
    }
    fn collect_filter(withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {
        withs.push(std::any::TypeId::of::<T>());
    }
    fn collect_filter_access(_reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {}
}

#[derive(Debug)]
pub struct Without<T: Component>(PhantomData<T>);
impl<T: Component> QueryFilter for Without<T> {
    type FilterData<'w> = bool;
    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
        !archetype.has_column::<T>()
    }
    fn matches(archetype: &Archetype) -> bool {
        !archetype.has_column::<T>()
    }
    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, _row_idx: usize) -> bool {
        *filter_data
    }
    fn collect_filter(_withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
        withouts.push(std::any::TypeId::of::<T>());
    }

    fn collect_filter_access(_reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {}
}

pub struct Or<T>(PhantomData<T>);

pub struct Not<F>(PhantomData<F>);

impl<F: QueryFilter> QueryFilter for Not<F> {
    type FilterData<'w> = Option<F::FilterData<'w>>;

    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
        if F::matches(archetype) {
            Some(F::init_filter_data(archetype))
        } else {
            None
        }
    }

    fn matches(_archetype: &Archetype) -> bool {
        true
    }

    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool {
        if let Some(data) = filter_data {
            !F::matches_row(data, row_idx)
        } else {
            true
        }
    }

    fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
        F::collect_filter(withouts, withs);
    }

    fn collect_filter_access(reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        F::collect_filter_access(reads, writes);
    }
}

macro_rules! impl_or_tuple {
    ($($name:ident),*) => {
        impl<$($name: QueryFilter),*> QueryFilter for Or<($($name,)*)> {
            type FilterData<'w> = ($( Option<$name::FilterData<'w>>, )*);

            #[inline]
            fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
                ($(
                    if $name::matches(archetype) {
                        Some($name::init_filter_data(archetype))
                    } else {
                        None
                    },
                )*)
            }

            fn matches(_archetype: &Archetype) -> bool {
                true
            }

            fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool {
                #[allow(non_snake_case)]
                let ($($name,)*) = filter_data;
                $(
                    $name.as_ref().map_or(false, |inner_data| $name::matches_row(inner_data, row_idx))
                )||*
            }

            fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
                $(
                    $name::collect_filter(withs, withouts);
                )*
            }

            fn collect_filter_access(reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
                $(
                    $name::collect_filter_access(reads, writes);
                )*
            }
        }
    };
}

impl_or_tuple!(A, B);
impl_or_tuple!(A, B, C);
impl_or_tuple!(A, B, C, D);
impl_or_tuple!(A, B, C, D, E);
impl_or_tuple!(A, B, C, D, E, F);
impl_or_tuple!(A, B, C, D, E, F, G);
impl_or_tuple!(A, B, C, D, E, F, G, H);
impl_or_tuple!(A, B, C, D, E, F, G, H, I);
impl_or_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_or_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_or_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

#[derive(Debug)]
pub struct EmptyQueryFilter;
impl QueryFilter for EmptyQueryFilter {
    type FilterData<'w> = ();
    fn init_filter_data<'w>(_archetype: &Archetype) -> Self::FilterData<'w> {}
    fn matches(_archetype: &Archetype) -> bool {
        true
    }
    fn matches_row<'w>(_filter_data: &Self::FilterData<'w>, _row_idx: usize) -> bool {
        true
    }
    fn collect_filter(_withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {}
    fn collect_filter_access(_reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {}
}

macro_rules! impl_query_filter_tuple {
    ($($name:ident),*) => {
        impl<$($name: QueryFilter),*> QueryFilter for ($($name,)*) {
            type FilterData<'w> = ($($name::FilterData<'w>,)*);

            fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
                ($($name::init_filter_data(archetype),)*)
            }

            fn matches(archetype: &Archetype) -> bool {
                $($name::matches(archetype))&&*
            }

            fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool {
                #[allow(non_snake_case)]
                let ($($name,)*) = filter_data;
                $($name::matches_row($name, row_idx))&&*
            }

            fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
                $(
                    $name::collect_filter(withs, withouts);
                )*
            }

            fn collect_filter_access(reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
                $(
                    $name::collect_filter_access(reads, writes);
                )*
            }
        }
    };
}

impl_query_filter_tuple!(A);
impl_query_filter_tuple!(A, B);
impl_query_filter_tuple!(A, B, C);
impl_query_filter_tuple!(A, B, C, D);
impl_query_filter_tuple!(A, B, C, D, E);
impl_query_filter_tuple!(A, B, C, D, E, F);
impl_query_filter_tuple!(A, B, C, D, E, F, G);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
