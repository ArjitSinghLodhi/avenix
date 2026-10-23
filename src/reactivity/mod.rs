#![cfg(feature = "reactivity")]

use crate::{app_impl::APP_BUILT, world::archetypes::ComponentColumn};
use indexmap::IndexMap;
use parking_lot::{Mutex, RwLockReadGuard};
use rustc_hash::FxBuildHasher;
use std::any::{Any, TypeId};

pub(crate) mod added;
pub(crate) mod changed;
pub(crate) mod removed;

pub(crate) fn verify_app_not_built_yet<'a>(context: &str) -> RwLockReadGuard<'a, bool> {
    let built = APP_BUILT.read();
    if *built {
        panic!(
            "AvenixLazyReactivityViolation\n\
            -> App was already built when trying to register a tracked component pipeline.\n\
            -> Context: {}",
            context
        );
    }
    built
}

pub(crate) struct TrackedComponentMeta {
    pub(crate) marker_id: TypeId,
    pub(crate) create_marker_column: fn() -> ComponentColumn,
    pub(crate) push_default_marker: unsafe fn(&mut ComponentColumn),
    pub(crate) clear_column_markers: unsafe fn(&mut dyn Any),
}

pub(crate) static TRACKED_COMPONENTS: Mutex<
    IndexMap<TypeId, Vec<TrackedComponentMeta>, FxBuildHasher>,
> = Mutex::new(IndexMap::with_hasher(FxBuildHasher));

pub use changed::{Changed, ChangedRef, ChangedRefMut, ChangedTracker, Mut};

pub(crate) use changed::ChangedMarker;

pub use added::{Added, AddedTracker};

pub use removed::RemovedComponents;

pub(crate) use removed::{REMOVAL_TRACKED_COMPS, register_removal_tracking_buffers};
