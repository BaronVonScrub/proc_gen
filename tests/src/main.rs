use bevy::prelude::*;
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioApp, AudioPlugin};
use bevy_pbr::CascadeShadowConfig;
use proc_gen::core::components::MainDirectionalLight;
use proc_gen::core::generator_plugin::GeneratorPlugin;
use proc_gen::systems::events::*;

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
                    mode: bevy::window::WindowMode::BorderlessFullscreen,
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
        color: Color::rgba(154.0 / 255.0, 166.0 / 255.0, 254.0 / 255.0, 1.0),
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

    app.add_systems(OnEnter(proc_gen::management::material_autoloader::GameState::Playing), (ingame_setup,generation::generate_map).chain());

    // Setup events
    app.add_event::<ObjectSpawnEvent>()
        .add_event::<FogEvent>()
        .add_event::<DirLightEvent>()
        .add_event::<AmbLightEvent>()
        .add_event::<BGMusicEvent>()
        .add_event::<SFXEvent>()
        .add_event::<SelectiveReplacementEvent>();

    // Setup map generator
    app.add_plugins(GeneratorPlugin);

    // Setup input system
    app.add_plugins(crate::input_manager::InputPlugin);

    // Setup camera
    app.add_plugins(crate::camera::CameraPlugin);

    // Setup audio: add the audio plugin, an audio channel, and spacial audio resource
    app.add_plugins(AudioPlugin)
        .add_audio_channel::<proc_gen::management::audio_management::SoundEffects>()
        //.add_systems(Startup, audio_manager::start_background_audio)
        .insert_resource(bevy_kira_audio::prelude::SpacialAudio { max_distance: 25. });

    // Setup physics
    //app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());

    // Setup object logic
    app.add_plugins(proc_gen::spawning::object_logic::ObjectLogicPlugin);

    // Optionally, if needed, you can add the physics debug plugin:
    // app.add_plugins(RapierDebugRenderPlugin::default());

    app.run();
}
fn ingame_setup(
    mut commands: Commands,
    //mut rapier_config: ResMut<RapierConfiguration>
) {
    /*rapier_config.timestep_mode = TimestepMode::Variable {
        max_dt: 10.0,
        time_scale: 10.0,
        substeps: 10,
    };*/

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            illuminance: 30000.0,
            color: Color::rgba(171.0 / 255.0, 183.0 / 255.0, 255.0 / 255.0, 1.0),
            ..default()
        },
        transform: Transform::from_rotation(
            Quat::from_euler(EulerRot::XYZ,0.0,3.1,-6.3)),
        cascade_shadow_config: CascadeShadowConfig {
            bounds: vec![0.0, 30.0, 90.0, 270.0],
            overlap_proportion: 0.2,
            minimum_distance: 0.0,
        },
        ..default()
    })
        .insert(MainDirectionalLight);
}
