use std::any::{Any, TypeId};

use rustc_hash::FxHashMap;

#[doc(hidden)]
pub trait SystemData {
    #[doc(hidden)]
    fn get_raw(&self, id: TypeId) -> Option<&Box<dyn Any>>;
    #[doc(hidden)]
    fn get_raw_mut(&mut self, id: TypeId) -> Option<&mut Box<dyn Any>>;
    #[doc(hidden)]
    fn insert_raw(&mut self, id: TypeId, value: Box<dyn Any>);
}

#[doc(hidden)]
pub trait SystemExt {
    fn get_data<T: 'static>(&self) -> Option<&T>;
    fn get_data_mut<T: 'static>(&mut self) -> Option<&mut T>;
    fn insert<T: 'static>(&mut self, value: T);
    fn get_or_init<T: 'static>(&mut self, init: impl FnOnce() -> T) -> &T;
    fn get_or_init_mut<T: 'static>(&mut self, init: impl FnOnce() -> T) -> &mut T;
}

impl<S: SystemData + ?Sized> SystemExt for S {
    fn get_data<T: 'static>(&self) -> Option<&T> {
        self.get_raw(TypeId::of::<T>())
            .and_then(|any| any.downcast_ref::<T>())
    }

    fn get_data_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.get_raw_mut(TypeId::of::<T>())
            .and_then(|any| any.downcast_mut::<T>())
    }

    fn insert<T: 'static>(&mut self, value: T) {
        self.insert_raw(TypeId::of::<T>(), Box::new(value));
    }

    fn get_or_init<T>(&mut self, init: impl FnOnce() -> T) -> &T
    where
        T: 'static,
    {
        let id = TypeId::of::<T>();
        if self.get_raw(id).is_none() {
            self.insert_raw(id, Box::new(init()));
        }
        self.get_data::<T>().unwrap()
    }

    fn get_or_init_mut<T>(&mut self, init: impl FnOnce() -> T) -> &mut T
    where
        T: 'static,
    {
        let id = TypeId::of::<T>();
        if self.get_raw(id).is_none() {
            self.insert_raw(id, Box::new(init()));
        }
        self.get_data_mut::<T>().unwrap()
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct FunctionData {
    data: FxHashMap<TypeId, Box<dyn Any>>,
}

impl FunctionData {
    pub(crate) fn new() -> Self {
        Self {
            data: FxHashMap::default(),
        }
    }
    pub fn get_data<T: 'static>(&self) -> Option<&T> {
        self.data
            .get(&TypeId::of::<T>())
            .and_then(|any| any.downcast_ref::<T>())
    }

    pub fn get_data_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.data
            .get_mut(&TypeId::of::<T>())
            .and_then(|any| any.downcast_mut::<T>())
    }
    pub fn get_or_init<T: 'static>(&mut self, init: impl FnOnce() -> T) -> &T {
        let id = TypeId::of::<T>();
        let entry = self.data.entry(id).or_insert_with(|| Box::new(init()));
        entry.downcast_ref::<T>().unwrap()
    }

    pub fn get_or_init_mut<T: 'static>(&mut self, init: impl FnOnce() -> T) -> &mut T {
        let id = TypeId::of::<T>();
        let entry = self.data.entry(id).or_insert_with(|| Box::new(init()));
        entry.downcast_mut::<T>().unwrap()
    }
    pub fn insert<T: 'static>(&mut self, value: T) {
        let id = std::any::TypeId::of::<T>();
        self.data.insert(id, Box::new(value));
    }

    pub(crate) fn get_raw_data(&self, type_id: &TypeId) -> Option<&Box<dyn Any>> {
        self.data.get(type_id)
    }

    pub(crate) fn get_raw_data_mut(&mut self, type_id: &TypeId) -> Option<&mut Box<dyn Any>> {
        self.data.get_mut(type_id)
    }
    pub(crate) fn insert_raw_data(&mut self, type_id: &TypeId, value: Box<dyn Any>) {
        self.data.insert(*type_id, value);
    }
}
