use libnoise::{Generator, Source};
use bevy::prelude::*;
use crate::proc_gen::core::structure_key::StructureKey;
use crate::proc_gen::core::sample_size::SampleSize;
use rand::Rng;
use crate::generation::GenRng;
use crate::proc_gen::core::seeded_or_not::SeededOrNot;
use crate::proc_gen::spawning::euler_transform::EulerTransform;

pub fn get_looped_position_list(origin: Vec3, transform: EulerTransform, x_times: usize) -> Vec<Vec3> {
    let mut positions = Vec::new();

    for n in 1..=x_times {
        let translated = Vec3::new(
            transform.translation.0 * (1.0 + transform.scale.0 * (n as f32 - 1.0)),
            transform.translation.1 * (1.0 + transform.scale.1 * (n as f32 - 1.0)),
            transform.translation.2 * (1.0 + transform.scale.2 * (n as f32 - 1.0)),
        );

        let rotation = Quat::from_euler(
            bevy::math::EulerRot::XYZ,
            transform.rotation.0.to_radians() * (n - 1) as f32,
            transform.rotation.1.to_radians() * (n - 1) as f32,
            transform.rotation.2.to_radians() * (n - 1) as f32,
        );

        let rotated_pos = rotation * translated;
        positions.push(origin + rotated_pos);
    }

    positions
}

pub fn generate_noise_spawn_points(
    data: &StructureKey,
    gen_rng: &mut ResMut<GenRng>,
) -> Vec<(f32, f32, f32)> {
    let (fbm, sample_size, count, exclusivity_radius, resolution_modifier) = if let StructureKey::NoiseSpawn {
        fbm,
        sample_size,
        count,
        exclusivity_radius,
        resolution_modifier, ..
    } = data {
        (fbm, sample_size, count, exclusivity_radius, resolution_modifier)
    } else {
        unreachable!()
    };

    let seed = match fbm.seed {
        SeededOrNot::Seeded(s) => s,
        SeededOrNot::Unseeded => gen_rng.rng_mut().gen::<u64>(),
    };

    match sample_size {
        SampleSize::UBiDim(x) => {
            generate_noise_spawn_points_2d(
                (x, x),
                fbm.scale,
                fbm.octaves,
                fbm.frequency,
                fbm.lacunarity,
                fbm.persistence,
                count,
                exclusivity_radius,
                resolution_modifier,
                seed,
            )
        }
        SampleSize::BiDim(x, y) => {
            generate_noise_spawn_points_2d(
                (x, y),
                fbm.scale,
                fbm.octaves,
                fbm.frequency,
                fbm.lacunarity,
                fbm.persistence,
                count,
                exclusivity_radius,
                resolution_modifier,
                seed,
            )
        }
        _ => {
            panic!("THIS NOISE DIMENSIONALITY IS NOT IMPLEMENTED!");
        }
    }
}

pub fn generate_noise_spawn_points_2d(
    sample_size: (&i32, &i32),
    scale: f32,
    octaves: u8,
    frequency: f32,
    lacunarity: f32,
    persistence: f32,
    spawn_count: &u32,
    exclusivity_radius: &f32,
    resolution_modifier: &f32,
    seed: u64,
) -> Vec<(f32, f32, f32)> {
    let effective_width = *sample_size.0 as f32 * resolution_modifier;
    let effective_height = *sample_size.1 as f32 * resolution_modifier;

    assert!(effective_width as i64 * effective_height as i64 <= 2097152,
            "Product of sample size dimensions and resolution modifier must be no larger than 262144.");
    assert!(effective_width % 2.0 == 0.0 && effective_height % 2.0 == 0.0,
            "Effective dimensions (sample size multiplied by resolution modifier) must result in integers divisible by 2.");

    let generator = Source::simplex(seed)
        .fbm(octaves as u32, frequency as f64, lacunarity as f64, persistence as f64)
        .scale([scale as f64; 3]);

    let start_sample_x = sample_size.0 / 2 * *resolution_modifier as i32;
    let end_sample_x = 3 * sample_size.0 / 2 * *resolution_modifier as i32;
    let start_sample_y = sample_size.1 / 2 * *resolution_modifier as i32;
    let end_sample_y = 3 * sample_size.1 / 2 * *resolution_modifier as i32;

    let radius_x = (end_sample_x - start_sample_x) as f32 / 2.0;
    let radius_y = (end_sample_y - start_sample_y) as f32 / 2.0;

    let centerpoint = Vec2::new(
        ((start_sample_x + end_sample_x) / 2) as f32,
        ((start_sample_y + end_sample_y) / 2) as f32,
    );

    let mut values_and_coords = Vec::new();

    for x in start_sample_x..=end_sample_x {
        for y in start_sample_y..=end_sample_y {
            let sample_x = x as f32 / resolution_modifier;
            let sample_y = y as f32 / resolution_modifier;

            let nx = (sample_x - centerpoint.x) / radius_x;
            let ny = (sample_y - centerpoint.y) / radius_y;

            if nx.powi(2) + ny.powi(2) > 1.0 {
                continue;
            }

            let value = generator.sample([sample_x as f64, sample_y as f64, (start_sample_x as f64 + start_sample_y as f64) / 2.0]);

            values_and_coords.push((sample_x, sample_y, 0.0, value));
        }
    }

    values_and_coords.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

    filter_by_exclusivity(&values_and_coords, spawn_count, exclusivity_radius)
        .iter()
        .map(|&(x, y, _)| (
            (x - centerpoint.x) / centerpoint.x,
            (y - centerpoint.y) / centerpoint.y,
            0.0
        ))
        .collect()
}

fn filter_by_exclusivity(
    sorted_values: &Vec<(f32, f32, f32, f64)>,
    n: &u32,
    radius: &f32,
) -> Vec<(f32, f32, f32)> {
    let mut results = Vec::new();
    let mut candidates = std::collections::VecDeque::from(sorted_values.to_vec());

    let square_radius = radius * radius;

    while results.len() < *n as usize && !candidates.is_empty() {
        if let Some((x, y, z, _)) = candidates.pop_front() {
            results.push((x, y, z));

            candidates = candidates.into_iter().filter(|&(cx, cy, cz, _)| {
                let square_distance = (x - cx).powi(2) + (y - cy).powi(2) + (z - cz).powi(2);
                square_distance > square_radius
            }).collect();

            if results.len() >= *n as usize {
                break;
            }
        }
    }

    results
}
