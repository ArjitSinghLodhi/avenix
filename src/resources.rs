use crate::{extensions::SystemParam, system::SystemMeta, world::storage::World};
use std::{
    any::TypeId,
    fmt::{Debug, Display},
    marker::PhantomData,
};

#[doc(hidden)]
pub trait Resource: Send + Sync + 'static {}

pub struct Res<'w, T: Resource> {
    val_ptr: *const T,
    _marker: PhantomData<&'w T>,
}

impl<'w, T: Resource> Res<'w, T> {
    pub(crate) unsafe fn new(world: &World) -> Self {
        let res = world.get_resource::<T>();
        Self {
            val_ptr: res as *const T,
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.val_ptr }
    }
}

impl<'w, T: Resource> std::ops::Deref for Res<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

unsafe impl<'w, T: Send + Resource> Send for Res<'w, T> {}
unsafe impl<'w, T: Sync + Resource> Sync for Res<'w, T> {}

impl<'w, T: Debug + Resource> Debug for Res<'w, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { write!(f, "{:?}", (*self.val_ptr)) }
    }
}

impl<'w, T: Display + Resource> Display for Res<'w, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { write!(f, "{}", (*self.val_ptr)) }
    }
}

impl<'w, T: Resource> SystemParam for Res<'w, T> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.resource_writes.contains(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_reads.insert(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        unsafe { Self::new(world) }
    }
}

pub struct ResMut<'w, T: Resource> {
    val_ptr: *mut T,
    _marker: PhantomData<&'w mut T>,
}

impl<'w, T: Resource> ResMut<'w, T> {
    pub(crate) unsafe fn new(world: &mut World) -> Self {
        let res = world.get_resource_mut::<T>();
        Self {
            val_ptr: res as *mut T,
            _marker: PhantomData,
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.val_ptr }
    }
}

impl<'w, T: Resource> std::ops::Deref for ResMut<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.val_ptr }
    }
}

impl<'w, T: Resource> std::ops::DerefMut for ResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

unsafe impl<'w, T: Send + Resource> Send for ResMut<'w, T> {}
unsafe impl<'w, T: Sync + Resource> Sync for ResMut<'w, T> {}

impl<'w, T: Debug + Resource> Debug for ResMut<'w, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { write!(f, "{:?}", (*self.val_ptr)) }
    }
}

impl<'w, T: Display + Resource> Display for ResMut<'w, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { write!(f, "{}", (*self.val_ptr)) }
    }
}

impl<'w, T: Resource> SystemParam for ResMut<'w, T> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.resource_writes.contains(&res_id)
            || system_meta.resource_reads.contains(&res_id)
        {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_writes.insert(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        unsafe { Self::new(world) }
    }
}

impl<'w, T: Resource> SystemParam for Option<Res<'w, T>> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.resource_writes.contains(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_reads.insert(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        let res_opt = world.get_resource_opt::<T>();
        if let Some(res) = res_opt {
            Some(Res {
                val_ptr: res as *const T,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}

impl<'w, T: Resource> SystemParam for Option<ResMut<'w, T>> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.resource_writes.contains(&res_id)
            || system_meta.resource_reads.contains(&res_id)
        {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters (e.g. ResMut alongside Res, or duplicate ResMut) targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_writes.insert(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        let res_opt = world.get_resource_mut_opt::<T>();
        if let Some(res) = res_opt {
            Some(ResMut {
                val_ptr: res as *mut T,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}
