use bevy::prelude::*;
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioApp, AudioPlugin, SpatialAudioPlugin};
use bevy_rapier3d::prelude::{NoUserData, RapierPhysicsPlugin};
use proc_gen::core::generator_plugin::GeneratorPlugin;
use proc_gen::event_system::event_listeners::GenerationState;
use oxidized_navigation::{OxidizedNavigationPlugin, NavMeshSettings, NavMesh};
use oxidized_navigation::debug_draw::OxidizedNavigationDebugDrawPlugin;
use oxidized_navigation::query;

mod input_manager;
mod camera;
mod generation;

fn main() {
    let mut app = App::new();

    // Setup default plugins
    app.add_plugins(
        DefaultPlugins
            .set(bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::WgpuSettings {
                    backends: Some(bevy::render::settings::Backends::VULKAN),
                    ..default()
                }

                    .into(),
                ..default()
            })
            .set(bevy::log::LogPlugin {
                filter: "warn".to_string(),
                level: bevy::log::Level::WARN,
                ..default()
            })
            .set(ImagePlugin::default_linear())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Proc Gen Testing".into(),
                    resolution: (1024.0, 768.0).into(),
                    resizable: false,
                    mode: bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                mode: AssetMode::Processed,
                ..default()
            })
            .build(),
    );

    // Setup world (resources, types)
    app.insert_resource(AmbientLight {
        color: Color::srgba(154.0 / 255.0, 166.0 / 255.0, 254.0 / 255.0, 1.0),
        brightness: 75.0,
    });

    // Setup material autoloader
    //app.add_plugins(MaterialAutoloader);

    // Setup inspector plugins
    app.add_plugins(
        WorldInspectorPlugin::default().run_if(bevy::input::common_conditions::input_toggle_active(false, KeyCode::Escape)),
    );

    // Setup skybox (atmosphere)
    app.add_plugins(AtmospherePlugin);

    app.add_systems(OnEnter(proc_gen::management::material_autoloader::GameState::Playing), generation::generate_map);
    app.add_systems(Update, generation::reset_on_space);

    // Setup map generator
    app.add_plugins(GeneratorPlugin);

    // Setup input system
    app.add_plugins(crate::input_manager::InputPlugin);

    // Setup camera
    app.add_plugins(crate::camera::CameraPlugin);

    // Setup audio: add the audio plugin, an audio channel, and spacial audio resource
    app.add_plugins(AudioPlugin)
        .add_audio_channel::<proc_gen::management::audio_management::SoundEffects>();

    app.add_plugins(SpatialAudioPlugin);
        //.add_systems(Startup, audio_manager::start_background_audio)


    // Setup physics
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());

    // Optionally, if needed, you can add the physics debug plugin:
    // app.add_plugins(RapierDebugRenderPlugin::default());

    // Setup navmesh generation and debug draw
    app.add_plugins(OxidizedNavigationPlugin::<bevy_rapier3d::prelude::Collider>::new(
        NavMeshSettings::from_agent_and_bounds(
            0.5,   // agent radius
            1.9,   // agent height
            250.0, // world half extents
            -1.0,  // world bottom bound (y)
        ),
    ));
    app.add_plugins(OxidizedNavigationDebugDrawPlugin);

    // Path test state
    app.insert_resource(PathTestState::default());
    // Debug resource for visualizing the computed path
    app.insert_resource(PathDebug::default());

    // Setup object logic
    app.add_plugins(proc_gen::spawning::object_logic::ObjectLogicPlugin);

    // Add a simple path test system to run after things have spawned & navmesh built
    app.add_systems(Update, reset_path_test_on_space);
    app.add_systems(Update, (path_test_system, draw_path_debug).run_if(in_state(GenerationState::Completed)));

    app.run();
}

#[derive(Resource, Default)]
struct PathTestState {
    frames_waited: u32,
    done: bool,
}

#[derive(Resource, Default)]
struct PathDebug {
    points: Vec<Vec3>,
}

fn reset_path_test_on_space(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<PathTestState>,
    mut path_debug: ResMut<PathDebug>,
) {
    if keys.just_pressed(KeyCode::Space) {
        state.frames_waited = 0;
        state.done = false;
        path_debug.points.clear();
    }
}

fn path_test_system(
    mut state: ResMut<PathTestState>,
    nav_mesh: Option<Res<NavMesh>>, // present after plugin
    settings: Option<Res<NavMeshSettings>>, // inserted by plugin
    mut path_debug: ResMut<PathDebug>,
) {
    state.frames_waited = state.frames_waited.saturating_add(1);

    // Define start/end across the circle on XZ plane
    let start = Vec3::new(-8.0, 0.0, 0.0);
    let end = Vec3::new(8.0, 0.0, 0.0);

    let (nav_mesh, settings) = match (nav_mesh, settings) {
        (Some(nm), Some(ns)) => (nm, ns),
        _ => return,
    };

    // Read navmesh tiles and query a path (recompute each frame so debug stays in sync)
    if let Ok(tiles) = nav_mesh.get().read() {
        match query::find_path(&tiles, &settings, start, end, None, None) {
            Ok(points) if points.len() >= 2 => {
                // Update the debug path every frame
                path_debug.points = points.clone();
            }
            _ => {
                // Clear path if not valid this frame
                path_debug.points.clear();
            }
        }
    } else {
        path_debug.points.clear();
    }
}

fn draw_path_debug(
    mut gizmos: Gizmos,
    dbg: Res<PathDebug>,
) {
    if dbg.points.len() < 2 { return; }
    for w in dbg.points.windows(2) {
        let a = w[0];
        let b = w[1];
        gizmos.line(a, b, Color::srgba(1.0, 1.0, 0.0, 1.0));
    }
}
