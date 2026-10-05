use crate::system::FunctionSystem;
use crate::system::IntoSystem;
use crate::system::System;
use crate::system::SystemMeta;
use crate::system::SystemParam;
use crate::system::system_storage::SystemExt;
use crate::world::storage::World;
use std::any::TypeId;
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
