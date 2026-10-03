use avenix::prelude::*;

#[derive(States, Default, Debug, Clone, PartialEq, Eq)]
enum GameState {
    #[default]
    MainMenu,
    InGame,
}

#[derive(Resource, Default)]
struct TransitionTracker {
    history: Vec<&'static str>,
}

fn main() {
    println!("=== Avenix States Integration Test ===");

    let mut app = App::new();
    app.init_state::<GameState>()
        .insert_resource(TransitionTracker::default())
        .add_systems(
            Update,
            (
                cleanup_menu_assets.run_if(exited_state(GameState::MainMenu)),
                setup_game_world
                    .run_if(entered_state(GameState::InGame))
                    .run_if(not(in_state(GameState::MainMenu))),
            )
                .chain(),
        )
        .add_systems(
            Update,
            gameplay_system
                .run_if(in_state(GameState::InGame))
                .after(setup_game_world),
        );

    app.build();
    app.run_startup();

    println!("Checking initial state conditions...");
    app.update();
    {
        let tracker = app.world().get_resource::<TransitionTracker>();
        let state = app.world().get_resource::<State<GameState>>();

        assert_eq!(*state.current(), GameState::MainMenu);
        assert!(tracker.history.is_empty());
    }

    println!("Scheduling state modification hook...");
    {
        let mut next_state = app.world_mut().get_resource_mut::<NextState<GameState>>();
        next_state.set(GameState::InGame);
    }

    println!("Processing transition lifecycle step...");
    app.update();
    {
        let tracker = app.world().get_resource::<TransitionTracker>();
        let state = app.world().get_resource::<State<GameState>>();

        assert_eq!(*state.current(), GameState::InGame);
        assert_eq!(*state.last(), GameState::MainMenu);
        assert!(state.just_changed());

        assert_eq!(
            tracker.history,
            vec!["exit_menu", "enter_game", "gameplay_tick"]
        );
    }

    println!("Verifying lifecycle boundary stabilization...");
    app.update();
    {
        let tracker = app.world().get_resource::<TransitionTracker>();
        let state = app.world().get_resource::<State<GameState>>();

        assert_eq!(*state.current(), GameState::InGame);
        assert!(!state.just_changed());

        assert_eq!(
            tracker.history,
            vec!["exit_menu", "enter_game", "gameplay_tick", "gameplay_tick"]
        );
    }

    println!(" -> Success: State transition engine verified perfectly across all frame cycles!");
}

fn cleanup_menu_assets(mut tracker: ResMut<TransitionTracker>) {
    println!(" >> Executing exited_state(MainMenu) hook");
    tracker.history.push("exit_menu");
}

fn setup_game_world(mut tracker: ResMut<TransitionTracker>) {
    println!(" >> Executing entered_state(InGame) hook");
    tracker.history.push("enter_game");
}

fn gameplay_system(mut tracker: ResMut<TransitionTracker>) {
    println!(" >> Executing Gameplay System Tick");
    tracker.history.push("gameplay_tick");
}
