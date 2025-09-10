use bevy::prelude::*;
use bevy_pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType, Shader};
use bevy::asset::load_internal_asset;

pub const MAX_PATH_SEGMENTS: usize = 256;

// Packed configuration for the path blend extension.
#[derive(Clone, Copy, ShaderType)]
pub struct PathBlendParams {
    // Fade configuration
    pub fade_radius: f32,
    // Global thickness multiplier. Distances are divided by this before applying falloff.
    // 1.0 = no change; >1.0 = thicker; <1.0 = thinner.
    pub thickness_scale: f32,
    // Base inner width (in world units) where the path is fully "near" (w=1) before falloff begins.
    // The falloff is applied beyond this radius over fade_radius.
    pub base_width: f32,
    pub min_blend: f32,
    pub max_blend: f32,
    // Near material scalars
    pub near_metallic: f32,
    pub near_roughness: f32,
    // Falloff/flags packed to be WebGL-friendly
    // flags.x = falloff_mode (0=smoothstep,1=inverse_sq,2=linear)
    // flags.y = invert (0/1)
    // flags.z = segment_count
    // flags.w = unused
    pub flags: UVec4,
    // Near color tint (includes alpha)
    pub near_base_color: Vec4,
    // World-XZ segments as (ax, az, bx, bz)
    #[align(16)]
    pub segments: [Vec4; MAX_PATH_SEGMENTS],
}

impl PathBlendParams {
    pub fn set_segments_from_points(&mut self, points: &[Vec3]) {
        // Convert a polyline of points into consecutive line segments
        let mut count = 0u32;
        if points.len() >= 2 {
            for w in points.windows(2) {
                if count as usize >= MAX_PATH_SEGMENTS { break; }
                let a = w[0];
                let b = w[1];
                self.segments[count as usize] = Vec4::new(a.x, a.z, b.x, b.z);
                count += 1;
            }
        }
        // Zero any remaining slots to be safe
        for i in count as usize..MAX_PATH_SEGMENTS {
            self.segments[i] = Vec4::ZERO;
        }
        // Update segment_count in flags.z
        self.flags = UVec4::new(self.flags.x, self.flags.y, count, 0);
    }

    pub fn clear_segments(&mut self) {
        for i in 0..MAX_PATH_SEGMENTS { self.segments[i] = Vec4::ZERO; }
        self.flags = UVec4::new(self.flags.x, self.flags.y, 0, 0);
    }

    // Build segments from multiple separate polylines without connecting them end-to-end.
    // Each polyline contributes its own windows(2) segments; boundaries are not bridged.
    pub fn set_segments_from_polylines(&mut self, polylines: &[Vec<Vec3>]) {
        let mut count: u32 = 0;
        for poly in polylines.iter() {
            if poly.len() < 2 { continue; }
            for w in poly.windows(2) {
                if count as usize >= MAX_PATH_SEGMENTS { break; }
                let a = w[0];
                let b = w[1];
                self.segments[count as usize] = Vec4::new(a.x, a.z, b.x, b.z);
                count += 1;
            }
            if count as usize >= MAX_PATH_SEGMENTS { break; }
        }
        for i in count as usize..MAX_PATH_SEGMENTS {
            self.segments[i] = Vec4::ZERO;
        }
        self.flags = UVec4::new(self.flags.x, self.flags.y, count, 0);
    }
}

impl Default for PathBlendParams {
    fn default() -> Self {
        Self {
            fade_radius: 4.0,
            thickness_scale: 1.0,
            base_width: 0.0,
            min_blend: 0.0,
            max_blend: 1.0,
            near_metallic: 0.0,
            near_roughness: 0.5,
            flags: UVec4::new(0, 0, 0, 0),
            near_base_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            segments: [Vec4::ZERO; MAX_PATH_SEGMENTS],
        }
    }
}

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct PathBlendExt {
    #[uniform(100)]
    pub params: PathBlendParams,
    // Optional alternate albedo (base color) map for the "near" region.
    // When present, the shader blends toward this texture instead of a flat near_base_color.
    #[texture(101)]
    #[sampler(102)]
    pub near_albedo: Option<Handle<Image>>,
    // Optional alternate metallic-roughness map for the near region.
    // Expecting glTF convention: roughness in G, metallic in B.
    #[texture(103)]
    #[sampler(104)]
    pub near_metallic_roughness: Option<Handle<Image>>,
    // Optional ambient occlusion map for the near region.
    #[texture(105)]
    #[sampler(106)]
    pub near_ao: Option<Handle<Image>>,
}

impl Default for PathBlendExt {
    fn default() -> Self {
        PathBlendExt {
            params: PathBlendParams {
                fade_radius: 4.0,
                thickness_scale: 1.0,
                base_width: 0.0,
                min_blend: 0.0,
                max_blend: 1.0,
                near_metallic: 0.0,
                near_roughness: 0.5,
                flags: UVec4::new(0, 0, 0, 0),
                near_base_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                segments: [Vec4::ZERO; MAX_PATH_SEGMENTS],
            },
            near_albedo: None,
            near_metallic_roughness: None,
            near_ao: None,
        }
    }
}

impl MaterialExtension for PathBlendExt {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(PATH_BLEND_SHADER_HANDLE)
    }
}

pub type PathBlendMaterial = ExtendedMaterial<StandardMaterial, PathBlendExt>;

// Convenience: falloff mode constants for flags.x
pub mod falloff_mode {
    pub const SMOOTHSTEP: u32 = 0;
    pub const INVERSE_SQUARED: u32 = 1;
    pub const LINEAR: u32 = 2;
}

impl PathBlendParams {
    pub fn set_falloff_mode(&mut self, mode: u32) {
        self.flags = UVec4::new(mode, self.flags.y, self.flags.z, self.flags.w);
    }
    pub fn set_invert(&mut self, invert: bool) {
        self.flags = UVec4::new(self.flags.x, if invert { 1 } else { 0 }, self.flags.z, self.flags.w);
    }
    // flags.w bitmask:
    // bit0 = near albedo present, bit1 = near metallic-roughness present, bit2 = near AO present
    pub fn set_near_presence(&mut self, has_albedo: bool, has_mr: bool, has_ao: bool) {
        let mut bits: u32 = 0;
        if has_albedo { bits |= 1; }
        if has_mr { bits |= 1 << 1; }
        if has_ao { bits |= 1 << 2; }
        self.flags = UVec4::new(self.flags.x, self.flags.y, self.flags.z, bits);
    }
}

// Helper to build a PathBlendMaterial handle and set path segments
pub fn make_path_blend_material(
    materials: &mut Assets<PathBlendMaterial>,
    base: StandardMaterial,
    mut params: PathBlendParams,
    path_points_world_xz: &[Vec3],
    near_albedo: Option<Handle<Image>>,
    near_metallic_roughness: Option<Handle<Image>>,
    near_ao: Option<Handle<Image>>,
) -> Handle<PathBlendMaterial> {
    params.set_segments_from_points(path_points_world_xz);
    params.set_near_presence(near_albedo.is_some(), near_metallic_roughness.is_some(), near_ao.is_some());
    materials.add(PathBlendMaterial { base, extension: PathBlendExt { params, near_albedo, near_metallic_roughness, near_ao } })
}

// Simple plugin to register the material type
pub struct PathBlendPlugin;

impl Plugin for PathBlendPlugin {
    fn build(&self, app: &mut App) {
        // Register internal WGSL so it works regardless of external asset dirs
        load_internal_asset!(
            app,
            PATH_BLEND_SHADER_HANDLE,
            "../../assets/shaders/path_blend_ext.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins(bevy_pbr::MaterialPlugin::<PathBlendMaterial>::default());
        app.init_resource::<GroundPathMaterial>();
    }
}

// Static handle used to refer to the embedded WGSL shader
pub const PATH_BLEND_SHADER_HANDLE: bevy::prelude::Handle<Shader> = bevy::prelude::Handle::weak_from_u128(0xA5F1_3C8D_21B3_5E77);

// Resource to keep track of the ground PathBlend material
#[derive(Resource, Default, Clone)]
pub struct GroundPathMaterial(pub Option<Handle<PathBlendMaterial>>);
