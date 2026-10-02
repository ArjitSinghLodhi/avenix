#![cfg(feature = "reactivity")]

use crate::{app_impl::APP_BUILT, extensions::ComponentColumn};
use indexmap::IndexMap;
use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::FxBuildHasher;
use std::any::{Any, TypeId};

mod added;
mod changed;
mod removed;

fn verify_app_not_build_yet<'a>() -> RwLockReadGuard<'a, bool> {
    let built = APP_BUILT.read();
    if *built {
        panic!("App already built when trying to register a tracked component");
    } else {
        built
    }
}

pub(crate) struct TrackedComponentMeta {
    pub(crate) marker_id: TypeId,
    pub(crate) create_marker_column: fn() -> ComponentColumn,
    pub(crate) push_default_marker: unsafe fn(&mut ComponentColumn),
    pub(crate) clear_column_markers: unsafe fn(&mut dyn Any),
}

pub(crate) static TRACKED_COMPONENTS: RwLock<
    IndexMap<TypeId, Vec<TrackedComponentMeta>, FxBuildHasher>,
> = RwLock::new(IndexMap::with_hasher(FxBuildHasher));

pub use changed::{Changed, ChangedTracker};

pub(crate) use changed::{ChangedMarker, Mut};

pub use added::{Added, AddedTracker};

pub use removed::RemovedComponents;

pub(crate) use removed::{REMOVAL_TRACKED_COMPS, register_removal_tracking_buffers};
