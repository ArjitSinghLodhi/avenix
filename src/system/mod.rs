pub(crate) mod condition;
mod functions;
pub(crate) mod system_set;
pub(crate) mod system_set_traits;
pub(crate) mod system_storage;
pub(crate) mod system_traits;
use std::{
    any::{Any, TypeId},
    hash::Hash,
    marker::PhantomData,
};

use indexmap::IndexSet;
use rustc_hash::FxBuildHasher;

use crate::{
    extensions::{FunctionData, SystemData},
    world::storage::World,
};

/// # Safety
///
/// Anything implementing this trait must guarantee it can be used concurrently from any thread.
pub unsafe trait ParallelSystemParam: SystemParam + Send + Sync + 'static {}

pub struct AccessHashSet<T: Eq + Hash> {
    pub(crate) set: IndexSet<T, FxBuildHasher>,
}

impl<T: Eq + Hash> AccessHashSet<T> {
    pub(crate) fn new() -> Self {
        Self {
            set: IndexSet::default(),
        }
    }

    pub fn insert(&mut self, val: T) -> bool {
        self.set.insert(val)
    }

    pub fn contains(&self, val: &T) -> bool {
        self.set.contains(val)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.set.iter()
    }
}

impl<T: Eq + Hash> Default for AccessHashSet<T> {
    fn default() -> Self {
        AccessHashSet::new()
    }
}

pub struct AccessVec<T> {
    pub(crate) vec: Vec<T>,
}

impl<T> Default for AccessVec<T> {
    fn default() -> Self {
        Self { vec: Vec::new() }
    }
}

impl<T: Eq> AccessVec<T> {
    pub fn contains(&self, val: &T) -> bool {
        self.vec.contains(val)
    }
}

impl<T> AccessVec<T> {
    pub(crate) fn new() -> Self {
        Self { vec: Vec::new() }
    }

    pub fn push(&mut self, val: T) {
        self.vec.push(val);
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.vec.iter()
    }
    pub fn as_slice(&self) -> &[T] {
        self.vec.as_slice()
    }
}

pub struct SystemMeta {
    name: String,
    component_reads: AccessHashSet<TypeId>,
    component_writes: AccessHashSet<TypeId>,
    resource_reads: AccessHashSet<TypeId>,
    resource_writes: AccessHashSet<TypeId>,
    with_filters: AccessHashSet<TypeId>,
    without_filters: AccessHashSet<TypeId>,
    is_send: bool,
}

impl Default for SystemMeta {
    fn default() -> Self {
        Self::new("DefaultSystemMeta".to_string())
    }
}

impl SystemMeta {
    pub fn new(func_name: String) -> Self {
        Self {
            name: func_name,
            component_reads: AccessHashSet::new(),
            component_writes: AccessHashSet::new(),
            resource_reads: AccessHashSet::new(),
            resource_writes: AccessHashSet::new(),
            with_filters: AccessHashSet::new(),
            without_filters: AccessHashSet::new(),
            is_send: true,
        }
    }

    pub fn get_func_name(&self) -> String {
        self.name.clone()
    }

    pub fn set_non_send(&mut self) {
        self.is_send = false;
    }

    pub fn is_send(&self) -> bool {
        self.is_send
    }

    pub fn add_component_read(&mut self, type_id: TypeId) {
        self.component_reads.insert(type_id);
    }

    pub fn add_component_write(&mut self, type_id: TypeId) {
        self.component_writes.insert(type_id);
    }

    pub fn add_resource_read(&mut self, type_id: TypeId) {
        self.resource_reads.insert(type_id);
    }

    pub fn add_resource_write(&mut self, type_id: TypeId) {
        self.resource_writes.insert(type_id);
    }

    pub fn add_with_filter(&mut self, type_id: TypeId) {
        self.with_filters.insert(type_id);
    }

    pub fn add_without_filter(&mut self, type_id: TypeId) {
        self.without_filters.insert(type_id);
    }

    pub fn has_component_read(&self, type_id: &TypeId) -> bool {
        self.component_reads.contains(type_id)
    }

    pub fn has_component_write(&self, type_id: &TypeId) -> bool {
        self.component_writes.contains(type_id)
    }

    pub fn has_resource_read(&self, type_id: &TypeId) -> bool {
        self.resource_reads.contains(type_id)
    }

    pub fn has_resource_write(&self, type_id: &TypeId) -> bool {
        self.resource_writes.contains(type_id)
    }

    pub fn has_with_filter(&self, type_id: &TypeId) -> bool {
        self.with_filters.contains(type_id)
    }

    pub fn has_without_filter(&self, type_id: &TypeId) -> bool {
        self.without_filters.contains(type_id)
    }
}

impl SystemMeta {
    pub fn component_reads(&self) -> impl Iterator<Item = &TypeId> {
        self.component_reads.iter()
    }

    pub fn component_writes(&self) -> impl Iterator<Item = &TypeId> {
        self.component_writes.iter()
    }

    pub fn resource_reads(&self) -> impl Iterator<Item = &TypeId> {
        self.resource_reads.iter()
    }

    pub fn resource_writes(&self) -> impl Iterator<Item = &TypeId> {
        self.resource_writes.iter()
    }

    pub fn with_filters(&self) -> impl Iterator<Item = &TypeId> {
        self.with_filters.iter()
    }

    pub fn without_filters(&self) -> impl Iterator<Item = &TypeId> {
        self.without_filters.iter()
    }
}

impl SystemMeta {
    pub fn extend(&mut self, param_access_other: &mut SystemMeta) {
        self.component_reads
            .set
            .append(&mut param_access_other.component_reads.set);
        self.component_writes
            .set
            .append(&mut param_access_other.component_writes.set);
        self.with_filters
            .set
            .append(&mut param_access_other.with_filters.set);
        self.without_filters
            .set
            .append(&mut param_access_other.without_filters.set);
        self.resource_reads
            .set
            .append(&mut param_access_other.resource_reads.set);
        self.resource_writes
            .set
            .append(&mut param_access_other.resource_writes.set);
        if !param_access_other.is_send {
            self.is_send = false;
        }
    }
}

pub trait SystemParam {
    fn init_access(system_meta: &mut SystemMeta);
    fn get_param(world: &World) -> Self;
}

#[doc(hidden)]
pub trait System: SystemData + 'static {
    fn run(&mut self, world: &World);
    #[doc(hidden)]
    fn func_type_id(&self) -> TypeId;
    #[doc(hidden)]
    fn name(&self) -> &'static str;
}

#[doc(hidden)]
pub struct FunctionSystem<Marker, F> {
    pub(crate) func: F,
    pub(crate) data: FunctionData,
    pub(crate) _marker: std::marker::PhantomData<Marker>,
}

impl<Marker, F> FunctionSystem<Marker, F> {
    pub(crate) fn new(func: F) -> Self {
        Self {
            func,
            data: FunctionData::new(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<Marker, F> SystemData for FunctionSystem<Marker, F> {
    #[doc(hidden)]
    fn get_raw(&self, id: TypeId) -> Option<&Box<dyn Any + Send + Sync>> {
        self.data.get_raw_data(&id)
    }

    #[doc(hidden)]
    fn get_raw_mut(&mut self, id: TypeId) -> Option<&mut Box<dyn Any + Send + Sync>> {
        self.data.get_raw_data_mut(&id)
    }

    #[doc(hidden)]
    fn insert_raw(&mut self, id: TypeId, value: Box<dyn Any + Send + Sync>) {
        self.data.insert_raw_data(&id, value);
    }
}

#[doc(hidden)]
pub trait IntoSystem<Marker> {
    #[doc(hidden)]
    type SystemType: System + 'static;
    #[doc(hidden)]
    fn into_system(self) -> Self::SystemType;
}

#[doc(hidden)]
pub struct SystemConfigs<Marker> {
    pub(crate) systems: Vec<Box<dyn System>>,
    pub(crate) _marker: PhantomData<Marker>,
}

#[doc(hidden)]
pub trait IntoSystemConfigs<MarkerGroup> {
    #[doc(hidden)]
    fn into_configs(self) -> SystemConfigs<MarkerGroup>;
}

macro_rules! impl_system_configs_tuple {
    ($($sys:ident),* ; $($marker:ident),*) => {
        impl<$($sys,)* $($marker,)*> IntoSystemConfigs<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystem<$marker> + 'static ),*
        {
            fn into_configs(self) -> SystemConfigs<($($marker,)*)> {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                SystemConfigs {
                    systems: vec![
                        $( Box::new($sys.into_system()) ),*
                    ],
                    _marker: PhantomData,
                }
            }
        }

        impl<$($marker,)*> IntoSystemConfigs<($($marker,)*)> for SystemConfigs<($($marker,)*)>
        {
            fn into_configs(self) -> SystemConfigs<($($marker,)*)> {
                self
            }
        }
    };
}

impl_system_configs_tuple!(S1 ; M1);
impl_system_configs_tuple!(S1, S2 ; M1, M2);
impl_system_configs_tuple!(S1, S2, S3 ; M1, M2, M3);
impl_system_configs_tuple!(S1, S2, S3, S4; M1, M2, M3, M4);
impl_system_configs_tuple!(S1, S2, S3, S4, S5; M1, M2, M3, M4, M5);
impl_system_configs_tuple!(S1, S2, S3, S4, S5, S6; M1, M2, M3, M4, M5, M6);
impl_system_configs_tuple!(S1, S2, S3, S4, S5, S6, S7; M1, M2, M3, M4, M5, M6, M7);
impl_system_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8; M1, M2, M3, M4, M5, M6, M7, M8);
impl_system_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9; M1, M2, M3, M4, M5, M6, M7, M8, M9);
impl_system_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10; M1, M2, M3, M4, M5, M6, M7, M8, M9, M10);

impl<S, Marker> IntoSystemConfigs<Marker> for S
where
    S: IntoSystem<Marker> + 'static,
{
    fn into_configs(self) -> SystemConfigs<Marker> {
        SystemConfigs {
            systems: vec![Box::new(self.into_system())],
            _marker: PhantomData,
        }
    }
}

impl IntoSystemConfigs<()> for SystemConfigs<()> {
    fn into_configs(self) -> SystemConfigs<()> {
        self
    }
}
