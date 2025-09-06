use bevy::prelude::*;
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioApp, AudioPlugin, SpatialAudioPlugin};
use bevy_rapier3d::prelude::{NoUserData, RapierPhysicsPlugin};
#[cfg(feature = "debug")]
use bevy_rapier3d::prelude::RapierDebugRenderPlugin;
#[cfg(feature = "debug")]
use bevy_rapier3d::render::{DebugRenderContext, ColliderDebug};
use proc_gen::core::generator_plugin::GeneratorPlugin;
use oxidized_navigation::{OxidizedNavigationPlugin, NavMeshSettings};
use oxidized_navigation::debug_draw::OxidizedNavigationDebugDrawPlugin;

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
    // Physics debug render (colliders, contacts) when 'debug' feature is enabled
    #[cfg(feature = "debug")]
    {
        // Enable collider debug rendering by default
        app.insert_resource(DebugRenderContext {
            default_collider_debug: ColliderDebug::AlwaysRender,
            ..Default::default()
        });
        app.add_plugins(RapierDebugRenderPlugin::default());
    }

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

    // Setup object logic
    app.add_plugins(proc_gen::spawning::object_logic::ObjectLogicPlugin);

    app.run();
}
