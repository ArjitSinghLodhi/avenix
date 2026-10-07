use crate::system::{
    system_set::{SetConditionBuilder, SystemSet},
    system_traits::{AsSchedulingTarget, SystemOrderings},
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

pub trait SystemSetExt: SystemSet + Sized {
    fn after<TM>(self, target: impl AsSchedulingTarget<TM>) -> SetConfig {
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

    fn before<TM>(self, target: impl AsSchedulingTarget<TM>) -> SetConfig {
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
        let builder = Box::new(move |world: &mut crate::world::storage::World| {
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
    pub fn after<TM>(mut self, target: impl AsSchedulingTarget<TM>) -> Self {
        let mut orderings = SystemOrderings::default();
        target.populate_after(&mut orderings);
        self.run_after_sets.extend(orderings.run_after_sets);
        self.run_after_systems.extend(orderings.run_after_systems);
        self
    }

    pub fn before<TM>(mut self, target: impl AsSchedulingTarget<TM>) -> Self {
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
        let builder = Box::new(move |world: &mut crate::world::storage::World| {
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

impl IntoSystemSetConfigConfigs for Vec<SetConfig> {
    fn into_configs(self) -> Vec<SetConfig> {
        self
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

#[doc(hidden)]
pub trait SystemSetConfigsExt {
    fn after<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> Vec<SetConfig>;
    fn before<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> Vec<SetConfig>;
    fn in_set(self, parent: impl SystemSet + Clone) -> Vec<SetConfig>;
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

        impl<$($set: IntoSystemSetConfigConfigs),*> SystemSetConfigsExt for ($($set,)*) {
            fn after<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> Vec<SetConfig> {
                let mut configs = self.into_configs();
                let mut orderings = crate::system::system_traits::SystemOrderings::default();
                target.populate_after(&mut orderings);
                for config in configs.iter_mut() {
                    config.run_after_sets.extend(orderings.run_after_sets.iter().map(|s| s.clone_box()));
                    config.run_after_systems.extend(orderings.run_after_systems.clone());
                }
                configs
            }

            fn before<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> Vec<SetConfig> {
                let mut configs = self.into_configs();
                let mut orderings = crate::system::system_traits::SystemOrderings::default();
                target.populate_before(&mut orderings);
                for config in configs.iter_mut() {
                    config.run_before_sets.extend(orderings.run_before_sets.iter().map(|s| s.clone_box()));
                    config.run_before_systems.extend(orderings.run_before_systems.clone());
                }
                configs
            }

            fn in_set(self, parent: impl SystemSet + Clone) -> Vec<SetConfig> {
                let mut configs = self.into_configs();
                for config in configs.iter_mut() {
                    config.member_of.push(Box::new(parent.clone()));
                }
                configs
            }
        }
    };
}

impl_system_set_configs_for_tuples!(A);
impl_system_set_configs_for_tuples!(A, B);
impl_system_set_configs_for_tuples!(A, B, C);
impl_system_set_configs_for_tuples!(A, B, C, D);
impl_system_set_configs_for_tuples!(A, B, C, D, E);
impl_system_set_configs_for_tuples!(A, B, C, D, E, F);
impl_system_set_configs_for_tuples!(A, B, C, D, E, F, G);
impl_system_set_configs_for_tuples!(A, B, C, D, E, F, G, H);
impl_system_set_configs_for_tuples!(A, B, C, D, E, F, G, H, I);
impl_system_set_configs_for_tuples!(A, B, C, D, E, F, G, H, I, J);
impl_system_set_configs_for_tuples!(A, B, C, D, E, F, G, H, I, J, K);
impl_system_set_configs_for_tuples!(A, B, C, D, E, F, G, H, I, J, K, L);
