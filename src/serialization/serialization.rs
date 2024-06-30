use serde::{Serialize, Deserialize};
use bevy::prelude::{Color};
use bevy::pbr::{PointLight, SpotLight, DirectionalLight, AmbientLight};
use bevy::math::Vec3;
use bevy::pbr::FogFalloff;
use bevy::pbr::FogSettings;

#[derive(Serialize, Deserialize)]
#[serde(remote = "PointLight")]
pub struct SerializablePointLight {
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
    pub radius: f32,
    pub shadows_enabled: bool,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "SpotLight")]
pub struct SerializableSpotLight {
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
    pub radius: f32,
    pub shadows_enabled: bool,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    pub outer_angle: f32,
    pub inner_angle: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "DirectionalLight")]
pub struct SerializableDirectionalLight {
    pub color: Color,
    pub illuminance: f32,
    pub shadows_enabled: bool,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "AmbientLight")]
pub struct SerializableAmbientLight{
    pub color: Color,
    pub brightness: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "FogSettings")]
pub struct SerializableFogSettings {
    pub color: Color,
    pub directional_light_color: Color,
    pub directional_light_exponent: f32,
    #[serde(with = "SerializableFogFalloff")]
    pub falloff: bevy_pbr::FogFalloff,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "FogFalloff")]
pub enum SerializableFogFalloff {
    Linear {
        start: f32,
        end: f32,
    },
    Exponential {
        density: f32,
    },
    ExponentialSquared {
        density: f32,
    },
    Atmospheric {
        extinction: Vec3,
        inscattering: Vec3,
    },
}
