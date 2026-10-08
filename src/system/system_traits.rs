use std::marker::PhantomData;

use crate::app::system::SystemMeta;
use crate::app::system::SystemParam;
use crate::ecs::world::World;
use crate::extensions::FunctionSystem;
use crate::system::IntoSystem;
use crate::system::System;
use crate::system::condition::{Condition, RunConditionsList};
use crate::system::system_set::SystemSet;
use crate::system::system_storage::SystemExt;
use std::any::TypeId;

#[doc(hidden)]
pub struct SystemConfigs<Marker> {
    pub(crate) systems: Vec<Box<dyn System>>,
    pub(crate) _marker: PhantomData<Marker>,
}

#[doc(hidden)]
pub trait IntoSystemConfigs<MarkerGroup> {
    #[doc(hidden)]
    fn into_configs(self) -> SystemConfigs<MarkerGroup>;
}

impl IntoSystemConfigs<()> for SystemConfigs<()> {
    fn into_configs(self) -> SystemConfigs<()> {
        self
    }
}

#[derive(Default, Clone)]
#[doc(hidden)]
pub struct SystemOrderings {
    pub(crate) run_after_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_before_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) member_of: Vec<Box<dyn SystemSet>>,
    pub(crate) run_after_systems: Vec<std::any::TypeId>,
    pub(crate) run_before_systems: Vec<std::any::TypeId>,
}

#[doc(hidden)]
pub trait AsSchedulingTarget<T> {
    #[doc(hidden)]
    fn populate_before(&self, orderings: &mut SystemOrderings);
    #[doc(hidden)]
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
pub trait ConfigureSystem<Marker> {
    #[doc(hidden)]
    type SystemType: System + 'static;

    fn before<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType;
    fn after<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType;
    fn run_if<P>(self, condition: impl Condition<P>) -> Self::SystemType;
    fn in_set(self, set: impl SystemSet + Clone) -> Self::SystemType;
}

#[doc(hidden)]
pub struct FunctionTargetMarker<P, F>(std::marker::PhantomData<(P, F)>);

#[doc(hidden)]
pub trait ConfigureSystemConfigs<MarkerGroup> {
    fn run_if<P>(self, condition: impl Condition<P> + Clone) -> SystemConfigs<MarkerGroup>;
    fn in_set(self, set: impl SystemSet + Clone) -> SystemConfigs<MarkerGroup>;
    fn before<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemConfigs<MarkerGroup>;
    fn after<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemConfigs<MarkerGroup>;
    fn chain(self) -> SystemConfigs<MarkerGroup>;
}

#[doc(hidden)]
pub trait TrackHierarchyEdges<MarkerGroup> {
    #[doc(hidden)]
    fn collect_edge_ids(&self, ids: &mut Vec<TypeId>);
}

impl<Marker> TrackHierarchyEdges<Marker> for SystemConfigs<Marker> {
    fn collect_edge_ids(&self, ids: &mut Vec<TypeId>) {
        for sys in &self.systems {
            ids.push(sys.func_type_id());
        }
    }
}

macro_rules! impl_system_for_functions {
    ($($param:ident),*) => {
        impl<$($param,)* F> System for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            fn run(&mut self, world: &World) {
                $(
                    #[allow(non_snake_case)]
                    let $param = <$param>::get_param(world);
                )*
                #[allow(non_snake_case)]
                (self.func)($($param),*);
            }

            fn func_type_id(&self) -> TypeId {
                TypeId::of::<F>()
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<F>()
            }
        }

        impl<$($param,)* F> IntoSystem<($($param,)*)> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;
            fn into_system(self) -> Self::SystemType {
                let mut system_meta = SystemMeta::new(std::any::type_name::<F>().to_string());
                $(
                    #[allow(non_snake_case)]
                    <$param>::init_access(&mut system_meta);
                )*

                let mut system = FunctionSystem::new(self);
                system.insert(system_meta);
                system
            }
        }

        impl<$($param,)* F> IntoSystem<($($param,)*)> for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;
            fn into_system(self) -> Self::SystemType {
                self
            }
        }

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

        impl<$($param,)* F> ConfigureSystem<($($param,)*)> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;

            fn before<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
                self.into_system().before(target)
            }

            fn after<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
                self.into_system().after(target)
            }

            fn run_if<P>(self, condition: impl Condition<P>) -> Self::SystemType {
                self.into_system().run_if(condition)
            }

            fn in_set(self, set: impl SystemSet + Clone) -> Self::SystemType {
                self.into_system().in_set(set)
            }
        }

        impl<$($param,)* F> ConfigureSystem<($($param,)*)> for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            type SystemType = FunctionSystem<($($param,)*), F>;

            fn run_if<P>(self, condition: impl Condition<P>) -> Self::SystemType {
                let mut system = self.into_system();
                system.get_or_init_mut::<RunConditionsList>(RunConditionsList::default).add_condition(condition);
                system
            }

            fn in_set(self, set: impl SystemSet + Clone) -> Self::SystemType {
                let mut system = self.into_system();
                system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default).member_of.push(Box::new(set));
                system
            }

            fn before<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
                let mut system = self.into_system();
                target.populate_before(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                system
            }

            fn after<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
                let mut system = self.into_system();
                target.populate_after(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                system
            }
        }

        impl<$($param,)* F> TrackHierarchyEdges<($($param,)*)> for F
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            fn collect_edge_ids(&self, ids: &mut Vec<TypeId>) {
                ids.push(TypeId::of::<Self>());
            }
        }

        impl<$($param,)* F> TrackHierarchyEdges<($($param,)*)> for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: 'static,
        {
            fn collect_edge_ids(&self, ids: &mut Vec<TypeId>) {
                ids.push(TypeId::of::<F>());
            }
        }
    };
}

impl_system_for_functions!(A);
impl_system_for_functions!(A, B);
impl_system_for_functions!(A, B, C);
impl_system_for_functions!(A, B, C, D);
impl_system_for_functions!(A, B, C, D, E);
impl_system_for_functions!(A, B, C, D, E, G);
impl_system_for_functions!(A, B, C, D, E, G, H);
impl_system_for_functions!(A, B, C, D, E, G, H, I);
impl_system_for_functions!(A, B, C, D, E, G, H, I, J);
impl_system_for_functions!(A, B, C, D, E, G, H, I, J, K);
impl_system_for_functions!(A, B, C, D, E, G, H, I, J, K, L);
impl_system_for_functions!(A, B, C, D, E, G, H, I, J, K, L, M);

impl<F> System for FunctionSystem<(), F>
where
    F: Fn() + 'static,
{
    fn run(&mut self, _world: &World) {
        (self.func)();
    }

    fn func_type_id(&self) -> TypeId {
        TypeId::of::<F>()
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }
}

impl<F> IntoSystem<()> for F
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;
    fn into_system(self) -> Self::SystemType {
        FunctionSystem::new(self)
    }
}

impl<F> IntoSystem<()> for FunctionSystem<(), F>
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;
    fn into_system(self) -> Self::SystemType {
        self
    }
}

impl<F> ConfigureSystem<()> for F
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;

    fn before<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
        self.into_system().before(target)
    }

    fn after<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
        self.into_system().after(target)
    }

    fn run_if<P>(self, condition: impl Condition<P>) -> Self::SystemType {
        self.into_system().run_if(condition)
    }

    fn in_set(self, set: impl SystemSet + Clone) -> Self::SystemType {
        self.into_system().in_set(set)
    }
}

impl<F> ConfigureSystem<()> for FunctionSystem<(), F>
where
    F: Fn() + 'static,
{
    type SystemType = FunctionSystem<(), F>;

    fn run_if<P>(self, condition: impl Condition<P>) -> Self::SystemType {
        let mut system = self.into_system();
        system
            .get_or_init_mut::<RunConditionsList>(RunConditionsList::default)
            .add_condition(condition);
        system
    }

    fn in_set(self, set: impl SystemSet + Clone) -> Self::SystemType {
        let mut system = self.into_system();
        system
            .get_or_init_mut::<SystemOrderings>(SystemOrderings::default)
            .member_of
            .push(Box::new(set));
        system
    }

    fn before<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
        let mut system = self.into_system();
        target.populate_before(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
        system
    }

    fn after<T>(self, target: impl AsSchedulingTarget<T>) -> Self::SystemType {
        let mut system = self.into_system();
        target.populate_after(system.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
        system
    }
}

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

impl<F> TrackHierarchyEdges<()> for F
where
    F: Fn() + 'static,
{
    fn collect_edge_ids(&self, ids: &mut Vec<TypeId>) {
        ids.push(TypeId::of::<Self>());
    }
}

impl<F> TrackHierarchyEdges<()> for FunctionSystem<(), F>
where
    F: 'static,
{
    fn collect_edge_ids(&self, ids: &mut Vec<TypeId>) {
        ids.push(TypeId::of::<F>());
    }
}

macro_rules! impl_system_configs_for_tuples {
    ($($sys:ident),* ; $($marker:ident),*) => {
        impl<$($sys,)* $($marker,)*> IntoSystemConfigs<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystemConfigs<$marker> + 'static ),*
        {
            fn into_configs(self) -> SystemConfigs<($($marker,)*)> {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                let mut out = Vec::new();
                $( out.extend($sys.into_configs().systems); )*
                SystemConfigs {
                    systems: out,
                    _marker: PhantomData,
                }
            }
        }

        impl<$($sys,)* $($marker,)*> TrackHierarchyEdges<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: TrackHierarchyEdges<$marker> ),*
        {
            fn collect_edge_ids(&self, ids: &mut Vec<TypeId>) {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                $( $sys.collect_edge_ids(ids); )*
            }
        }

        impl<$($sys,)* $($marker,)*> ConfigureSystemConfigs<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: IntoSystemConfigs<$marker> + TrackHierarchyEdges<$marker> + 'static ),*
        {
            fn run_if<P>(self, condition: impl Condition<P> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().run_if(condition)
            }

            fn in_set(self, set: impl SystemSet + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().in_set(set)
            }

            fn before<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().before(target)
            }

            fn after<T>(self, target: impl AsSchedulingTarget<T> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.into_configs().after(target)
            }

            #[allow(unused_assignments)]
            fn chain(self) -> SystemConfigs<($($marker,)*)> {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                let mut flattened_configs = Vec::new();

                let mut previous_edges: Vec<TypeId> = Vec::new();

                $({
                    let mut current_edges = Vec::new();
                    $sys.collect_edge_ids(&mut current_edges);

                    let mut current_config = $sys.into_configs();

                    if !previous_edges.is_empty() {
                        for sys_item in current_config.systems.iter_mut() {
                            let orderings = sys_item.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
                            for prev_id in previous_edges.iter() {
                                if !orderings.run_after_systems.contains(prev_id) {
                                    orderings.run_after_systems.push(*prev_id);
                                }
                            }
                        }
                    }

                    flattened_configs.extend(current_config.systems);
                    previous_edges = current_edges;
                })*

                SystemConfigs {
                    systems: flattened_configs,
                    _marker: PhantomData,
                }
            }
        }

        impl<$($marker,)*> IntoSystemConfigs<($($marker,)*)> for SystemConfigs<($($marker,)*)>
        {
            fn into_configs(self) -> SystemConfigs<($($marker,)*)> {
                self
            }
        }

        impl<$($marker,)*> ConfigureSystemConfigs<($($marker,)*)> for SystemConfigs<($($marker,)*)>
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

            fn before<T>(mut self, target: impl AsSchedulingTarget<T> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.systems.iter_mut().for_each(|sys| {
                    target.populate_before(sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                });
                self
            }

            fn after<T>(mut self, target: impl AsSchedulingTarget<T> + Clone) -> SystemConfigs<($($marker,)*)> {
                self.systems.iter_mut().for_each(|sys| {
                    target.populate_after(sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default));
                });
                self
            }

            fn chain(mut self) -> SystemConfigs<($($marker,)*)> {
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

        impl<$($sys,)* $($marker,)*> AsSchedulingTarget<($($marker,)*)> for ($($sys,)*)
        where
            $( $sys: AsSchedulingTarget<$marker> + 'static ),*
        {
            fn populate_before(&self, orderings: &mut SystemOrderings) {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                $( $sys.populate_before(orderings); )*
            }

            fn populate_after(&self, orderings: &mut SystemOrderings) {
                #[allow(non_snake_case)]
                let ($($sys,)*) = self;
                $( $sys.populate_after(orderings); )*
            }
        }
    };
}

impl_system_configs_for_tuples!(S1 ; M1);
impl_system_configs_for_tuples!(S1, S2 ; M1, M2);
impl_system_configs_for_tuples!(S1, S2, S3 ; M1, M2, M3);
impl_system_configs_for_tuples!(S1, S2, S3, S4; M1, M2, M3, M4);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5; M1, M2, M3, M4, M5);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5, S6; M1, M2, M3, M4, M5, M6);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5, S6, S7; M1, M2, M3, M4, M5, M6, M7);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5, S6, S7, S8; M1, M2, M3, M4, M5, M6, M7, M8);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5, S6, S7, S8, S9; M1, M2, M3, M4, M5, M6, M7, M8, M9);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10; M1, M2, M3, M4, M5, M6, M7, M8, M9, M10);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11; M1, M2, M3, M4, M5, M6, M7, M8, M9, M10, M11);
impl_system_configs_for_tuples!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12; M1, M2, M3, M4, M5, M6, M7, M8, M9, M10, M11, M12);

impl<S, Marker> IntoSystemConfigs<Marker> for S
where
    S: IntoSystem<Marker> + 'static,
{
    fn into_configs(self) -> SystemConfigs<Marker> {
        SystemConfigs {
            systems: vec![Box::new(self.into_system())],
            _marker: PhantomData,
        }
    }
}
