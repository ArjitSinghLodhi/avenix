mod plugin;
use parking_lot::RwLock;
pub use plugin::{Plugin, PluginsBuildAll};

use std::{
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    app::system::condition::RunConditionsList,
    extensions::SystemExt,
    query::{QueryData, QueryFilter, parallel_query::ParallelQueryAccessor},
    resources::Resource,
    schedule::{
        CleanupHandles, DefaultSchedulesPlugin, Schedule, ScheduleConstraint, ScheduleLabel,
        Startup,
        schedule_sorter::sort_schedules,
        system_sorter::{dispatch_system_blocks, sort_schedule_systems},
    },
    states::{States, setup_states_schedules_and_systems},
    system::{AccessVec, IntoSystemConfigs, System},
    world::storage::World,
};

#[cfg(feature = "events")]
use crate::events::{Event, EventBuffer, register_event};
#[cfg(feature = "events")]
use std::any::type_name;

#[cfg(feature = "reactivity")]
use crate::reactivity::register_removal_tracking_buffers;

static APP_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub(crate) static APP_BUILT: RwLock<bool> = RwLock::new(false);

struct ConfigurationContext {
    building_plugins: bool,
    schedules_processed: bool,
    systems_processed: bool,
    built: bool,
    ran_startup: bool,
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
            panic!("Schedules not added and processed when expected");
        }
    }

    fn systems_processed(&self) {
        if !self.systems_processed {
            panic!("Systems not Added and Configured when expected");
        }
    }

    fn built(&self) {
        self.schedules_processed();
        self.systems_processed();
        if !self.built {
            panic!("Configuration Somehow Not Built even After All Check: Engine problem Likely!");
        }
    }

    fn ran_startup(&self) {
        if !self.ran_startup {
            panic!("Startup not processed already when expected");
        }
    }

    fn not_ready(&self) {
        if self.built || self.ran_startup {
            panic!("App already built when expected not");
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
    pub(crate) world: World,
    startup_schedule: Schedule,
    cleanup_schedule: Schedule,
    schedules: Vec<Schedule>,
    systems_blocks: Vec<SystemsBlock>,
    pub(crate) schedule_order_constraints: Vec<ScheduleConstraint>,
    runner_fn: RunnerFn,
    configuration: ConfigurationContext,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self::empty();
        DefaultSchedulesPlugin::build(&DefaultSchedulesPlugin, &mut app);
        app
    }

    /// Initializes App with no default schedules except [`Startup`] and [`CleanupHandles`] schedules.
    ///
    pub fn empty() -> Self {
        if APP_INITIALIZED.swap(true, Ordering::Relaxed) {
            panic!(
                "❌ AVENIX ARCHITECTURE VIOLATION: Multiple App instances detected!\nEnsure you only instantiate exactly one App::new() across your entire binary runtime."
            );
        }

        Self {
            world: World::new(),
            startup_schedule: Schedule::new(Startup),
            cleanup_schedule: Schedule::new(CleanupHandles),
            schedules: Vec::new(),
            systems_blocks: Vec::new(),
            schedule_order_constraints: Vec::new(),
            runner_fn: Box::new(runner_once),
            configuration: ConfigurationContext::new(),
        }
    }

    pub fn init_state<T: States + Default>(&mut self) -> &mut Self {
        self.insert_state(T::default())
    }

    pub fn insert_state<T: States>(&mut self, state: T) -> &mut Self {
        setup_states_schedules_and_systems(self, state);
        self
    }

    pub fn add_schedule(&mut self, schedule: Schedule) -> &mut Self {
        self.configuration.not_ready();

        if schedule.schedule().dyn_eq(&Startup) {
            panic!("Startup schedule cannot be overwritten")
        } else if schedule.schedule().dyn_eq(&CleanupHandles) {
            panic!("CleanupHandles schedule cannnot be overwritten")
        } else {
            self.schedules.push(schedule);
        }
        self
    }

    pub fn configure_schedule_order(
        &mut self,
        before: impl ScheduleLabel,
        after: impl ScheduleLabel,
    ) -> &mut Self {
        self.configuration.not_ready();
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
        self.configuration.not_ready();
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

    pub fn add_plugins(&mut self, plugins: impl PluginsBuildAll + 'static) -> &mut Self {
        self.configuration.not_ready();
        plugins.build_all(self);
        self
    }

    #[cfg(feature = "events")]
    pub fn init_event<T: Event>(&mut self) -> &mut Self {
        self.configuration.not_ready();
        if self.world.has_resource::<EventBuffer<T>>() {
            panic!("Event: {} Already initialized", type_name::<T>())
        }
        self.world.insert_resource(EventBuffer::<T>::new());
        register_event::<T>();
        self
    }

    pub fn build(&mut self) -> &mut Self {
        if self.configuration.is_building_plugins() {
            panic!("App::build() was called while building plugins")
        }
        self.configuration.not_ready();
        self.build_everything();
        *APP_BUILT.write() = false;
        self.configuration.built = true;
        self
    }

    pub fn run_startup(&mut self) -> &mut Self {
        if self.configuration.is_building_plugins() {
            panic!("App::run_startup() was called while building plugins")
        }
        self.configuration.built();
        if self.configuration.ran_startup {
            panic!("Startup Already ran");
        }
        self.startup_schedule.run(&mut self.world);
        self.configuration.ran_startup = true;
        self
    }

    pub fn update(&mut self) {
        if self.configuration.is_building_plugins() {
            panic!("App::update() was called while building plugins")
        }
        self.configuration.built();
        self.configuration.ran_startup();
        for schedule in self.schedules.iter_mut() {
            schedule.run(&mut self.world);
        }
        self.cleanup_schedule.run(&mut self.world);
        self.world_mut().end_of_frame_sync();
    }

    pub fn run(&mut self) {
        if self.configuration.is_building_plugins() {
            panic!("App::run() was called while building plugins")
        }
        let function = std::mem::replace(&mut self.runner_fn, Box::new(runner_once));
        function(self);
    }

    pub fn set_runner(&mut self, function: impl FnOnce(&mut App) + 'static) -> &mut Self {
        self.runner_fn = Box::new(function);
        self
    }

    pub fn insert_resource<T: Resource + Send + Sync>(&mut self, resource: T) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn remove_resource<T: Resource + Send + Sync>(&mut self) -> &mut Self {
        self.world.remove_resource::<T>();
        self
    }

    pub fn insert_non_send_resource<T: Resource>(&mut self, resource: T) -> &mut Self {
        self.world.insert_non_send_resource(resource);
        self
    }

    pub fn remove_non_send_resource<T: Resource>(&mut self) -> &mut Self {
        self.world.remove_non_send_resource::<T>();
        self
    }
}

impl App {
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn get_par_query_accessor<Q: QueryData, F: QueryFilter>(
        &mut self,
    ) -> ParallelQueryAccessor<Q, F> {
        self.configuration.not_ready();
        let mut local_reads = AccessVec::new();
        let mut local_writes = AccessVec::new();
        let mut local_with = AccessVec::new();
        let mut local_without = AccessVec::new();

        Q::collect_access(&mut local_reads, &mut local_writes);
        F::collect_filter(&mut local_with, &mut local_without);
        let has_intra_conflict = local_writes.iter().any(|w| local_reads.contains(w));
        let mut unique_writes = rustc_hash::FxHashSet::default();
        let has_duplicate_writes = local_writes.iter().any(|w| !unique_writes.insert(w));

        if has_intra_conflict || has_duplicate_writes {
            panic!(
                "❌ ECS INTRA-QUERY ARGUMENT CONFLICT in ParallelQueryAccessor: ParallelQueryAccessor<{}, {}> contains internal overlaps!",
                std::any::type_name::<Q>(),
                std::any::type_name::<F>()
            );
        }
        ParallelQueryAccessor {
            archetypes_map: self.world.archetypes_manager.archetypes.clone(),
            _marker: PhantomData,
        }
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
        self.cleanup_schedule.init_executor();
        self.configuration.schedules_processed = true;
        #[cfg(feature = "reactivity")]
        register_removal_tracking_buffers(self);
    }

    fn configure_systems(&mut self) {
        dispatch_system_blocks(
            &mut self.systems_blocks,
            &mut self.startup_schedule,
            &mut self.cleanup_schedule,
            &mut self.schedules,
        );

        let mut schedules_to_sort = Vec::new();
        schedules_to_sort.push(self.startup_schedule.systems_schedule_mut().systems_mut());
        schedules_to_sort.push(self.cleanup_schedule.systems_schedule_mut().systems_mut());
        for schedule in &mut self.schedules {
            schedules_to_sort.push(schedule.systems_schedule_mut().systems_mut());
        }

        for schedule_systems in schedules_to_sort.iter_mut() {
            sort_schedule_systems(schedule_systems);
        }

        for schedule_systems in schedules_to_sort.iter_mut() {
            for node in schedule_systems.iter_mut() {
                node.system_mut()
                    .get_or_init_mut(RunConditionsList::default)
                    .build_conditions(&mut self.world);
            }
        }

        self.configuration.systems_processed = true;
    }

    fn configure_schedules(&mut self) {
        self.schedules = sort_schedules(
            std::mem::take(&mut self.schedules),
            &self.schedule_order_constraints,
        );
        self.schedule_order_constraints.clear();
    }
}

fn runner_once(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
}
