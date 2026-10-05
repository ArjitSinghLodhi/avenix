use crate::app::system::SystemParam;
use crate::extensions::FunctionSystem;
use crate::system::System;
use crate::system::condition::{Condition, RunConditionsList};
use crate::system::system_storage::SystemExt;
use crate::system::{IntoSystem, SystemConfigs};
use std::any::TypeId;
use std::marker::PhantomData;

// System Traits.

#[doc(hidden)]
pub trait SystemCondition<Marker> {
    #[doc(hidden)]
    type SystemType: System + 'static;
    fn run_if<P>(self, condition: impl Condition<P>) -> Self::SystemType;
}

#[derive(Default)]
pub(crate) struct SystemOrderings {
    pub(crate) run_after: Vec<TypeId>,
    pub(crate) run_before: Vec<TypeId>,
}

#[doc(hidden)]
pub trait SystemOrder<Marker> {
    #[doc(hidden)]
    type SystemType: System + 'static;
    fn before<Target: 'static>(self, target: Target) -> Self::SystemType;
    fn after<Target: 'static>(self, target: Target) -> Self::SystemType;
}

macro_rules! impl_traits_for_function {
    ($($param:ident),*) => {
        impl<$($param,)* F> SystemCondition<($($param,)*)> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;
            fn run_if<P>(self, condition: impl Condition<P>) -> FunctionSystem<($($param,)*), F> {
                let system = self.into_system();
                system.run_if(condition)
            }
        }
        impl<$($param,)* F> SystemOrder<($($param,)*)> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;

            fn before<Target: 'static>(self, target: Target) -> Self::SystemType {
                self.into_system().before(target)
            }

            fn after<Target: 'static>(self, target: Target) -> Self::SystemType {
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

impl<F> SystemCondition<()> for F
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;
    fn run_if<P>(self, condition: impl Condition<P>) -> FunctionSystem<(), F> {
        let system = self.into_system();
        system.run_if(condition)
    }
}

impl<F> SystemOrder<()> for F
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;

    fn before<Target: 'static>(self, target: Target) -> Self::SystemType {
        self.into_system().before(target)
    }

    fn after<Target: 'static>(self, target: Target) -> Self::SystemType {
        self.into_system().after(target)
    }
}

// System traits for a function system

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
        }

        impl<$($param,)* F> SystemOrder<($($param,)*)> for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;

            fn before<Target: 'static>(self, _target: Target) -> FunctionSystem<($($param,)*), F> {
                let mut system = self.into_system();
                system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default).run_before.push(TypeId::of::<Target>());
                system
            }

            fn after<Target: 'static>(self, _target: Target) -> FunctionSystem<($($param,)*), F> {
                let mut system = self.into_system();
                system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default).run_after.push(TypeId::of::<Target>());
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
}

impl<F> SystemOrder<()> for FunctionSystem<(), F>
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;

    fn before<Target: 'static>(self, _target: Target) -> FunctionSystem<(), F> {
        let mut system = self.into_system();
        system
            .get_or_init_mut::<SystemOrderings>(SystemOrderings::default)
            .run_before
            .push(TypeId::of::<Target>());
        system
    }

    fn after<Target: 'static>(self, _target: Target) -> FunctionSystem<(), F> {
        let mut system = self.into_system();
        system
            .get_or_init_mut::<SystemOrderings>(SystemOrderings::default)
            .run_after
            .push(TypeId::of::<Target>());
        system
    }
}

// System Configs Traits.

#[doc(hidden)]
pub trait SystemConfigsCondition<MarkerGroup> {
    fn run_if<P>(self, condition: impl Condition<P> + Clone) -> SystemConfigs<MarkerGroup>;
}

#[doc(hidden)]
pub trait SystemConfigsOrder<MarkerGroup> {
    fn before<Target: Clone + 'static>(self, target: Target) -> SystemConfigs<MarkerGroup>;
    fn after<Target: Clone + 'static>(self, target: Target) -> SystemConfigs<MarkerGroup>;
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
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                SystemConfigs {
                    systems: vec![
                        $( Box::new($sys.run_if(condition.clone())) ),*
                    ],
                    _marker: PhantomData,
                }
            }
        }

        impl<$($sys,)* $($marker,)*> SystemConfigsOrder<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystem<$marker> + 'static + SystemOrder<$marker> ),*
        {
            fn before<Target: Clone + 'static>(self, target: Target) -> SystemConfigs<($($marker,)*)> {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                SystemConfigs {
                    systems: vec![
                        $( Box::new($sys.before(target.clone())) ),*
                    ],
                    _marker: PhantomData,
                }
            }

            fn after<Target: Clone + 'static>(self, target: Target) -> SystemConfigs<($($marker,)*)> {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                SystemConfigs {
                    systems: vec![
                        $( Box::new($sys.after(target.clone())) ),*
                    ],
                    _marker: PhantomData,
                }
            }
        }

        impl<$($sys,)* $($marker,)*> SystemsChain<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystem<$marker> + 'static ),*
        {
            fn chain(self) -> SystemConfigs<($($marker,)*)> {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;

                let mut boxed_systems: Vec<Box<dyn System>> = vec![
                    $( Box::new($sys.into_system()) ),*
                ];

                let n = boxed_systems.len();
                for idx in 1..n {
                    let prev_type_id = boxed_systems[idx - 1].func_type_id();
                    if let Some(sys_mut) = boxed_systems.get_mut(idx) {
                        sys_mut
                            .get_or_init_mut::<SystemOrderings>(SystemOrderings::default)
                            .run_after
                            .push(prev_type_id);
                    }
                }

                SystemConfigs {
                    systems: boxed_systems,
                    _marker: std::marker::PhantomData,
                }
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

// System traits for a system configs.

macro_rules! impl_traits_for_system_configs {
    ($($sys:ident),* ; $($marker:ident),*) => {
        impl<$($marker,)*> SystemConfigsCondition<($($marker,)*)> for SystemConfigs<($($marker,)*)>

        {
            fn run_if<P>(self, condition: impl Condition<P> + Clone) -> SystemConfigs<($($marker,)*)> {
                let systems = self.systems.into_iter().map(|mut sys| {
                    sys.get_or_init_mut::<RunConditionsList>(RunConditionsList::default).add_condition(condition.clone());
                    sys
                }).collect::<Vec<Box<dyn System>>>();
                SystemConfigs {
                    systems,
                    _marker: PhantomData,
                }
            }
        }
        impl<$($marker,)*> SystemConfigsOrder<($($marker,)*)> for SystemConfigs<($($marker,)*)>
        {
            fn before<Target: 'static>(self, _target: Target) -> SystemConfigs<($($marker,)*)> {
                let systems = self.systems.into_iter().map(|mut sys| {
                    sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default).run_before.push(std::any::TypeId::of::<Target>());
                    sys
                }).collect::<Vec<Box<dyn System>>>();
                SystemConfigs {
                    systems,
                    _marker: PhantomData,
                }
            }

            fn after<Target: 'static>(self, _target: Target) -> SystemConfigs<($($marker,)*)> {
                let systems = self.systems.into_iter().map(|mut sys| {
                    sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default).run_after.push(std::any::TypeId::of::<Target>());
                    sys
                }).collect::<Vec<Box<dyn System>>>();
                SystemConfigs {
                    systems,
                    _marker: PhantomData,
                }
            }
        }
    };
}

impl_traits_for_system_configs!(S1 ; M1);
impl_traits_for_system_configs!(S1, S2 ; M1, M2);
impl_traits_for_system_configs!(S1, S2, S3 ; M1, M2, M3);
impl_traits_for_system_configs!(S1, S2, S3, S4; M1, M2, M3, M4);
impl_traits_for_system_configs!(S1, S2, S3, S4, S5; M1, M2, M3, M4, M5);
impl_traits_for_system_configs!(S1, S2, S3, S4, S5, S6; M1, M2, M3, M4, M5, M6);
impl_traits_for_system_configs!(S1, S2, S3, S4, S5, S6, S7; M1, M2, M3, M4, M5, M6, M7);
impl_traits_for_system_configs!(S1, S2, S3, S4, S5, S6, S7, S8; M1, M2, M3, M4, M5, M6, M7, M8);
impl_traits_for_system_configs!(S1, S2, S3, S4, S5, S6, S7, S8, S9; M1, M2, M3, M4, M5, M6, M7, M8, M9);
impl_traits_for_system_configs!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10; M1, M2, M3, M4, M5, M6, M7, M8, M9, M10);
