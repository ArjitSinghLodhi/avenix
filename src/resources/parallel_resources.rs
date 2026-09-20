use crate::{
    extensions::World,
    resources::{ConcurrentResourceRegistry, Res, ResMut, Resource},
    system::{SystemMeta, SystemParam},
};
use std::{marker::PhantomData, sync::Arc};

/// A thread-safe, thread-clonable handle that acts as a detached remote to access the engine's resources registry.
///
/// `ParallelResourceAccessor` can be passed to external or background worker threads, allowing them to
/// concurrently modify or read resources outside the main execution path. It can be obtained directly from
/// the world layer via [`.get_par_resource_accessor()`].
///
/// # Rules
///
/// Because the underlying storage uses synchronous locks, invoking another scope with a mutable scope open
/// on the same resource within the *same thread* will trigger a deadlock.
/// The engine does not protect against or check for this condition at runtime to maintain maximum throughput.
///
/// Also usable as a system param.
///
/// [`.get_par_resource_accessor()`]: crate::world::storage::World::get_par_resource_accessor
pub struct ParallelResourceAccessor<T: Resource + Send + Sync> {
    pub(crate) resources: Arc<ConcurrentResourceRegistry>,
    pub(crate) _marker: PhantomData<T>,
}

unsafe impl<T: Resource + Send + Sync> Send for ParallelResourceAccessor<T> {}
unsafe impl<T: Resource + Send + Sync> Sync for ParallelResourceAccessor<T> {}

impl<T: Resource + Send + Sync> Clone for ParallelResourceAccessor<T> {
    fn clone(&self) -> Self {
        Self {
            resources: self.resources.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: Resource + Send + Sync> ParallelResourceAccessor<T> {
    pub fn scope<F, R>(&self, f: F) -> R
    where
        F: for<'b> FnOnce(Res<'b, T>) -> R,
    {
        let resource = self.resources.get_resource::<T>();
        f(resource)
    }

    pub fn scope_mut<F, R>(&self, f: F) -> R
    where
        F: for<'b> FnOnce(ResMut<'b, T>) -> R,
    {
        let resource = self.resources.get_resource_mut::<T>();
        f(resource)
    }

    pub fn scope_opt<F, R>(&self, f: F) -> R
    where
        F: for<'b> FnOnce(Option<Res<'b, T>>) -> R,
    {
        let resource = self.resources.get_resource_opt::<T>();
        f(resource)
    }

    pub fn scope_mut_opt<F, R>(&self, f: F) -> R
    where
        F: for<'b> FnOnce(Option<ResMut<'b, T>>) -> R,
    {
        let resource = self.resources.get_resource_mut_opt::<T>();
        f(resource)
    }

    pub fn is_present(&self) -> bool {
        self.resources.has_resource::<T>()
    }

    /// Unlike [`Commands::insert_resource`], which queues the operation until the next apply phase.
    ///
    /// This method acquires a write lock and instantly registers the resource, returning the previous
    /// instance if it was already present in the resource registry.
    ///
    /// [`Commands::insert_resource`]: crate::commands::Commands::insert_resource
    pub fn insert_resource(&self, resource: T) -> Option<T> {
        self.resources.insert_resource(resource)
    }

    /// Unlike [`Commands::remove_resource`], which queues the operation until the next apply phase.
    ///
    /// This method acquires a write lock and instantly removes the resource, returning the
    /// instance if it was present in the resource registry.
    ///
    /// [`Commands::remove_resource`]: crate::commands::Commands::remove_resource
    pub fn remove_resource(&self) -> Option<T> {
        self.resources.remove_resource::<T>()
    }
}

impl<T: Resource + Send + Sync> SystemParam for ParallelResourceAccessor<T> {
    fn init_access(_system_meta: &mut SystemMeta) {}

    fn get_param(world: &mut World) -> Self {
        ParallelResourceAccessor {
            resources: world.resources.clone(),
            _marker: PhantomData,
        }
    }
}
