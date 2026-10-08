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
            IntoSystem, System, SystemMeta, SystemParam,
            condition::{
                Condition, ConditionBuilder, ConditionFn, RunConditionsList, not, resource_exists,
            },
            system_set::{
                IntoSystemSetConfigConfigs, SystemSet, SystemSetConfigsExt, SystemSetExt,
            },
            system_traits::{
                ConfigureSystem, ConfigureSystemConfigs, IntoSystemConfigs, SystemConfigs,
            },
        },
    };
    pub use crate::derive::{
        Component, ComponentBundle, Event, ParallelSystemParam, QueryData, QueryFilter, Resource,
        ScheduleLabel, States, SystemParam, SystemSet,
    };
    #[cfg(feature = "events")]
    pub use crate::ecs::events::{
        EnumeratePar, Event, EventReader, EventWriter, Par, ParallelEventReader,
        ParallelEventWriter,
    };
    #[cfg(feature = "reactivity")]
    pub use crate::ecs::reactivity::{
        added::{Added, AddedTracker},
        changed::{Changed, ChangedTracker},
        removed::RemovedComponents,
    };
    pub use crate::ecs::{
        Component,
        commands::{Commands, EntityCommands, ParallelCommands, bundle::ComponentBundle},
        entity::Entity,
        query::{
            Has, Query, QueryArchetypeView, QueryData, QuerySubChunk,
            filter::{EmptyQueryFilter, Not, Or, QueryFilter, With, Without},
            parallel_query::ParallelQueryAccessor,
        },
        resources::{NonSend, NonSendMut, ParallelResourceAccessor, Res, ResMut, Resource},
        world::World,
    };
}

pub mod extensions {
    pub use crate::system::system_storage::{FunctionData, SystemData, SystemExt};
    pub use crate::system::{
        AccessHashSet, AccessVec, FunctionSystem, IntoSystem, System, SystemMeta, SystemParam,
    };
    pub use crate::world::archetypes::{Archetype, ComponentColumn};
    pub use crate::world::archetypes::{ComponentColumnRead, ComponentColumnWrite};
    pub use crate::world::storage::World;
}

pub mod ecs {
    pub use crate::component::Component;
    #[cfg(feature = "events")]
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
        pub use crate::query::{Has, Query, QueryArchetypeView, QueryData, QuerySubChunk};
        pub mod filter {
            pub use crate::query::filter::{EmptyQueryFilter, Not, Or, QueryFilter, With, Without};
        }
        pub mod parallel_query {
            pub use crate::query::parallel_query::ParallelQueryAccessor;
        }
    }
    #[cfg(feature = "reactivity")]
    pub mod reactivity {
        pub mod changed {
            pub use crate::reactivity::{Changed, ChangedTracker};
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
        pub use crate::world::storage::World;
    }
    pub mod entity {
        pub use crate::entity::Entity;
    }
}

pub mod app {
    pub use crate::app_impl::App;
    pub mod system {
        pub use crate::system::{IntoSystem, ParallelSystemParam, System, SystemMeta, SystemParam};
        pub mod condition {
            pub use crate::system::condition::{
                Condition, ConditionBuilder, ConditionFn, RunConditionsList, not, resource_exists,
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
                system_set::{GuardNode, SetConditionBuilder, SystemSet},
                system_set_traits::{
                    IntoSystemSetConfig, IntoSystemSetConfigConfigs, SetConfig,
                    SystemSetConfigsExt, SystemSetExt,
                },
            };
        }
    }
    pub mod schedule {
        pub use crate::schedule::{
            CleanupHandles, First, Last, PostUpdate, PreUpdate, Schedule, ScheduleLabel, Startup,
            SystemExecutor, SystemNode, SystemsSchedule, Update,
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
