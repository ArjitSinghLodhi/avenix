use crate::system::FunctionSystem;
use crate::system::IntoSystem;
use crate::system::System;
use crate::system::SystemMeta;
use crate::system::SystemParam;
use crate::world::storage::World;
macro_rules! impl_system_for_functions {
    ($($param:ident),*) => {
        impl<$($param,)* F> System for FunctionSystem<($($param,)*), F>
        where
            $( $param: SystemParam + 'static, )*
            F: Fn($($param),*) + 'static,
        {
            fn run(&mut self, world: &mut World) {
                $(
                    #[allow(non_snake_case)]
                    let $param = <$param>::get_param(world);
                )*
                #[allow(non_snake_case)]
                (self.func)($($param),*);
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

                FunctionSystem::new(self)
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
    F: Fn()
{
    fn run(&mut self, _world: &mut World) {
        (self.func)();
    }
}

impl<F> IntoSystem<()> for F
where 
    F: Fn() + 'static
{
    type SystemType = FunctionSystem<(), F>;
    fn into_system(self) -> Self::SystemType {
        FunctionSystem::new(self)
    }
}