use avenix::prelude::*;

fn on_add_burning(world: &mut DeferredWorld, entity: &Entity) {
    world.insert_components(entity.clone(), VisualEffectComponent { effect_type: 1 });
}

fn on_insert_burning(world: &mut DeferredWorld, entity: &Entity) {
    let mut meets_condition = false;
    {
        if let Some(fx) = world.get::<VisualEffectComponent>(entity) {
            if fx.effect_type == 1 {
                meets_condition = true;
            }
        }
    }
    if meets_condition {
        if let Some(mut fx) = world.get_mut::<VisualEffectComponent>(entity) {
            fx.effect_type = 2;
        }
    }
}

fn on_discard_burning(world: &mut DeferredWorld, entity: &Entity) {
    let mut meets_condition = false;
    {
        if let Some(burning) = world.get::<BurningEffect>(entity) {
            if burning.intensity > 5.0 {
                meets_condition = true;
            }
        }
    }
    if meets_condition {
        world.insert_components(entity.clone(), CombustionMarker);
    }
}

fn on_remove_burning(world: &mut DeferredWorld, entity: &Entity) {
    world.remove_components::<VisualEffectComponent>(entity.clone());
    world.remove_components::<CombustionMarker>(entity.clone());
}

#[derive(Component)]
#[component(
    on_add = on_add_burning,
    on_insert = on_insert_burning,
    on_discard = on_discard_burning,
    on_remove = on_remove_burning
)]
struct BurningEffect {
    intensity: f32,
    #[allow(unused)]
    duration: f32,
}

#[derive(Component)]
struct VisualEffectComponent {
    effect_type: u32,
}

#[derive(Component)]
struct CombustionMarker;

fn setup_entities(commands: Commands) {
    commands.spawn(BurningEffect {
        intensity: 2.0,
        duration: 3.0,
    });
}

fn update_simulation(commands: Commands, query: Query<Entity, EmptyQueryFilter>) {
    for entity in query.iter() {
        commands.entity(&entity).insert(BurningEffect {
            intensity: 10.0,
            duration: 2.0,
        });
    }
}

fn verify_state(
    query: Query<(Option<&VisualEffectComponent>, Option<&CombustionMarker>), EmptyQueryFilter>,
) {
    for (fx, combustion) in query.iter() {
        if let Some(effect) = fx {
            println!("VisualEffectComponent type: {}", effect.effect_type);
        }
        if combustion.is_some() {
            println!("CombustionMarker present via on_discard intercept!");
        }
    }
}

fn main() {
    let mut app = App::new();

    app.add_systems(Startup, setup_entities)
        .add_systems(Update, (update_simulation, verify_state).chain());

    app.build();
    app.run_startup();
    app.update();
}
