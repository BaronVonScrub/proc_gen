use std::fs;
use std::fs::write;
use std::path::{Path, PathBuf};
use image::ImageReader as ImageReader;
use image::{RgbImage, Rgb, DynamicImage};
use image::GenericImageView;

fn main() {
    generate_metallic_roughness_maps();
    generate_texture_meta_files();
    generate_ui_sprites();
    generate_ui_meta_files();
}

fn generate_ui_sprites() {
    let sprites_path = Path::new("assets/ui");
    let dest_path = sprites_path.join("ui_sprite_names.rs");
    let entries = fs::read_dir(sprites_path).unwrap_or_else(|_| {
        panic!("Failed to read directory {:?}", sprites_path);
    });

    let sprite_names: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().is_file() &&
                entry.file_name() != "ui_sprite_names.rs" && // Exclude the script file itself
                !entry.file_name().to_string_lossy().ends_with(".meta")
        })
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    let sprite_names_array = format!("#[allow(dead_code)]\npub const UI_SPRITE_NAMES: &[&str] = &{:?};", sprite_names);
    write(&dest_path, sprite_names_array).unwrap();
}

fn generate_ui_meta_files() {
    let ui_sprites_path = Path::new("assets/ui");
    for entry in fs::read_dir(ui_sprites_path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() && path.extension().unwrap_or_default() == "png" {
            let meta_path = path.with_extension("png.meta");
            if !meta_path.exists() {
                let meta_content = create_ui_meta_content();
                write(&meta_path, meta_content).unwrap();
            }
        }
    }
}

fn create_ui_meta_content() -> String {
    r#"(
        meta_format_version: "1.0",
        asset: Load(
            loader: "bevy_render::texture::image_loader::ImageLoader",
            settings: (
                format: FromExtension,
                is_srgb: true,
                sampler: Default,
                asset_usage:("MAIN_WORLD | RENDER_WORLD"),
            ),

        ),
    )
    "#.to_string()
}

fn generate_texture_meta_files() {
    let materials_path = Path::new("assets/materials");
    for entry in fs::read_dir(materials_path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            for material_entry in fs::read_dir(&path).unwrap() {
                let material_path = material_entry.unwrap().path();
                if material_path.extension().unwrap_or_default() == "png" {
                    let meta_path = material_path.with_extension("png.meta");
                    if !meta_path.exists() {
                        let is_normal_map = material_path.file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .ends_with("_normal");
                        let meta_content = create_meta_content(is_normal_map);
                        write(&meta_path, meta_content).unwrap();
                    }
                }
            }
        }
    }
}

fn create_meta_content(is_normal_map: bool) -> String {
    format!(r#"(
    meta_format_version: "1.0",
    asset: Process(
        processor: "bevy_asset::processor::process::LoadAndSave<bevy_render::texture::image_loader::ImageLoader, bevy_render::texture::compressed_image_saver::CompressedImageSaver>",
        settings: (
            loader_settings: (
                format: FromExtension,
                is_srgb: {},
                sampler: Descriptor((
                    label: None,
                    address_mode_u: Repeat,
                    address_mode_v: Repeat,
                    address_mode_w: ClampToEdge,
                    mag_filter: Linear,
                    min_filter: Linear,
                    mipmap_filter: Linear,
                    lod_min_clamp: 0.0,
                    lod_max_clamp: 32.0,
                    compare: None,
                    anisotropy_clamp: 1,
                    border_color: None,
                )),
                asset_usage:("MAIN_WORLD | RENDER_WORLD"),
            ),
            saver_settings: (),
        ),
    ),
)
"#, if is_normal_map { "false" } else { "true" })
}

fn generate_metallic_roughness_maps() {
    let materials_path = Path::new("assets/materials");
    for entry in fs::read_dir(materials_path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            let mut metallic_path = None;
            let mut roughness_path = None;

            let mut exists = false;

            for inner_entry in fs::read_dir(&path).unwrap() {
                let inner_path = inner_entry.unwrap().path();
                if let Some(file_name) = inner_path.file_name().and_then(|n| n.to_str()) {
                    if file_name.to_lowercase().ends_with("_metallicroughness.png") {
                        exists=true;
                        continue;
                    }
                    if file_name.to_lowercase().ends_with("_metallic.png") {
                        metallic_path = Some(inner_path.clone());
                        continue;
                    }
                    if file_name.to_lowercase().ends_with("_roughness.png") {
                        roughness_path = Some(inner_path.clone());
                        continue;
                    }
                }
            }

            if exists==true {
                continue;
            }

            println!("cargo:warning=MetallicRoughness map not existent; creating!");

            if let (Some(metallic_path), Some(roughness_path)) = (metallic_path.clone(), roughness_path.clone()) {
                combine_metallic_roughness(&metallic_path, &roughness_path);
            } else if let Some(metallic_path) = metallic_path {
                combine_metallic_roughness(&metallic_path, &PathBuf::new());
            } else if let Some(roughness_path) = roughness_path {
                combine_metallic_roughness(&PathBuf::new(), &roughness_path);
            }
        }
    }
}

fn combine_metallic_roughness(
    input_metallic_path: &Path,
    input_roughness_path: &Path
) {
    // Determine which path exists
    let existing_path = if input_metallic_path.exists() {
        input_metallic_path
    } else {
        input_roughness_path
    };

    // Extract file stem up to underscore and append "metallicRoughness"
    let file_stem = existing_path.file_stem().unwrap().to_string_lossy();
    let combined_filename = format!("{}_metallicRoughness.png", &file_stem[..file_stem.rfind('_').unwrap()]);
    let output_path = existing_path.with_file_name(combined_filename);


    // Load metallic and roughness images
    let metallic_img_maybe = load_image(input_metallic_path);
    let roughness_img_maybe = load_image(input_roughness_path);

    // Use the dimensions of the existing images or default to 1 if both images are missing
    let width = metallic_img_maybe
        .as_ref()
        .map_or_else(|| roughness_img_maybe
            .as_ref()
            .map_or(1, |img| img.width()), |img| img.width());
    let height = metallic_img_maybe
        .as_ref()
        .map_or_else(|| roughness_img_maybe
            .as_ref()
            .map_or(1, |img| img.height()), |img| img.height());

    let mut combined_img = RgbImage::new(width, height);

    match (metallic_img_maybe, roughness_img_maybe) {
        (Some(metallic_img), Some(roughness_img)) => {
            // Both images are successfully loaded
            for x in 0..width {
                for y in 0..height {
                    let metallic_blue = metallic_img.get_pixel(x, y)[1];
                    let roughness_green = roughness_img.get_pixel(x, y)[1];
                    combined_img.put_pixel(x, y, Rgb([0, roughness_green, metallic_blue]));
                }
            }
        }
        (Some(metallic_img), _) => {
            let roughness_green = 255;
            // Only metallic_img is successfully loaded
            for x in 0..width {
                for y in 0..height {
                    let metallic_blue = metallic_img.get_pixel(x, y)[1];
                    combined_img.put_pixel(x, y, Rgb([0, roughness_green, metallic_blue]));
                }
            }
        }
        (_, Some(roughness_img)) => {
            // Only roughness_img is successfully loaded
            let metallic_blue = 255;
            for x in 0..width {
                for y in 0..height {
                    let roughness_green = roughness_img.get_pixel(x, y)[1];
                    combined_img.put_pixel(x, y, Rgb([0, roughness_green, metallic_blue]));
                }
            }
        }
        _ => {}
    }

    // Attempt to save the image
    if let Err(error) = combined_img.save(&output_path) {
        println!(
            "cargo:warning=Error saving image: {}",
            error
        );
    }
}

fn load_image(path: &Path) -> Option<DynamicImage> {
    match ImageReader::open(path) {
        Ok(reader) => match reader.decode() {
            Ok(img) => Some(img),
            Err(_) => None,
        },
        Err(_) => None,
    }
}