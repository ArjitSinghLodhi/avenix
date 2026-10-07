use std::any::Any;
#[doc(hidden)]
pub trait DynEq: Any {
    #[doc(hidden)]
    fn as_any(&self) -> &dyn Any;
    #[doc(hidden)]
    fn dyn_eq(&self, other: &dyn DynEq) -> bool;
}

impl<T: Any + PartialEq> DynEq for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dyn_eq(&self, other: &dyn DynEq) -> bool {
        if let Some(other_concrete) = other.as_any().downcast_ref::<Self>() {
            self == other_concrete
        } else {
            false
        }
    }
}
