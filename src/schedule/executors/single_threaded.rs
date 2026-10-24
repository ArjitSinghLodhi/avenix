use crate::{
    app::system::SystemExt,
    schedule::{SystemExecutor, SystemsSchedule},
    system::condition::RunConditionsList,
    world::World,
};

#[derive(Default)]
pub struct SingleThreadedExecutor;

impl SystemExecutor for SingleThreadedExecutor {
    fn init(&mut self, _schedule: &mut SystemsSchedule) {}
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World) {
        for node in schedule.systems_mut() {
            let should_run = node
                .system
                .get_or_init(RunConditionsList::default)
                .conditions()
                .iter()
                .all(|cond| cond(world));

            if should_run {
                node.system.run(world);
            }
        }
    }
}
