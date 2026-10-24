#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[path = "app/mod.rs"]
mod app_impl;
mod commands;
mod component;
mod entity;
mod entity_registry;
mod events;
mod query;
#[cfg(feature = "reactivity")]
mod reactivity;
mod resources;
mod schedule;
mod states;
mod system;
mod world;

pub mod derive {
    pub use avenix_macros::{
        Component, ComponentBundle, Event, ParallelSystemParam, QueryData, QueryFilter, Resource,
        ScheduleLabel, States, SystemParam, SystemSet,
    };
}

pub use indexmap;
pub use rayon;
pub use rustc_hash;

pub mod prelude {
    pub use crate::app::{
        App,
        plugin::{Plugin, PluginsBuildAll},
        schedule::{
            CleanupHandles, First, Last, PostUpdate, PreUpdate, Schedule, ScheduleLabel, Startup,
            Update,
        },
        states::{NextState, State, States, entered_state, exited_state, in_state},
        system::{
            FunctionIdentity, IntoSystem, System, SystemIdentity, SystemMeta, SystemParam,
            condition::{Condition, ConditionFn, RunConditionsList, not, resource_exists},
            system_set::{IntoSystemSetConfigs, SystemSet, SystemSetConfigsExt, SystemSetExt},
            system_traits::{
                ConfigureSystem, ConfigureSystemConfigs, IntoSystemConfigs, SystemConfigs,
            },
        },
    };
    pub use crate::derive::{
        Component, ComponentBundle, Event, ParallelSystemParam, QueryData, QueryFilter, Resource,
        ScheduleLabel, States, SystemParam, SystemSet,
    };
    #[cfg(feature = "reactivity")]
    pub use crate::ecs::reactivity::{
        added::{Added, AddedTracker},
        changed::{Changed, ChangedRef, ChangedRefMut, ChangedTracker, Mut},
        removed::RemovedComponents,
    };
    pub use crate::ecs::{
        Component,
        commands::{Commands, EntityCommands, ParallelCommands, bundle::ComponentBundle},
        entity::Entity,
        events::{
            EnumeratePar, Event, EventReader, EventWriter, Par, ParallelEventReader,
            ParallelEventWriter,
        },
        query::{
            Has, ParallelQueryAccessor, Query, QueryData, QueryMutableChunk, QueryReadOnlyChunk,
            QuerySingleError,
            filter::{EmptyQueryFilter, Not, Or, QueryFilter, With, Without},
            single::{Single, SingleMut},
        },
        resources::{NonSend, NonSendMut, ParallelResourceAccessor, Res, ResMut, Resource},
        world::World,
    };

    pub use crate::rayon::prelude::*;
}

pub mod ecs {
    pub use crate::component::Component;
    pub mod events {
        pub use crate::events::{
            EnumeratePar, Event, EventReader, EventWriter, Par, ParallelEventReader,
            ParallelEventWriter,
        };
    }
    pub mod resources {
        pub use crate::resources::{
            NonSend, NonSendMut, ParallelResourceAccessor, Res, ResMut, Resource,
        };
    }
    pub mod query {
        pub use crate::query::{
            Has, MatchedArchetypeData, Query, QueryData, QueryMutableChunk, QueryReadOnlyChunk,
            QuerySingleError,
        };
        pub mod filter {
            pub use crate::query::filter::{EmptyQueryFilter, Not, Or, QueryFilter, With, Without};
        }

        pub mod single {
            pub use crate::query::single::{Single, SingleMut};
        }
        pub use crate::query::parallel_query::ParallelQueryAccessor;
    }
    #[cfg(feature = "reactivity")]
    #[cfg_attr(docsrs, doc(cfg(feature = "reactivity")))]
    /// # Component Reactivity Architecture
    ///
    /// When the `reactivity` feature is active, Avenix uses a demand-driven model to minimize runtime overhead.
    ///
    /// * **Lazy Tracking Allocations:** Double-buffered tracking queues are not allocated for every component
    ///   type by default. They are registered and initialized only if a system explicitly requests them
    ///   (e.g., via [`RemovedComponents<T>`], [`Changed<T>`], [`ChangedRefMut<T>`], or [`Added<T>`])
    ///   or a parallel handle (e.g., [`ParallelQueryAccessor`]) requests it in its parameters before [`app.build()`],
    ///   or they are manually registered through [`World`] or [`App`].
    /// * **Optimized Runtime Pathways:** Component types that are never used in reactive query filters skip
    ///   frame-boundary memory swaps and tracking management entirely, keeping untracked data paths completely
    ///   unburdened by default.
    /// * **Automated Static Discovery:** During the conversion into systems before finalizing the [`App`], the
    ///   engine automatically analyzes all system parameters, parameters inside [`ParallelQueryAccessor`]
    ///   (when registered before [`app.build()`]), and [`RemovedComponents<T>`] parameters. It
    ///   then auto-registers all necessary trackers by itself without requiring any manual initialization.
    ///
    /// For any tracking data requirements introduced dynamically post-build, explicit tracker registerers
    /// are provided on both [`App`] and [`World`] to be called before [`app.build()`].
    ///
    /// [`App`]: crate::app::App
    /// [`World`]: crate::world::World
    /// [`RemovedComponents<T>`]: crate::reactivity::removed::RemovedComponents
    /// [`Changed<T>`]: crate::reactivity::changed::Changed<T>
    /// [`ChangedRefMut<T>`]: crate::reactivity::changed::ChangedRefMut
    /// [`Added<T>`]: crate::reactivity::added::Added<T>
    /// [`ParallelQueryAccessor`]: crate::ecs::query::ParallelQueryAccessor
    /// [`app.build()`]: crate::app::App::build
    pub mod reactivity {
        pub mod changed {
            pub use crate::reactivity::{Changed, ChangedRef, ChangedRefMut, ChangedTracker, Mut};
        }
        pub mod added {
            pub use crate::reactivity::{Added, AddedTracker};
        }
        pub mod removed {
            pub use crate::reactivity::RemovedComponents;
        }
    }
    pub mod commands {
        pub use crate::commands::{Commands, EntityCommands, ParallelCommands};
        pub mod bundle {
            pub use crate::commands::bundle::ComponentBundle;
        }
    }
    pub mod world {
        pub use crate::world::World;
        pub mod archetypes {
            pub use crate::world::archetypes::{
                Archetype, ComponentColumn, ComponentColumnRead, ComponentColumnWrite,
            };
        }
    }
    pub mod entity {
        pub use crate::entity::Entity;
    }
}

pub mod app {
    pub use crate::app_impl::App;
    pub mod system {
        pub use crate::system::{
            AccessHashSet, AccessVec, FunctionIdentity, FunctionSystem, IntoSystem,
            ParallelSystemParam, System, SystemIdentity, SystemMeta, SystemParam,
            system_storage::{SystemData, SystemExt},
        };
        pub mod condition {
            pub use crate::system::condition::{
                Condition, ConditionFn, RunConditionsList, not, resource_exists,
            };
        }
        pub mod system_traits {
            pub use crate::system::system_traits::{
                AsSchedulingTarget, ConfigureSystem, ConfigureSystemConfigs, FunctionTargetMarker,
                IntoSystemConfigs, SystemConfigs,
            };
        }

        pub mod system_set {
            pub use crate::system::{
                system_set::{GuardIdentity, GuardNode, SystemSet},
                system_set_traits::{
                    IntoSystemSetConfig, IntoSystemSetConfigs, SetConfig, SystemSetConfigsExt,
                    SystemSetExt,
                },
            };
        }
    }
    pub mod schedule {
        pub use crate::schedule::{
            CleanupHandles, DefaultSchedulesPlugin, First, Last, PostUpdate, PreUpdate, Schedule,
            ScheduleLabel, Startup, SystemExecutor, SystemNode, SystemsSchedule, Update,
            dyn_eq::DynEq,
            executors::{
                multi_threaded::MultiThreadedExecutor, single_threaded::SingleThreadedExecutor,
            },
        };
    }
    pub mod plugin {
        pub use crate::app_impl::{Plugin, PluginsBuildAll};
    }
    pub mod states {
        pub use crate::states::{NextState, State, States, entered_state, exited_state, in_state};
    }
}
