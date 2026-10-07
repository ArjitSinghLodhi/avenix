use crate::app::system::SystemParam;
use crate::extensions::FunctionSystem;
use crate::system::IntoSystemConfigs;
use crate::system::System;
use crate::system::condition::{Condition, RunConditionsList};
use crate::system::system_set::SystemSet;
use crate::system::system_storage::SystemExt;
use crate::system::{IntoSystem, SystemConfigs};

// System Traits.

#[doc(hidden)]
pub trait SystemCondition<Marker> {
    #[doc(hidden)]
    type SystemType: System + 'static;
    fn run_if<P>(self, condition: impl Condition<P>) -> Self::SystemType;
    fn in_set(self, set: impl SystemSet + Clone) -> Self::SystemType;
}

#[derive(Default, Clone)]
pub struct SystemOrderings {
    pub(crate) run_after_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_before_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) member_of: Vec<Box<dyn SystemSet>>,
    pub(crate) run_after_systems: Vec<std::any::TypeId>,
    pub(crate) run_before_systems: Vec<std::any::TypeId>,
}

pub trait AsSchedulingTarget<TM> {
    fn populate_before(&self, orderings: &mut SystemOrderings);
    fn populate_after(&self, orderings: &mut SystemOrderings);
}

impl<T: SystemSet + Clone> AsSchedulingTarget<()> for T {
    fn populate_before(&self, orderings: &mut SystemOrderings) {
        orderings.run_before_sets.push(Box::new(self.clone()));
    }

    fn populate_after(&self, orderings: &mut SystemOrderings) {
        orderings.run_after_sets.push(Box::new(self.clone()));
    }
}

#[doc(hidden)]
pub trait SystemOrder<Marker> {
    #[doc(hidden)]
    type SystemType: System + 'static;
    fn before<TM>(self, target: impl AsSchedulingTarget<TM>) -> Self::SystemType;
    fn after<TM>(self, target: impl AsSchedulingTarget<TM>) -> Self::SystemType;
}

pub struct FunctionTargetMarker<P, F>(std::marker::PhantomData<(P, F)>);

macro_rules! impl_traits_for_function {
    ($($param:ident),*) => {
        impl<$($param,)* F> AsSchedulingTarget<FunctionTargetMarker<($($param,)*), F>> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            fn populate_before(&self, orderings: &mut SystemOrderings) {
                orderings.run_before_systems.push(std::any::TypeId::of::<F>());
            }

            fn populate_after(&self, orderings: &mut SystemOrderings) {
                orderings.run_after_systems.push(std::any::TypeId::of::<F>());
            }
        }

        impl<$($param,)* F> SystemCondition<($($param,)*)> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;
            fn run_if<P>(self, condition: impl Condition<P>) -> FunctionSystem<($($param,)*), F> {
                self.into_system().run_if(condition)
            }

            fn in_set(self, set: impl SystemSet + Clone) -> FunctionSystem<($($param,)*), F> {
                self.into_system().in_set(set)
            }
        }

        impl<$($param,)* F> SystemOrder<($($param,)*)> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;

            fn before<TM>(self, target: impl AsSchedulingTarget<TM>) -> Self::SystemType {
                self.into_system().before(target)
            }

            fn after<TM>(self, target: impl AsSchedulingTarget<TM>) -> Self::SystemType {
                self.into_system().after(target)
            }
        }
    }
}

impl_traits_for_function!(A);
impl_traits_for_function!(A, B);
impl_traits_for_function!(A, B, C);
impl_traits_for_function!(A, B, C, D);
impl_traits_for_function!(A, B, C, D, E);
impl_traits_for_function!(A, B, C, D, E, G);
impl_traits_for_function!(A, B, C, D, E, G, H);
impl_traits_for_function!(A, B, C, D, E, G, H, I);
impl_traits_for_function!(A, B, C, D, E, G, H, I, J);
impl_traits_for_function!(A, B, C, D, E, G, H, I, J, K);
impl_traits_for_function!(A, B, C, D, E, G, H, I, J, K, L);
impl_traits_for_function!(A, B, C, D, E, G, H, I, J, K, L, M);

impl<F> AsSchedulingTarget<FunctionTargetMarker<(), F>> for F
where
    F: Fn() + 'static,
{
    fn populate_before(&self, orderings: &mut SystemOrderings) {
        orderings
            .run_before_systems
            .push(std::any::TypeId::of::<F>());
    }

    fn populate_after(&self, orderings: &mut SystemOrderings) {
        orderings
            .run_after_systems
            .push(std::any::TypeId::of::<F>());
    }
}

impl<F> SystemCondition<()> for F
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;
    fn run_if<P>(self, condition: impl Condition<P>) -> FunctionSystem<(), F> {
        self.into_system().run_if(condition)
    }

    fn in_set(self, set: impl SystemSet + Clone) -> FunctionSystem<(), F> {
        self.into_system().in_set(set)
    }
}

impl<F> SystemOrder<()> for F
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;

    fn before<TM>(self, target: impl AsSchedulingTarget<TM>) -> Self::SystemType {
        self.into_system().before(target)
    }

    fn after<TM>(self, target: impl AsSchedulingTarget<TM>) -> Self::SystemType {
        self.into_system().after(target)
    }
}

macro_rules! impl_traits_for_function_system {
    ($($param:ident),*) => {
        impl<$($param,)* F> SystemCondition<($($param,)*)> for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;
            fn run_if<P>(self, condition: impl Condition<P>) -> FunctionSystem<($($param,)*), F> {
                let mut system = self.into_system();
                system.get_or_init_mut::<RunConditionsList>(RunConditionsList::default).add_condition(condition);
                system
            }

            fn in_set(self, set: impl SystemSet + Clone) -> FunctionSystem<($($param,)*), F> {
                let mut system = self.into_system();
                system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default).member_of.push(Box::new(set));
                system
            }
        }

        impl<$($param,)* F> SystemOrder<($($param,)*)> for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;

            fn before<TM>(self, target: impl AsSchedulingTarget<TM>) -> FunctionSystem<($($param,)*), F> {
                let mut system = self.into_system();
                target.populate_before(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                system
            }

            fn after<TM>(self, target: impl AsSchedulingTarget<TM>) -> FunctionSystem<($($param,)*), F> {
                let mut system = self.into_system();
                target.populate_after(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                system
            }
        }
    };
}

impl_traits_for_function_system!(A);
impl_traits_for_function_system!(A, B);
impl_traits_for_function_system!(A, B, C);
impl_traits_for_function_system!(A, B, C, D);
impl_traits_for_function_system!(A, B, C, D, E);
impl_traits_for_function_system!(A, B, C, D, E, G);
impl_traits_for_function_system!(A, B, C, D, E, G, H);
impl_traits_for_function_system!(A, B, C, D, E, G, H, I);
impl_traits_for_function_system!(A, B, C, D, E, G, H, I, J);
impl_traits_for_function_system!(A, B, C, D, E, G, H, I, J, K);
impl_traits_for_function_system!(A, B, C, D, E, G, H, I, J, K, L);
impl_traits_for_function_system!(A, B, C, D, E, G, H, I, J, K, L, M);

impl<F> SystemCondition<()> for FunctionSystem<(), F>
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;
    fn run_if<P>(self, condition: impl Condition<P>) -> FunctionSystem<(), F> {
        let mut system = self.into_system();
        system
            .get_or_init_mut::<RunConditionsList>(RunConditionsList::default)
            .add_condition(condition);
        system
    }

    fn in_set(self, set: impl SystemSet + Clone) -> FunctionSystem<(), F> {
        let mut system = self.into_system();
        system
            .get_or_init_mut::<SystemOrderings>(SystemOrderings::default)
            .member_of
            .push(Box::new(set));
        system
    }
}

impl<F> SystemOrder<()> for FunctionSystem<(), F>
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;

    fn before<TM>(self, target: impl AsSchedulingTarget<TM>) -> FunctionSystem<(), F> {
        let mut system = self.into_system();
        target.populate_before(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
        system
    }

    fn after<TM>(self, target: impl AsSchedulingTarget<TM>) -> FunctionSystem<(), F> {
        let mut system = self.into_system();
        target.populate_after(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
        system
    }
}

// System Configs Traits.

#[doc(hidden)]
pub trait SystemConfigsCondition<MarkerGroup> {
    fn run_if<P>(self, condition: impl Condition<P> + Clone) -> SystemConfigs<MarkerGroup>;
    fn in_set(self, set: impl SystemSet + Clone) -> SystemConfigs<MarkerGroup>;
}

#[doc(hidden)]
pub trait SystemConfigsOrder<MarkerGroup> {
    fn before<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> SystemConfigs<MarkerGroup>;
    fn after<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> SystemConfigs<MarkerGroup>;
}

#[doc(hidden)]
pub trait SystemsChain<MarkerGroup> {
    fn chain(self) -> SystemConfigs<MarkerGroup>;
}

macro_rules! impl_traits_for_function_system_configs {
    ($($sys:ident),* ; $($marker:ident),*) => {
        impl<$($sys,)* $($marker,)*> SystemConfigsCondition<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystem<$marker> + 'static + SystemCondition<$marker> ),*
        {
            fn run_if<P>(self, condition: impl Condition<P> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().run_if(condition)
            }

            fn in_set(self, set: impl SystemSet + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().in_set(set)
            }
        }

        impl<$($sys,)* $($marker,)*> SystemConfigsOrder<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystem<$marker> + 'static + SystemOrder<$marker> ),*
        {
            fn before<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().before(target)
            }

            fn after<TM>(self, target: impl AsSchedulingTarget<TM> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().after(target)
            }
        }

        impl<$($sys,)* $($marker,)*> SystemsChain<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystem<$marker> + 'static ),*
        {
            fn chain(self) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().chain()
            }
        }
    };
}

impl_traits_for_function_system_configs!(S1 ; M1);
impl_traits_for_function_system_configs!(S1, S2 ; M1, M2);
impl_traits_for_function_system_configs!(S1, S2, S3 ; M1, M2, M3);
impl_traits_for_function_system_configs!(S1, S2, S3, S4; M1, M2, M3, M4);
impl_traits_for_function_system_configs!(S1, S2, S3, S4, S5; M1, M2, M3, M4, M5);
impl_traits_for_function_system_configs!(S1, S2, S3, S4, S5, S6; M1, M2, M3, M4, M5, M6);
impl_traits_for_function_system_configs!(S1, S2, S3, S4, S5, S6, S7; M1, M2, M3, M4, M5, M6, M7);
impl_traits_for_function_system_configs!(S1, S2, S3, S4, S5, S6, S7, S8; M1, M2, M3, M4, M5, M6, M7, M8);
impl_traits_for_function_system_configs!(S1, S2, S3, S4, S5, S6, S7, S8, S9; M1, M2, M3, M4, M5, M6, M7, M8, M9);
impl_traits_for_function_system_configs!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10; M1, M2, M3, M4, M5, M6, M7, M8, M9, M10);

macro_rules! impl_traits_for_system_configs {
    ($($marker:ident),*) => {
        impl<$($marker,)*> SystemConfigsCondition<($($marker,)*)> for SystemConfigs<($($marker,)*)>
        {
            fn run_if<P>(mut self, condition: impl Condition<P> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.systems.iter_mut().for_each(|sys| {
                    sys.get_or_init_mut::<RunConditionsList>(RunConditionsList::default).add_condition(condition.clone());
                });
                self
            }

            fn in_set(mut self, set: impl SystemSet + Clone) -> SystemConfigs<($($marker,)*)> {
                self.systems.iter_mut().for_each(|sys| {
                    sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default).member_of.push(Box::new(set.clone()));
                });
                self
            }
        }

        impl<$($marker,)*> SystemConfigsOrder<($($marker,)*)> for SystemConfigs<($($marker,)*)>
        {
            fn before<TM>(mut self, target: impl AsSchedulingTarget<TM> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.systems.iter_mut().for_each(|sys| {
                    target.populate_before(sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                });
                self
            }

            fn after<TM>(mut self, target: impl AsSchedulingTarget<TM> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.systems.iter_mut().for_each(|sys| {
                    target.populate_after(sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                });
                self
            }
        }

        impl<$($marker,)*> SystemsChain<($($marker,)*)> for SystemConfigs<($($marker,)*)>
        {
            fn chain(mut self) -> SystemConfigs<($($marker,)*)> {
                #[allow(non_snake_case)]
                let n = self.systems.len();
                for idx in 1..n {
                    let prev_type_id = self.systems[idx - 1].func_type_id();
                    let sys_mut = &mut self.systems[idx];

                    sys_mut
                        .get_or_init_mut::<SystemOrderings>(SystemOrderings::default)
                        .run_after_systems
                        .push(prev_type_id);
                }
                self
            }
        }
    };
}

impl_traits_for_system_configs!(M1);
impl_traits_for_system_configs!(M1, M2);
impl_traits_for_system_configs!(M1, M2, M3);
impl_traits_for_system_configs!(M1, M2, M3, M4);
impl_traits_for_system_configs!(M1, M2, M3, M4, M5);
impl_traits_for_system_configs!(M1, M2, M3, M4, M5, M6);
impl_traits_for_system_configs!(M1, M2, M3, M4, M5, M6, M7);
impl_traits_for_system_configs!(M1, M2, M3, M4, M5, M6, M7, M8);
impl_traits_for_system_configs!(M1, M2, M3, M4, M5, M6, M7, M8, M9);
impl_traits_for_system_configs!(M1, M2, M3, M4, M5, M6, M7, M8, M9, M10);
