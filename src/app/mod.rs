mod plugin;
pub use plugin::{Plugin, PluginsBuildAll};

use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
};

use fxhash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::{
    resources::Resource,
    schedule::{CleanupHandles, IntoScheduleId, Schedule, ScheduleId, ScheduleLabel, Startup},
    system::{IntoSystemConfigs, System},
    world::storage::World,
};

#[cfg(feature = "events")]
use crate::events::{Event, EventBuffer, register_event};
#[cfg(feature = "events")]
use std::any::type_name;

#[cfg(feature = "reactivity")]
use crate::reactivity::register_removal_tracking_buffers;

static APP_INITIALIZED: AtomicBool = AtomicBool::new(false);

struct ConfigurationContext {
    building_plugins: bool,
    plugins_processed: bool,
    schedules_added: bool,
    systems_added: bool,
    built: bool,
    ran_startup: bool,
}
impl ConfigurationContext {
    fn new() -> Self {
        Self {
            building_plugins: false,
            plugins_processed: false,
            schedules_added: false,
            systems_added: false,
            built: false,
            ran_startup: false,
        }
    }
    fn is_building_plugins(&self) -> bool {
        self.building_plugins
    }
    fn plugins_processed(&self) {
        if !self.plugins_processed {
            panic!("plugins Not processed when expected");
        }
    }

    fn schedules_added(&self) {
        if !self.schedules_added {
            panic!("Schedules not added and processed when expected");
        }
    }

    fn systems_added(&self) {
        if !self.systems_added {
            panic!("Systems not Added and Configured when expected");
        }
    }

    fn built(&self) {
        self.plugins_processed();
        self.schedules_added();
        self.systems_added();
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

struct SystemsBlock {
    schedule_id: ScheduleId,
    systems: Vec<Box<dyn System>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

pub struct App {
    pub(crate) world: World,
    startup_schedule: Schedule,
    cleanup_schedule: Schedule,
    schedules: Vec<Schedule>,
    plugins: Vec<Box<dyn PluginsBuildAll>>,
    systems_blocks: Vec<SystemsBlock>,
    pub(crate) schedule_order_constraints: Vec<(ScheduleId, ScheduleId)>,
    runner_fn: fn(&mut App),
    configuration: ConfigurationContext,
}

impl App {
    pub fn new() -> Self {
        if APP_INITIALIZED.swap(true, Ordering::Relaxed) {
            panic!(
                "❌ AVENIX ARCHITECTURE VIOLATION: Multiple App instances detected!\nEnsure you only instantiate exactly one App::new() across your entire binary runtime."
            );
        }

        let function = |app: &mut App| {
            app.build();
            app.run_startup();
            loop {
                app.update();
            }
        };

        Self {
            world: World::new(),
            startup_schedule: Schedule::new(Startup),
            cleanup_schedule: Schedule::new(CleanupHandles),
            schedules: Vec::new(),
            plugins: Vec::new(),
            systems_blocks: Vec::new(),
            schedule_order_constraints: Vec::new(),
            runner_fn: function,
            configuration: ConfigurationContext::new(),
        }
    }

    pub fn add_schedule<L: ScheduleLabel + 'static>(&mut self, schedule: L) -> &mut Self {
        self.configuration.not_ready();

        if schedule.id() == Startup.id() {
            self.startup_schedule = Schedule::new(schedule);
        } else if schedule.id() == CleanupHandles.id() {
            panic!("CleanupHandles schedule cannnot be overwritten")
        } else {
            self.schedules.push(Schedule::new(schedule));
        }
        self
    }

    pub fn configure_schedule_order(
        &mut self,
        before: impl ScheduleLabel,
        after: impl ScheduleLabel,
    ) -> &mut Self {
        self.configuration.not_ready();
        self.schedule_order_constraints
            .push((before.id(), after.id()));
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
            .find(|b| b.schedule_id == schedule.id())
        {
            existing_block.systems.extend(configs.systems);
        } else {
            let block = SystemsBlock {
                schedule_id: schedule.id(),
                systems: configs.systems,
            };
            self.systems_blocks.push(block);
        }
        self
    }

    pub fn add_plugins(&mut self, plugins: impl PluginsBuildAll + 'static) -> &mut Self {
        self.configuration.not_ready();
        self.plugins.push(Box::new(plugins));
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

    pub fn build(&mut self) {
        if self.configuration.is_building_plugins() {
            panic!("App::build() was called while building plugins")
        }
        self.configuration.not_ready();
        self.build_everything();
        self.configuration.built = true;
    }

    pub fn run_startup(&mut self) {
        if self.configuration.is_building_plugins() {
            panic!("App::run_startup() was called while building plugins")
        }
        self.configuration.built();
        if self.configuration.ran_startup {
            panic!("Startup Already ran");
        }
        self.startup_schedule.run(&mut self.world);
        self.configuration.ran_startup = true;
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
        let function = self.runner_fn;
        function(self);
    }

    pub fn set_runner(&mut self, function: fn(&mut App)) -> &mut Self {
        self.runner_fn = function;
        self
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn remove_resource<T: Resource>(&mut self) -> &mut Self {
        self.world.remove_resource::<T>();
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
}

impl App {
    fn build_everything(&mut self) {
        self.configure_plugins();
        self.configure_schedules();
        self.configure_systems();
        #[cfg(feature = "reactivity")]
        register_removal_tracking_buffers(self);
    }

    fn configure_plugins(&mut self) {
        self.configuration.building_plugins = true;
        let mut seen_plugins = FxHashSet::default();

        while !self.plugins.is_empty() {
            let current_batch = std::mem::take(&mut self.plugins);
            for plugin_group in current_batch.iter() {
                for name in plugin_group.get_plugin_names() {
                    if !seen_plugins.insert(name) {
                        panic!(
                            "Duplicate plugin detected! The plugin '{}' has already been registered.",
                            name
                        );
                    }
                }
            }
            for plugins_build_all in current_batch {
                plugins_build_all.build_all(self);
            }
        }

        self.configuration.plugins_processed = true;
        self.configuration.building_plugins = false;
    }

    fn configure_systems(&mut self) {
        for system_block in self.systems_blocks.drain(..) {
            if system_block.schedule_id == Startup.id() {
                for system in system_block.systems {
                    self.startup_schedule.add_system(system);
                }
                continue;
            }
            if system_block.schedule_id == CleanupHandles.id() {
                for system in system_block.systems {
                    self.cleanup_schedule.add_system(system);
                }
                continue;
            }
            let target_schedule = match self
                .schedules
                .iter_mut()
                .find(|s| s.id() == system_block.schedule_id)
            {
                Some(schedule) => schedule,
                None => {
                    let missing_name = system_block.schedule_id.name;
                    panic!(
                        "❌ CONFIGURATION ERROR: Attempted to add systems to Schedule '{}' which was never registered via add_schedule()!",
                        missing_name
                    );
                }
            };

            for system in system_block.systems {
                target_schedule.add_system(system);
            }
        }
        self.configuration.systems_added = true;
    }

    fn configure_schedules(&mut self) {
        let unarranged = std::mem::take(&mut self.schedules);
        let mut schedule_map: FxHashMap<ScheduleId, Schedule> =
            FxHashMap::with_capacity_and_hasher(unarranged.len(), FxBuildHasher::default());
        for s in unarranged {
            let id = s.id();
            if schedule_map.contains_key(&id) {
                panic!(
                    "❌ CONFIGURATION ERROR: Duplicate Schedule detected for '{}'!",
                    id.name
                );
            }
            schedule_map.insert(id, s);
        }

        let mut adjacency_list: FxHashMap<ScheduleId, Vec<ScheduleId>> = FxHashMap::default();
        let mut in_degree: FxHashMap<ScheduleId, usize> = FxHashMap::default();

        for &id in schedule_map.keys() {
            in_degree.insert(id, 0);
            adjacency_list.entry(id).or_default();
        }

        for &(before_id, after_id) in &self.schedule_order_constraints {
            if before_id == Startup.id() || after_id == Startup.id() {
                panic!(
                    "❌ CONFIGURATION ERROR: Ordering constraint references the 'Startup' root! Startup is completely isolated from dynamic ordering rules."
                );
            }
            if before_id == CleanupHandles.id() || after_id == CleanupHandles.id() {
                panic!("CleanupHandles cannot be used for ordering schedules")
            }
            if !schedule_map.contains_key(&before_id) {
                panic!(
                    "❌ CONFIGURATION ERROR: Ordering constraint references an unregistered schedule: '{}'",
                    before_id.name
                );
            }
            if !schedule_map.contains_key(&after_id) {
                panic!(
                    "❌ CONFIGURATION ERROR: Ordering constraint references an unregistered schedule: '{}'",
                    after_id.name
                );
            }

            adjacency_list.entry(before_id).or_default().push(after_id);
            *in_degree.entry(after_id).or_default() += 1;
        }
        let mut sorted_ids = Vec::with_capacity(schedule_map.len());

        let mut queue: VecDeque<ScheduleId> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        while let Some(u) = queue.pop_front() {
            sorted_ids.push(u);

            if let Some(neighbors) = adjacency_list.get(&u) {
                for &v in neighbors {
                    let deg = in_degree.get_mut(&v).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(v);
                    }
                }
            }
        }
        if sorted_ids.len() != schedule_map.len() {
            let mut trapped_schedules = Vec::new();
            for id in schedule_map.keys() {
                if !sorted_ids.contains(id) {
                    trapped_schedules.push(id.name);
                }
            }
            panic!(
                "❌ CONFIGURATION ERROR: Circular dependency deadlock detected in Schedule constraints! Trapped Schedules: {:?}",
                trapped_schedules
            );
        }

        self.schedules = sorted_ids
            .into_iter()
            .map(|id| schedule_map.remove(&id).unwrap())
            .collect();

        self.configuration.schedules_added = true;
    }
}
