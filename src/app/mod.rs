mod plugin;
use parking_lot::RwLock;
pub use plugin::{Plugin, PluginsBuildAll};
use rustc_hash::FxHashSet;

use crate::{
    app::system::SystemExt,
    resources::Resource,
    schedule::{
        DefaultSchedulesPlugin, Schedule, ScheduleConstraint, ScheduleLabel, Startup,
        schedule_sorter::sort_schedules,
        system_sorter::{dispatch_system_blocks, sort_schedule_systems},
    },
    states::{States, setup_states_schedules_and_systems},
    system::{
        System, SystemMeta,
        system_set_traits::IntoSystemSetConfigs,
        system_traits::{IntoSystemConfigs, SystemOrderings},
    },
    world::World,
};
use crate::{schedule::SystemNode, system::system_set::SetRegistration};
use std::{
    any::TypeId,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::events::{Event, EventBuffer, register_event};
use std::any::type_name;

use crate::{ecs::component::Component, reactivity::removed::register_removal_tracking_buffers};

static APP_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub(crate) static APP_BUILT: RwLock<bool> = RwLock::new(false);

pub(crate) struct ConfigurationContext {
    building_plugins: bool,
    schedules_processed: bool,
    systems_processed: bool,
    built: bool,
    ran_startup: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AvenixAppConfigurationError {
    SchedulesNotInitialized,
    SystemsNotConfigured,
    CriticalEngineBuildMismatch,
    StartupNotProcessed,
    AppAlreadyBuilt,
}

impl ConfigurationContext {
    fn new() -> Self {
        Self {
            building_plugins: false,
            schedules_processed: false,
            systems_processed: false,
            built: false,
            ran_startup: false,
        }
    }

    fn is_building_plugins(&self) -> bool {
        self.building_plugins
    }

    fn schedules_processed(&self) {
        if !self.schedules_processed {
            panic!("{:?}", AvenixAppConfigurationError::SchedulesNotInitialized);
        }
    }

    fn systems_processed(&self) {
        if !self.systems_processed {
            panic!("{:?}", AvenixAppConfigurationError::SystemsNotConfigured);
        }
    }

    fn built(&self) {
        self.schedules_processed();
        self.systems_processed();
        if !self.built {
            panic!(
                "{:?}",
                AvenixAppConfigurationError::CriticalEngineBuildMismatch
            );
        }
    }

    fn ran_startup(&self) {
        if !self.ran_startup {
            panic!("{:?}", AvenixAppConfigurationError::StartupNotProcessed);
        }
    }

    fn not_ready(&self) {
        if self.built || self.ran_startup || *APP_BUILT.read() {
            panic!("{:?}", AvenixAppConfigurationError::AppAlreadyBuilt);
        }
    }
}

pub(crate) struct SystemsBlock {
    pub(crate) schedule: Box<dyn ScheduleLabel>,
    pub(crate) systems: Vec<Box<dyn System>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

type RunnerFn = Box<dyn FnOnce(&mut App) + 'static>;

pub struct App {
    world: World,
    registered_plugins: FxHashSet<TypeId>,
    startup_schedule: Schedule,
    schedules: Vec<Schedule>,
    systems_blocks: Vec<SystemsBlock>,
    pub(crate) schedule_order_constraints: Vec<ScheduleConstraint>,
    pub(crate) set_registrations: Vec<SetRegistration>,
    runner_fn: RunnerFn,
    configuration_context: ConfigurationContext,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self::empty();
        DefaultSchedulesPlugin::build_and_register_all(&DefaultSchedulesPlugin, &mut app);
        app
    }

    /// Initializes App with no default schedules except [`Startup`].
    pub fn empty() -> Self {
        if APP_INITIALIZED.swap(true, Ordering::SeqCst) {
            panic!(
                "AvenixMultipleAppInstancesError\n\
                ❌ AVENIX ARCHITECTURE VIOLATION: Multiple App instances detected!\nEnsure you only instantiate exactly one App::new() across your entire binary runtime."
            );
        }

        Self {
            world: World::new(),
            registered_plugins: FxHashSet::default(),
            startup_schedule: Schedule::new(Startup),
            schedules: Vec::new(),
            systems_blocks: Vec::new(),
            schedule_order_constraints: Vec::new(),
            set_registrations: Vec::new(),
            runner_fn: Box::new(runner_once),
            configuration_context: ConfigurationContext::new(),
        }
    }

    pub fn has_schedule(&self, label: &impl ScheduleLabel) -> bool {
        self.schedules
            .iter()
            .any(|schedule| schedule.label().dyn_eq(label))
    }

    pub fn init_state<T: States + Default>(&mut self) -> &mut Self {
        self.configuration().not_ready();
        self.insert_state(T::default())
    }

    pub fn insert_state<T: States>(&mut self, state: T) -> &mut Self {
        self.configuration().not_ready();
        setup_states_schedules_and_systems(self, state);
        self
    }

    pub fn add_schedule(&mut self, schedule: Schedule) -> &mut Self {
        self.configuration().not_ready();

        if schedule.label().dyn_eq(&Startup) {
            panic!("Startup schedule cannot be overwritten")
        } else {
            self.schedules.push(schedule);
        }
        self
    }

    pub fn configure_sets(
        &mut self,
        schedule: impl crate::schedule::ScheduleLabel,
        set_configs: impl IntoSystemSetConfigs,
    ) -> &mut Self {
        self.configuration().not_ready();

        let schedule_box: Box<dyn crate::schedule::ScheduleLabel> = Box::new(schedule);

        for config in set_configs.into_configs() {
            let registration = SetRegistration {
                schedule: schedule_box.dyn_clone(),
                set: config.set,
                run_after_sets: config.run_after_sets,
                run_before_sets: config.run_before_sets,
                run_after_systems: config.run_after_systems,
                run_before_systems: config.run_before_systems,
                member_of: config.member_of,
                condition_gates: config.condition_gates,
            };
            self.set_registrations.push(registration);
        }

        self
    }

    pub fn configure_schedule_order(
        &mut self,
        before: impl ScheduleLabel,
        after: impl ScheduleLabel,
    ) -> &mut Self {
        self.configuration().not_ready();
        self.schedule_order_constraints.push(ScheduleConstraint {
            before: Box::new(before),
            after: Box::new(after),
        });
        self
    }

    pub fn add_systems<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.configuration().not_ready();
        let configs = systems.into_configs();

        if let Some(existing_block) = self
            .systems_blocks
            .iter_mut()
            .find(|b| (*b.schedule).dyn_eq(&schedule))
        {
            existing_block.systems.extend(configs.systems);
        } else {
            let block = SystemsBlock {
                schedule: Box::new(schedule),
                systems: configs.systems,
            };
            self.systems_blocks.push(block);
        }
        self
    }

    /// Adds plugins and immediaely builds them.
    pub fn add_plugins(&mut self, plugins: impl PluginsBuildAll + 'static) -> &mut Self {
        self.configuration().not_ready();
        plugins.build_and_register_all(self);
        self
    }

    pub fn is_plugin_built<T: Plugin>(&self) -> bool {
        self.registered_plugins.contains(&TypeId::of::<T>())
    }

    pub fn init_event<T: Event>(&mut self) -> &mut Self {
        self.configuration().not_ready();
        if self.world.has_resource::<EventBuffer<T>>() {
            panic!("Event: {} Already initialized", type_name::<T>())
        }
        self.world.insert_resource(EventBuffer::<T>::new());
        register_event::<T>();
        self
    }

    pub fn build(&mut self) -> &mut Self {
        if self.configuration().is_building_plugins() {
            panic!("App::build() was called while building plugins")
        }
        self.configuration().not_ready();
        self.build_everything();
        *APP_BUILT.write() = true;
        self.cleanup_build();
        self.configuration().built = true;
        self
    }

    pub fn run_startup(&mut self) -> &mut Self {
        if self.configuration().is_building_plugins() {
            panic!("App::run_startup() was called while building plugins")
        }
        self.configuration().built();
        if self.configuration().ran_startup {
            panic!("Startup Already ran");
        }
        self.startup_schedule.run(&mut self.world);
        self.configuration().ran_startup = true;
        self
    }

    pub fn update(&mut self) {
        if self.configuration().is_building_plugins() {
            panic!("App::update() was called while building plugins")
        }
        self.configuration().built();
        self.configuration().ran_startup();
        self.world_mut().start_frame_sync();
        for schedule in self.schedules.iter_mut() {
            schedule.run(&mut self.world);
        }
        self.world.apply_queue_commands();
        self.world.apply_despawns();
        self.world_mut().end_of_frame_sync();
    }

    pub fn run(&mut self) {
        if self.configuration().is_building_plugins() {
            panic!("App::run() was called while building plugins")
        }
        let function = std::mem::replace(&mut self.runner_fn, Box::new(runner_once));
        function(self);
    }

    pub fn set_runner(&mut self, function: impl FnOnce(&mut App) + 'static) -> &mut Self {
        self.runner_fn = Box::new(function);
        self
    }

    pub fn init_resource<T: Resource + Send + Sync + Default>(&mut self) -> &mut Self {
        self.insert_resource(T::default())
    }

    pub fn insert_resource<T: Resource + Send + Sync>(&mut self, resource: T) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn remove_resource<T: Resource + Send + Sync>(&mut self) -> &mut Self {
        self.world.remove_resource::<T>();
        self
    }

    pub fn init_non_send_resource<T: Resource + Default>(&mut self) -> &mut Self {
        self.insert_non_send_resource(T::default())
    }

    pub fn insert_non_send_resource<T: Resource>(&mut self, resource: T) -> &mut Self {
        self.world.insert_non_send_resource(resource);
        self
    }

    pub fn remove_non_send_resource<T: Resource>(&mut self) -> &mut Self {
        self.world.remove_non_send_resource::<T>();
        self
    }

    pub fn register_changed_trackers<T: Component>(&mut self) -> &mut Self {
        self.world().register_changed_trackers::<T>();
        self
    }

    pub fn register_added_trackers<T: Component>(&mut self) -> &mut Self {
        self.world().register_added_trackers::<T>();
        self
    }

    pub fn register_removal_trackers<T: Component>(&mut self) -> &mut Self {
        self.world().register_removal_trackers::<T>();
        self
    }

    fn configuration(&mut self) -> &mut ConfigurationContext {
        &mut self.configuration_context
    }
}

impl App {
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl App {
    fn build_everything(&mut self) {
        self.configure_schedules();
        self.configure_systems();
        self.schedules.iter_mut().for_each(|schedule| {
            schedule.init_executor();
        });
        self.startup_schedule.init_executor();
        self.configuration().schedules_processed = true;
        register_removal_tracking_buffers(self.world_mut());
    }

    fn configure_systems(&mut self) {
        let global_registrations = std::mem::take(&mut self.set_registrations);

        dispatch_system_blocks(
            &mut self.systems_blocks,
            &mut self.startup_schedule,
            &mut self.schedules,
        );

        let mut remaining_registrations = global_registrations;
        {
            let label = self.startup_schedule.label();
            let mut schedule_registrations = Vec::new();
            let mut next_remaining = Vec::new();
            for reg in remaining_registrations.drain(..) {
                if reg.schedule.dyn_eq(label) {
                    schedule_registrations.push(reg);
                } else {
                    next_remaining.push(reg);
                }
            }
            remaining_registrations = next_remaining;

            let systems = self.startup_schedule.systems_schedule_mut().systems_mut();
            let mut borrowed_systems: Vec<&mut SystemNode> = systems.iter_mut().collect();
            let mut generated_guards = Vec::new();
            crate::system::system_set::preprocess_system_sets(
                &mut borrowed_systems,
                &mut schedule_registrations,
                &mut generated_guards,
            );
            systems.extend(generated_guards);
        }

        for schedule in &mut self.schedules {
            let label = schedule.label();
            let mut schedule_registrations = Vec::new();
            let mut next_remaining = Vec::new();
            for reg in remaining_registrations.drain(..) {
                if reg.schedule.dyn_eq(label) {
                    schedule_registrations.push(reg);
                } else {
                    next_remaining.push(reg);
                }
            }
            remaining_registrations = next_remaining;

            let systems = schedule.systems_schedule_mut().systems_mut();
            let mut borrowed_systems: Vec<&mut SystemNode> = systems.iter_mut().collect();
            let mut generated_guards = Vec::new();
            crate::system::system_set::preprocess_system_sets(
                &mut borrowed_systems,
                &mut schedule_registrations,
                &mut generated_guards,
            );
            systems.extend(generated_guards);
        }

        sort_schedule_systems(self.startup_schedule.systems_schedule_mut().systems_mut());

        for schedule in &mut self.schedules {
            let systems = schedule.systems_schedule_mut().systems_mut();
            sort_schedule_systems(systems);
        }

        self.configuration().systems_processed = true;
    }

    fn configure_schedules(&mut self) {
        self.schedules = sort_schedules(
            std::mem::take(&mut self.schedules),
            &self.schedule_order_constraints,
        );
    }

    fn cleanup_build(&mut self) {
        self.set_registrations.clear();
        self.registered_plugins.clear();
        self.systems_blocks.clear();
        self.systems_blocks.shrink_to_fit();
        self.schedule_order_constraints.clear();
        self.startup_schedule
            .systems_schedule_mut()
            .systems_mut()
            .iter_mut()
            .for_each(|node| {
                let system = node.system_mut();
                system.remove::<SystemMeta>();
                system.remove::<SystemOrderings>();
            });
        self.schedules.iter_mut().for_each(|schedule| {
            schedule
                .systems_schedule_mut()
                .systems_mut()
                .iter_mut()
                .for_each(|node| {
                    let system = node.system_mut();
                    system.remove::<SystemMeta>();
                    system.remove::<SystemOrderings>();
                });
        });
    }
}

fn runner_once(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
}
