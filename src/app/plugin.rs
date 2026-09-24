use crate::app::App;

pub trait Plugin: 'static {
    fn build(self, app: &mut App);
}

pub trait PluginsBuildAll {
    fn build_all(self, app: &mut App);
    fn get_plugin_names(&self) -> Vec<&'static str>;
}

impl<T: Plugin> PluginsBuildAll for T {
    fn build_all(self, app: &mut App) {
        self.build(app);
    }

    fn get_plugin_names(&self) -> Vec<&'static str> {
        vec![std::any::type_name::<T>()]
    }
}

macro_rules! plugin_tuple {
    ($($T:ident -> $idx:tt),*) => {
        impl<$($T: Plugin),*> PluginsBuildAll for ($($T,)*) {
            fn build_all(self, app: &mut App) {
                $(
                    $T::build(self.$idx, app);
                )*
            }

            fn get_plugin_names(&self) -> Vec<&'static str> {
                vec![$(std::any::type_name::<$T>()),*]
            }
        }
    };
}

plugin_tuple!(A -> 0);
plugin_tuple!(A -> 0, B -> 1);
plugin_tuple!(A -> 0, B -> 1, C -> 2);
plugin_tuple!(A -> 0, B -> 1, C -> 2, D -> 3);
plugin_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4);
plugin_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5);
plugin_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6);
