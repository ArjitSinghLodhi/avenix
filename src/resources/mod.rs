use crate::{extensions::SystemParam, system::SystemMeta, world::storage::World};
use dashmap::{
    DashMap,
    mapref::one::{Ref, RefMut},
};
use rustc_hash::FxBuildHasher;
use std::{
    any::{Any, TypeId, type_name},
    marker::PhantomData,
};

mod parallel_resources;

pub use parallel_resources::ParallelResourceAccessor;

pub(crate) struct ConcurrentResourceRegistry {
    pub(crate) resources: DashMap<TypeId, Box<dyn Any>, FxBuildHasher>,
}

unsafe impl Send for ConcurrentResourceRegistry {}
unsafe impl Sync for ConcurrentResourceRegistry {}

impl ConcurrentResourceRegistry {
    pub(crate) fn new() -> Self {
        Self {
            resources: DashMap::with_hasher(FxBuildHasher),
        }
    }

    pub(crate) fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }

    pub(crate) fn insert_resource<T: Resource + Send + Sync>(&self, resource: T) -> Option<T> {
        let type_id = TypeId::of::<T>();
        let boxed_res = Box::new(resource) as Box<dyn Any>;
        if let Some(res) = self.resources.insert(type_id, boxed_res) {
            let res = res.downcast::<T>().unwrap();
            Some(*res)
        } else {
            None
        }
    }

    pub(crate) fn remove_resource<T: Resource + Send + Sync>(&self) -> Option<T> {
        let type_id = TypeId::of::<T>();
        if let Some((_, res)) = self.resources.remove(&type_id) {
            let res = res.downcast::<T>().unwrap();
            Some(*res)
        } else {
            None
        }
    }

    pub(crate) fn get_resource<'w, T: Resource + Send + Sync>(&self) -> Res<'w, T> {
        let type_id = TypeId::of::<T>();
        let gaurd = self.resources.get(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested resource: '{}' was never registered!",
                type_name::<T>()
            );
        });

        let gaurd = unsafe {
            std::mem::transmute::<Ref<'_, TypeId, Box<dyn Any>>, Ref<'_, TypeId, Box<dyn Any>>>(
                gaurd,
            )
        };

        let res_ptr = gaurd.downcast_ref::<T>().expect("Resource type mismatch!") as *const T;
        Res {
            _gaurd: gaurd,
            res_ptr,
        }
    }

    pub(crate) fn get_resource_mut<'w, T: Resource + Send + Sync>(&self) -> ResMut<'w, T> {
        let type_id = TypeId::of::<T>();
        let gaurd = self.resources.get_mut(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested resource: '{}' was never registered!",
                type_name::<T>()
            );
        });

        let mut gaurd = unsafe {
            std::mem::transmute::<RefMut<'_, TypeId, Box<dyn Any>>, RefMut<'_, TypeId, Box<dyn Any>>>(
                gaurd,
            )
        };

        let res_ptr = gaurd.downcast_mut::<T>().expect("Resource type mismatch!") as *mut T;
        ResMut {
            _gaurd: gaurd,
            res_ptr,
        }
    }

    pub(crate) fn get_resource_opt<'w, T: Resource + Send + Sync>(&self) -> Option<Res<'w, T>> {
        let type_id = TypeId::of::<T>();
        unsafe {
            let gaurd = self.resources.get(&type_id)?;
            let gaurd = std::mem::transmute::<
                Ref<'_, TypeId, Box<dyn Any>>,
                Ref<'_, TypeId, Box<dyn Any>>,
            >(gaurd);
            let res_ptr = gaurd.downcast_ref::<T>()? as *const T;
            Some(Res {
                _gaurd: gaurd,
                res_ptr,
            })
        }
    }

    pub(crate) fn get_resource_mut_opt<'w, T: Resource + Send + Sync>(
        &self,
    ) -> Option<ResMut<'w, T>> {
        let type_id = TypeId::of::<T>();
        unsafe {
            let gaurd = self.resources.get_mut(&type_id)?;
            let mut gaurd = std::mem::transmute::<
                RefMut<'_, TypeId, Box<dyn Any>>,
                RefMut<'_, TypeId, Box<dyn Any>>,
            >(gaurd);
            let res_ptr = gaurd.downcast_mut::<T>()? as *mut T;
            Some(ResMut {
                _gaurd: gaurd,
                res_ptr,
            })
        }
    }
}

#[doc(hidden)]
pub trait Resource: 'static {}

pub struct Res<'w, T: Resource + Send + Sync> {
    pub(crate) _gaurd: Ref<'w, TypeId, Box<dyn Any>>,
    pub(crate) res_ptr: *const T,
}

impl<'w, T: Resource + Send + Sync> Res<'w, T> {
    pub(crate) unsafe fn new(world: &World) -> Self {
        world.get_resource::<T>()
    }
}

impl<'w, T: Resource + Send + Sync> std::ops::Deref for Res<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.res_ptr) }
    }
}

unsafe impl<'w, T: Resource + Send + Sync> Send for Res<'w, T> {}
unsafe impl<'w, T: Resource + Send + Sync> Sync for Res<'w, T> {}

impl<'w, T: Resource + Send + Sync> SystemParam for Res<'w, T> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_read(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        unsafe { Self::new(world) }
    }
}

pub struct ResMut<'w, T: Resource + Send + Sync> {
    pub(crate) _gaurd: RefMut<'w, TypeId, Box<dyn Any>>,
    pub(crate) res_ptr: *mut T,
}

impl<'w, T: Resource + Send + Sync> ResMut<'w, T> {
    pub(crate) unsafe fn new(world: &mut World) -> Self {
        world.get_resource_mut::<T>()
    }
}

impl<'w, T: Resource + Send + Sync> std::ops::Deref for ResMut<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.res_ptr) }
    }
}

impl<'w, T: Resource + Send + Sync> std::ops::DerefMut for ResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.res_ptr) }
    }
}

unsafe impl<'w, T: Resource + Send + Sync> Send for ResMut<'w, T> {}
unsafe impl<'w, T: Resource + Send + Sync> Sync for ResMut<'w, T> {}

impl<'w, T: Resource + Send + Sync> SystemParam for ResMut<'w, T> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) || system_meta.has_resource_read(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_write(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        unsafe { Self::new(world) }
    }
}

impl<'w, T: Resource + Send + Sync> SystemParam for Option<Res<'w, T>> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_read(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        world.get_resource_opt::<T>()
    }
}

impl<'w, T: Resource + Send + Sync> SystemParam for Option<ResMut<'w, T>> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) || system_meta.has_resource_read(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_write(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        world.get_resource_mut_opt::<T>()
    }
}

pub struct NonSend<'w, T: Resource> {
    val_ptr: *const T,
    _marker: PhantomData<&'w T>,
}

impl<'w, T: Resource> NonSend<'w, T> {
    pub(crate) unsafe fn new(world: &World) -> Self {
        let res = world.get_non_send_resource::<T>();
        Self {
            val_ptr: res as *const T,
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.val_ptr }
    }
}

impl<'w, T: Resource> std::ops::Deref for NonSend<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<'w, T: Resource> SystemParam for NonSend<'w, T> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_read(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        unsafe { Self::new(world) }
    }
}

pub struct NonSendMut<'w, T: Resource> {
    val_ptr: *mut T,
    _marker: PhantomData<&'w mut T>,
}

impl<'w, T: Resource> NonSendMut<'w, T> {
    pub(crate) unsafe fn new(world: &mut World) -> Self {
        let res = world.get_non_send_resource_mut::<T>();
        Self {
            val_ptr: res as *mut T,
            _marker: PhantomData,
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.val_ptr }
    }
}

impl<'w, T: Resource> std::ops::Deref for NonSendMut<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.val_ptr }
    }
}

impl<'w, T: Resource> std::ops::DerefMut for NonSendMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<'w, T: Resource> SystemParam for NonSendMut<'w, T> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) || system_meta.has_resource_read(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_write(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        unsafe { Self::new(world) }
    }
}

impl<'w, T: Resource> SystemParam for Option<NonSend<'w, T>> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_read(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        let res_opt = world.get_non_send_resource_opt::<T>();
        if let Some(res) = res_opt {
            Some(NonSend {
                val_ptr: res as *const T,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}

impl<'w, T: Resource> SystemParam for Option<NonSendMut<'w, T>> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.has_resource_write(&res_id) || system_meta.has_resource_read(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.get_func_name()
            );
        }
        system_meta.add_resource_write(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        let res_opt = world.get_non_send_resource_mut_opt::<T>();
        if let Some(res) = res_opt {
            Some(NonSendMut {
                val_ptr: res as *mut T,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}
