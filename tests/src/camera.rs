use bevy::app::{App, Plugin};
use bevy::input::mouse::{ MouseWheel};
use bevy::prelude::*;
use bevy_atmosphere::plugin::AtmosphereCamera;
use bevy_kira_audio::prelude::AudioReceiver;
use crate::input_manager::{InputStates, MouseButtonState};
use proc_gen::core::components::MainCamera;

#[derive(Component)]
pub(crate) struct CameraFocus;
#[derive(Component)]
struct CameraSystem;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, initialize_camera_system)
            .add_systems(Update, camera_controller_system)
            .insert_resource(ZoomParameters::default());
    }
}

pub(crate) fn initialize_camera_system(
    mut commands: Commands,
    zoom_parameters: ResMut<ZoomParameters>
) {
    let focus_trans = Transform::default();

    let mut cam_trans = Transform::from_xyz(7.0, 10.0, 7.0).looking_at(focus_trans.translation, Vec3::Y);

    // Sample the curve at the current position
    let curve_sample = zoom_parameters.curve.position(zoom_parameters.position);

    // Get the forward vector, as we know the camera is already facing the focal point
    let forward = cam_trans.rotation * Vec3::Z;
    // Project the forward vector onto the X-Z plane and normalize
    let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize();

    // Create the new position using the sampled curve values
    let new_position = focus_trans.translation
        + Vec3::new(0.0, curve_sample.y, 0.0) // Y offset from the curve sample
        + forward_xz * curve_sample.x;        // XZ offset in the forward direction

    // Update camera translation
    cam_trans.translation = new_position;

    // Update camera rotation to look at the focus point
    cam_trans.look_at(focus_trans.translation, Vec3::Y);

    // Spawn the parent entity named "CameraSystem" with the tag component
    let camera_system_entity = commands.spawn(
        SpatialBundle {
            transform: focus_trans,
            ..default()
        })
        .insert(Name::new("CameraSystem"))
        .insert(CameraSystem)
        .id();

    // Spawn the CameraFocus entity as a child of the CameraSystem entity
    commands.spawn((
        TransformBundle {
            local: focus_trans,
            global: Default::default(),
        },
        Name::new("CameraFocus"),
    ))
        .insert(CameraFocus)
        .set_parent(camera_system_entity);

    // Spawn the MainCamera entity as a child of the CameraSystem entity
    commands.spawn(Camera3dBundle {
        transform: cam_trans,
        ..default()
    })
        .insert(AtmosphereCamera::default())
        .insert(FogSettings {
            color: Color::rgba(0.35, 0.48, 0.66, 1.0),
            directional_light_color: Color::rgba(171.0 / 255.0, 183.0 / 255.0, 255.0 / 255.0, 1.0),
            directional_light_exponent: 30.0,
            falloff: FogFalloff::from_visibility_colors(
                20.0,
                Color::rgb(0.1, 0.1, 0.1),
                Color::rgb(0.5, 0.5, 0.5),
            ),
        })
        .insert(Name::new("MainCamera"))
        .insert(MainCamera)
        .insert(AudioReceiver)
        .set_parent(camera_system_entity);
}

#[derive(Resource)]
pub(crate) struct ZoomParameters {
    curve: bevy_math::cubic_splines::CubicCurve<Vec2>,
    position: f32
}

impl Default for ZoomParameters {
    fn default() -> Self {
        // Define your points for the spline
        let points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(3.0, 0.0),
            Vec2::new(30.0, 25.0),
            Vec2::new(0.1, 100.0),
            Vec2::new(0.0, 100.0),
        ];
        let curve = CubicCardinalSpline::new(0.5, points).to_curve();

        ZoomParameters {
            curve,
            position: 0.5, // Initialize the position to 0
        }
    }
}

fn camera_controller_system(
    mut system_query: Query<&mut Transform, (With<CameraSystem>, Without<MainCamera>, Without<CameraFocus>)>,
    focus_query: Query<&Transform, (With<CameraFocus>, Without<MainCamera>, Without<CameraSystem>)>,
    mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<CameraFocus>, Without<CameraSystem>)>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    input: Res<InputStates>,
    mut zoom_parameters: ResMut<ZoomParameters>,
) {
    let focus_trans = focus_query.get_single().expect("Focus transform not found!");
    let mut cam_trans = camera_query.get_single_mut().expect("Camera transform not found!");
    let mut sys_trans = system_query.get_single_mut().expect("System transform not found!");

    for event in mouse_wheel_events.read() {
        let scroll_increment = event.y * 0.1; // Adjust sensitivity as needed
        zoom_parameters.position += scroll_increment;
        zoom_parameters.position = zoom_parameters.position.clamp(0.1, zoom_parameters.curve.segments().len() as f32);
        let curve_sample = zoom_parameters.curve.position(zoom_parameters.position);
        let forward = cam_trans.rotation * Vec3::Z;
        let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize();
        let new_position = focus_trans.translation
            + Vec3::new(0.0, curve_sample.y, 0.0)
            + forward_xz * curve_sample.x;

        cam_trans.translation = new_position;
        cam_trans.look_at(focus_trans.translation, Vec3::Y);
        return;
    }

    for event in cursor_moved_events.read() {
        if let MouseButtonState::Held(_) = input.right {
            if let Some(delta) = event.delta {
                let direction = focus_trans.translation - cam_trans.translation;
                let distance = (direction.x.powi(2) + direction.z.powi(2)).sqrt();
                let mut angle = direction.z.atan2(direction.x);
                let delta_x = delta.x;
                let delta_angle = delta_x * 0.01; // Adjust sensitivity as needed
                angle += delta_angle;
                let new_x = focus_trans.translation.x + distance * -angle.cos();
                let new_z = focus_trans.translation.z + distance * -angle.sin();
                let new_position = Vec3::new(new_x, cam_trans.translation.y, new_z);
                cam_trans.translation = new_position;
                cam_trans.look_at(focus_trans.translation, Vec3::Y);
                return;
            }
        }

        if let MouseButtonState::Held(_) = input.middle {
            if let Some(delta) = event.delta {
                let forward = cam_trans.rotation * Vec3::Z;
                let right = cam_trans.rotation * Vec3::X;
                let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize();
                let right_xz = Vec3::new(right.x, 0.0, right.z).normalize();
                let delta_x = delta.x;
                let delta_y = delta.y;
                let translation_delta = right_xz * delta_x * 0.01 + forward_xz * delta_y * 0.01;
                sys_trans.translation += translation_delta;
                return;
            }
        }
    }
}