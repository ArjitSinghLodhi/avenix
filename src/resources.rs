use crate::{extensions::SystemParam, system::SystemMeta, world::storage::World};
use std::{any::TypeId, marker::PhantomData};

#[doc(hidden)]
pub trait Resource: 'static {}

pub struct Res<'w, T: Resource + Send + Sync> {
    val_ptr: *const T,
    _marker: PhantomData<&'w T>,
}

impl<'w, T: Resource + Send + Sync> Res<'w, T> {
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

impl<'w, T: Resource + Send + Sync> std::ops::Deref for Res<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

unsafe impl<'w, T: Resource + Send + Sync> Send for Res<'w, T> {}
unsafe impl<'w, T: Resource + Send + Sync> Sync for Res<'w, T> {}

impl<'w, T: Resource + Send + Sync> SystemParam for Res<'w, T> {
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

pub struct ResMut<'w, T: Resource + Send + Sync> {
    val_ptr: *mut T,
    _marker: PhantomData<&'w mut T>,
}

impl<'w, T: Resource + Send + Sync> ResMut<'w, T> {
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

impl<'w, T: Resource + Send + Sync> std::ops::Deref for ResMut<'w, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.val_ptr }
    }
}

impl<'w, T: Resource + Send + Sync> std::ops::DerefMut for ResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

unsafe impl<'w, T: Resource + Send + Sync> Send for ResMut<'w, T> {}
unsafe impl<'w, T: Resource + Send + Sync> Sync for ResMut<'w, T> {}

impl<'w, T: Resource + Send + Sync> SystemParam for ResMut<'w, T> {
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

impl<'w, T: Resource + Send + Sync> SystemParam for Option<Res<'w, T>> {
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

impl<'w, T: Resource + Send + Sync> SystemParam for Option<ResMut<'w, T>> {
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
        if system_meta.resource_writes.contains(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_reads.insert(res_id);
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
        if system_meta.resource_writes.contains(&res_id)
            || system_meta.resource_reads.contains(&res_id)
        {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_writes.insert(res_id);
    }

    fn get_param(world: &mut World) -> Self {
        unsafe { Self::new(world) }
    }
}

impl<'w, T: Resource> SystemParam for Option<NonSend<'w, T>> {
    fn init_access(system_meta: &mut SystemMeta) {
        let res_id = TypeId::of::<T>();
        if system_meta.resource_writes.contains(&res_id) {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_reads.insert(res_id);
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
        if system_meta.resource_writes.contains(&res_id)
            || system_meta.resource_reads.contains(&res_id)
        {
            panic!(
                "❌ ECS RESOURCE BORROW CONFLICT: Function '{}' contains conflicting parameters targeting the same global resource singleton!",
                system_meta.name
            );
        }
        system_meta.resource_writes.insert(res_id);
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
