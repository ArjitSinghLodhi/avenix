use crate::{
    ecs::world::World,
    system::{
        ParallelSystemParam,
        condition::Condition,
        system_set::{SetConditionBuilder, SystemSet},
        system_traits::{AsSchedulingTarget, SystemOrderings},
    },
};

#[doc(hidden)]
pub struct SetConfig {
    pub(crate) set: Box<dyn SystemSet>,
    pub(crate) run_after_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_before_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_after_systems: Vec<std::any::TypeId>,
    pub(crate) run_before_systems: Vec<std::any::TypeId>,
    pub(crate) member_of: Vec<Box<dyn SystemSet>>,
    pub(crate) condition_builders: Vec<SetConditionBuilder>,
}

#[doc(hidden)]
pub trait IntoSystemSetConfig {
    #[doc(hidden)]
    fn into_config(self) -> SetConfig;
}

impl IntoSystemSetConfig for SetConfig {
    fn into_config(self) -> SetConfig {
        self
    }
}

impl<T: SystemSet> IntoSystemSetConfig for T {
    fn into_config(self) -> SetConfig {
        SetConfig {
            set: Box::new(self),
            run_after_sets: Vec::new(),
            run_before_sets: Vec::new(),
            run_after_systems: Vec::new(),
            run_before_systems: Vec::new(),
            member_of: Vec::new(),
            condition_builders: Vec::new(),
        }
    }
}

impl IntoSystemSetConfig for Box<dyn SystemSet> {
    fn into_config(self) -> SetConfig {
        SetConfig {
            set: self,
            run_after_sets: Vec::new(),
            run_before_sets: Vec::new(),
            run_after_systems: Vec::new(),
            run_before_systems: Vec::new(),
            member_of: Vec::new(),
            condition_builders: Vec::new(),
        }
    }
}

#[doc(hidden)]
pub trait SystemSetConfigsExt {
    fn after<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemSetConfigs;
    fn before<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemSetConfigs;
    fn in_set(self, parent: impl SystemSet + Clone) -> SystemSetConfigs;
    fn chain(self) -> SystemSetConfigs;
    fn run_if<P, Cond>(self, condition: Cond) -> SystemSetConfigs
    where
        P: ParallelSystemParam + 'static,
        Cond: Condition<P> + Send + Sync + Clone + 'static;
}

#[doc(hidden)]
pub trait TrackSetHierarchyEdges {
    #[doc(hidden)]
    fn collect_set_edge_ids(&self, sets: &mut Vec<Box<dyn SystemSet>>);
}

impl TrackSetHierarchyEdges for SetConfig {
    fn collect_set_edge_ids(&self, sets: &mut Vec<Box<dyn SystemSet>>) {
        sets.push(self.set.clone_box());
    }
}

impl TrackSetHierarchyEdges for SystemSetConfigs {
    fn collect_set_edge_ids(&self, sets: &mut Vec<Box<dyn SystemSet>>) {
        for config in &self.configs {
            sets.push(config.set.clone_box());
        }
    }
}

impl<T: SystemSet> TrackSetHierarchyEdges for T {
    fn collect_set_edge_ids(&self, sets: &mut Vec<Box<dyn SystemSet>>) {
        sets.push(self.clone_box());
    }
}

#[doc(hidden)]
pub trait SystemSetExt: SystemSet + Sized {
    fn after<T>(self, target: impl AsSchedulingTarget<T>) -> SetConfig {
        let mut config = SetConfig {
            set: Box::new(self),
            run_after_sets: Vec::new(),
            run_before_sets: Vec::new(),
            run_after_systems: Vec::new(),
            run_before_systems: Vec::new(),
            member_of: Vec::new(),
            condition_builders: Vec::new(),
        };
        let mut orderings = SystemOrderings::default();
        target.populate_after(&mut orderings);
        config.run_after_sets = orderings.run_after_sets;
        config.run_after_systems = orderings.run_after_systems;
        config
    }

    fn before<T>(self, target: impl AsSchedulingTarget<T>) -> SetConfig {
        let mut config = SetConfig {
            set: Box::new(self),
            run_after_sets: Vec::new(),
            run_before_sets: Vec::new(),
            run_after_systems: Vec::new(),
            run_before_systems: Vec::new(),
            member_of: Vec::new(),
            condition_builders: Vec::new(),
        };
        let mut orderings = SystemOrderings::default();
        target.populate_before(&mut orderings);
        config.run_before_sets = orderings.run_before_sets;
        config.run_before_systems = orderings.run_before_systems;
        config
    }

    fn in_set(self, parent: impl SystemSet) -> SetConfig {
        SetConfig {
            set: Box::new(self),
            run_after_sets: Vec::new(),
            run_before_sets: Vec::new(),
            run_after_systems: Vec::new(),
            run_before_systems: Vec::new(),
            member_of: vec![Box::new(parent)],
            condition_builders: Vec::new(),
        }
    }

    fn run_if<P, C>(self, cond: C) -> SetConfig
    where
        P: crate::system::ParallelSystemParam + 'static,
        C: crate::system::condition::Condition<P> + Send + Sync + Clone + 'static,
    {
        let mut config = SetConfig {
            set: Box::new(self),
            run_after_sets: Vec::new(),
            run_before_sets: Vec::new(),
            run_after_systems: Vec::new(),
            run_before_systems: Vec::new(),
            member_of: Vec::new(),
            condition_builders: Vec::new(),
        };
        let builder = Box::new(move |world: &mut World| {
            let typed_data = cond.init_condition_data(world);
            let gate: Box<dyn Fn() -> bool + Send + Sync> = Box::new(move || cond.run(&typed_data));
            gate
        });
        config.condition_builders.push(builder);
        config
    }
}

impl<T: SystemSet> SystemSetExt for T {}

impl SetConfig {
    pub fn after<T>(mut self, target: impl AsSchedulingTarget<T>) -> Self {
        let mut orderings = SystemOrderings::default();
        target.populate_after(&mut orderings);
        self.run_after_sets.extend(orderings.run_after_sets);
        self.run_after_systems.extend(orderings.run_after_systems);
        self
    }

    pub fn before<T>(mut self, target: impl AsSchedulingTarget<T>) -> Self {
        let mut orderings = SystemOrderings::default();
        target.populate_before(&mut orderings);
        self.run_before_sets.extend(orderings.run_before_sets);
        self.run_before_systems.extend(orderings.run_before_systems);
        self
    }

    pub fn in_set(mut self, parent: impl SystemSet) -> Self {
        self.member_of.push(Box::new(parent));
        self
    }

    pub fn run_if<P, C>(mut self, cond: C) -> Self
    where
        P: crate::system::ParallelSystemParam + 'static,
        C: crate::system::condition::Condition<P> + Send + Sync + Clone + 'static,
    {
        let builder = Box::new(move |world: &mut World| {
            let typed_data = cond.init_condition_data(world);
            let gate: Box<dyn Fn() -> bool + Send + Sync> = Box::new(move || cond.run(&typed_data));
            gate
        });
        self.condition_builders.push(builder);
        self
    }
}

#[doc(hidden)]
pub trait IntoSystemSetConfigConfigs {
    #[doc(hidden)]
    fn into_configs(self) -> Vec<SetConfig>;
}

impl IntoSystemSetConfigConfigs for SetConfig {
    fn into_configs(self) -> Vec<SetConfig> {
        vec![self]
    }
}

impl IntoSystemSetConfigConfigs for SystemSetConfigs {
    fn into_configs(self) -> Vec<SetConfig> {
        self.configs
    }
}

impl IntoSystemSetConfigConfigs for Box<dyn SystemSet> {
    fn into_configs(self) -> Vec<SetConfig> {
        vec![self.into_config()]
    }
}

impl<T: SystemSet> IntoSystemSetConfigConfigs for T {
    fn into_configs(self) -> Vec<SetConfig> {
        vec![self.into_config()]
    }
}

pub struct SystemSetConfigs {
    pub(crate) configs: Vec<SetConfig>,
}

impl SystemSetConfigsExt for SystemSetConfigs {
    fn after<T>(mut self, target: impl AsSchedulingTarget<T> + Clone) -> SystemSetConfigs {
        let mut orderings = SystemOrderings::default();
        target.populate_after(&mut orderings);
        for config in self.configs.iter_mut() {
            config
                .run_after_sets
                .extend(orderings.run_after_sets.iter().map(|s| s.clone_box()));
            config
                .run_after_systems
                .extend(orderings.run_after_systems.clone());
        }
        self
    }

    fn before<T>(mut self, target: impl AsSchedulingTarget<T> + Clone) -> SystemSetConfigs {
        let mut orderings = SystemOrderings::default();
        target.populate_before(&mut orderings);
        for config in self.configs.iter_mut() {
            config
                .run_before_sets
                .extend(orderings.run_before_sets.iter().map(|s| s.clone_box()));
            config
                .run_before_systems
                .extend(orderings.run_before_systems.clone());
        }
        self
    }

    fn in_set(mut self, parent: impl SystemSet + Clone) -> SystemSetConfigs {
        for config in self.configs.iter_mut() {
            config.member_of.push(Box::new(parent.clone()));
        }
        self
    }

    fn chain(mut self) -> SystemSetConfigs {
        let n = self.configs.len();
        for idx in 1..n {
            let prev_set = self.configs[idx - 1].set.clone_box();
            let config_mut = &mut self.configs[idx];
            if !config_mut
                .run_after_sets
                .iter()
                .any(|s| s.dyn_eq(&*prev_set))
            {
                config_mut.run_after_sets.push(prev_set);
            }
        }
        self
    }

    fn run_if<P, Cond>(mut self, condition: Cond) -> SystemSetConfigs
    where
        P: ParallelSystemParam + 'static,
        Cond: Condition<P> + Send + Sync + Clone + 'static,
    {
        for config in self.configs.iter_mut() {
            let cond_clone = condition.clone();
            let builder = Box::new(move |world: &mut World| {
                let typed_data = cond_clone.init_condition_data(world);
                let gate: Box<dyn Fn() -> bool + Send + Sync> =
                    Box::new(move || cond_clone.run(&typed_data));
                gate
            });
            config.condition_builders.push(builder);
        }
        self
    }
}

macro_rules! impl_system_set_configs_for_tuples {
    ($($set:ident),*) => {
        impl<$($set: IntoSystemSetConfigConfigs),*> IntoSystemSetConfigConfigs for ($($set,)*) {
            fn into_configs(self) -> Vec<SetConfig> {
                #[allow(non_snake_case)]
                let ($($set,)*) = self;
                let mut out = Vec::new();
                $(out.extend($set.into_configs());)*
                out
            }
        }

        impl<$($set: IntoSystemSetConfigConfigs + TrackSetHierarchyEdges + 'static),*> SystemSetConfigsExt for ($($set,)*) {
            fn after<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemSetConfigs {
                SystemSetConfigs { configs: self.into_configs() }.after(target)
            }

            fn before<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemSetConfigs {
                SystemSetConfigs { configs: self.into_configs() }.before(target)
            }

            fn in_set(self, parent: impl SystemSet + Clone) -> SystemSetConfigs {
                SystemSetConfigs { configs: self.into_configs() }.in_set(parent)
            }

            fn run_if<P, Cond>(self, cond: Cond) -> SystemSetConfigs
            where
                P: ParallelSystemParam + 'static,
                Cond: Condition<P> + Send + Sync + Clone + 'static,
            {
                SystemSetConfigs { configs: self.into_configs() }.run_if(cond)
            }
            #[allow(unused_assignments)]
            fn chain(self) -> SystemSetConfigs {
                #[allow(non_snake_case)]
                let ($($set,)*) = self;
                let mut flattened_configs = Vec::new();

                let mut previous_edges: Vec<Box<dyn SystemSet>> = Vec::new();

                $({
                    let mut current_edges = Vec::new();
                    $set.collect_set_edge_ids(&mut current_edges);

                    let mut current_config = $set.into_configs();

                    if !previous_edges.is_empty() {
                        for config_item in current_config.iter_mut() {
                            for prev_set in previous_edges.iter() {
                                if !config_item.run_after_sets.iter().any(|s| s.dyn_eq(&**prev_set)) {
                                    config_item.run_after_sets.push(prev_set.clone_box());
                                }
                            }
                        }
                    }

                    flattened_configs.extend(current_config);
                    previous_edges = current_edges;
                })*

                SystemSetConfigs { configs: flattened_configs }
            }
        }
    };
}

impl_system_set_configs_for_tuples!(S0);
impl_system_set_configs_for_tuples!(S0, S1);
impl_system_set_configs_for_tuples!(S0, S1, S2);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3, S4);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3, S4, S5);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3, S4, S5, S6);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7, S8);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7, S8, S9);
impl_system_set_configs_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7, S8, S9, S10);

macro_rules! impl_track_set_edges_for_tuples {
    ($($set:ident),*) => {
        impl<$($set: TrackSetHierarchyEdges),*> TrackSetHierarchyEdges for ($($set,)*) {
            fn collect_set_edge_ids(&self, sets: &mut Vec<Box<dyn SystemSet>>) {
                #[allow(non_snake_case)]
                let ($($set,)*) = self;
                $( $set.collect_set_edge_ids(sets); )*
            }
        }
    };
}

impl_track_set_edges_for_tuples!(S0);
impl_track_set_edges_for_tuples!(S0, S1);
impl_track_set_edges_for_tuples!(S0, S1, S2);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3, S4);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3, S4, S5);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3, S4, S5, S6);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7, S8);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7, S8, S9);
impl_track_set_edges_for_tuples!(S0, S1, S2, S3, S4, S5, S6, S7, S8, S9, S10);
