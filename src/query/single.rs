use std::marker::PhantomData;

use crate::{
    query::{Query, QueryData, QueryFilter, filter::EmptyQueryFilter},
    system::{SystemMeta, SystemParam},
    world::World,
};

/// A system parameter that provides read-only access to a single, unique entity's component data.
///
/// This parameter fetches a specific entity matching the requested `QueryData` and `QueryFilter`.
/// If there is not exactly one entity matching the criteria within the current frame, the host system
/// automatically skips its execution entirely.
pub struct Single<'w, Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    item: Q::ReadOnlyItem<'w>,
    _query: Query<'w, Q, F>,
    _marker: PhantomData<F>,
}

impl<'w, Q: QueryData, F: QueryFilter> SystemParam for Single<'w, Q, F> {
    type Item<'world> = Single<'world, Q, F>;

    fn init_access(system_meta: &mut SystemMeta) {
        Query::<Q, F>::init_access(system_meta);
    }

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let query = <Query<Q, F> as SystemParam>::get_param(world)?;
        let query = unsafe { std::mem::transmute::<Query<'_, Q, F>, Query<'world, Q, F>>(query) };

        let item_ref = query.get_single().ok()?;
        let item = unsafe {
            std::mem::transmute::<Q::ReadOnlyItem<'_>, Q::ReadOnlyItem<'world>>(item_ref)
        };

        Some(Single {
            item,
            _query: query,
            _marker: PhantomData,
        })
    }
}

impl<'w, Q: QueryData, F: QueryFilter> std::ops::Deref for Single<'w, Q, F> {
    type Target = Q::ReadOnlyItem<'w>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<'w, Q: QueryData, F: QueryFilter> std::ops::DerefMut for Single<'w, Q, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

/// A system parameter that provides mutable write access to a single, unique entity's component data.
///
/// This parameter fetches a specific entity matching the requested `QueryData` and `QueryFilter`.
/// If there is not exactly one entity matching the criteria within the current frame, the host system
/// automatically skips its execution entirely.
pub struct SingleMut<'w, Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    _query: Query<'w, Q, F>,
    item: Q::Item<'w>,
    _marker: PhantomData<F>,
}

impl<'w, Q: QueryData, F: QueryFilter> SystemParam for SingleMut<'w, Q, F> {
    type Item<'world> = SingleMut<'world, Q, F>;

    fn init_access(system_meta: &mut SystemMeta) {
        Query::<Q, F>::init_access(system_meta);
    }

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let query = <Query<Q, F> as SystemParam>::get_param(world)?;
        let mut query =
            unsafe { std::mem::transmute::<Query<'_, Q, F>, Query<'world, Q, F>>(query) };

        let item_ref = query.get_single_mut().ok()?;
        let item = unsafe { std::mem::transmute::<Q::Item<'_>, Q::Item<'world>>(item_ref) };

        Some(SingleMut {
            item,
            _query: query,
            _marker: PhantomData,
        })
    }
}

impl<'w, Q: QueryData, F: QueryFilter> std::ops::Deref for SingleMut<'w, Q, F> {
    type Target = Q::Item<'w>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<'w, Q: QueryData, F: QueryFilter> std::ops::DerefMut for SingleMut<'w, Q, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

impl<'w, Q: QueryData, F: QueryFilter> SystemParam for Option<Single<'w, Q, F>> {
    type Item<'world> = Option<Single<'world, Q, F>>;

    fn init_access(system_meta: &mut SystemMeta) {
        Query::<Q, F>::init_access(system_meta);
    }

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let query = <Query<Q, F> as SystemParam>::get_param(world)?;
        let query = unsafe { std::mem::transmute::<Query<'_, Q, F>, Query<'world, Q, F>>(query) };

        let Ok(item_ref) = query.get_single() else {
            return Some(None);
        };

        let item = unsafe {
            std::mem::transmute::<Q::ReadOnlyItem<'_>, Q::ReadOnlyItem<'world>>(item_ref)
        };

        Some(Some(Single {
            item,
            _query: query,
            _marker: PhantomData,
        }))
    }
}

impl<'w, Q: QueryData, F: QueryFilter> SystemParam for Option<SingleMut<'w, Q, F>> {
    type Item<'world> = Option<SingleMut<'world, Q, F>>;

    fn init_access(system_meta: &mut SystemMeta) {
        Query::<Q, F>::init_access(system_meta);
    }

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let query = <Query<Q, F> as SystemParam>::get_param(world)?;
        let mut query =
            unsafe { std::mem::transmute::<Query<'_, Q, F>, Query<'world, Q, F>>(query) };

        let Ok(item_ref) = query.get_single_mut() else {
            return Some(None);
        };

        let item = unsafe { std::mem::transmute::<Q::Item<'_>, Q::Item<'world>>(item_ref) };

        Some(Some(SingleMut {
            item,
            _query: query,
            _marker: PhantomData,
        }))
    }
}
