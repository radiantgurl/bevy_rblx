use std::hash::Hash;

use crate::{enums::Material, instance::BasePartMembers};
use bevy::{platform::collections::HashMap, prelude::*};

pub fn default_material(material: Material, assets: &AssetServer) -> StandardMaterial {
    match material {
        Material::SmoothPlastic => StandardMaterial {
            perceptual_roughness: 0.0,
            ..default()
        },
        Material::Plastic => StandardMaterial {
            metallic_roughness_texture: Some(
                assets.load("assets/textures/plastic_roughness_metallic.png"),
            ),
            perceptual_roughness: 1.0,
            ..default()
        },
        Material::Neon => StandardMaterial {
            emissive: LinearRgba::new(2.0, 2.0, 2.0, 0.0),
            unlit: true,
            ..default()
        },
        Material::Wood => todo!(),
        Material::WoodPlanks => todo!(),
        Material::Marble => todo!(),
        Material::Basalt => todo!(),
        Material::Slate => todo!(),
        Material::CrackedLava => todo!(),
        Material::Concrete => todo!(),
        Material::Limestone => todo!(),
        Material::Granite => todo!(),
        Material::Pavement => todo!(),
        Material::Brick => todo!(),
        Material::Pebble => todo!(),
        Material::Cobblestone => todo!(),
        Material::Rock => todo!(),
        Material::Sandstone => todo!(),
        Material::CorrodedMetal => todo!(),
        Material::DiamondPlate => todo!(),
        Material::Foil => todo!(),
        Material::Metal => todo!(),
        Material::Grass => todo!(),
        Material::LeafyGrass => todo!(),
        Material::Sand => todo!(),
        Material::Fabric => todo!(),
        Material::Snow => todo!(),
        Material::Mud => todo!(),
        Material::Ground => todo!(),
        Material::Asphalt => todo!(),
        Material::Salt => todo!(),
        Material::Ice => todo!(),
        Material::Glacier => todo!(),
        Material::Glass => todo!(),
        Material::ForceField => todo!(),
        Material::Air => todo!(),
        Material::Water => todo!(),
        Material::Cardboard => todo!(),
        Material::Carpet => todo!(),
        Material::CeramicTiles => todo!(),
        Material::ClayRoofTiles => todo!(),
        Material::RoofShingles => todo!(),
        Material::Leather => todo!(),
        Material::Plaster => todo!(),
        Material::Rubber => todo!(),
    }
}
#[derive(Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Component, Copy, Reflect)]
pub(super) struct HashedMaterialData {
    pub color: [u8; 4],
    pub metallic: u8,
    pub material: Material,
    pub cast_shadow: bool,
}

pub fn assign_materials(
    parts: Query<
        (
            &BasePartMembers,
            Option<&mut MeshMaterial3d<StandardMaterial>>,
            Entity,
            Option<&mut HashedMaterialData>,
        ),
        Changed<BasePartMembers>,
    >,
    mut c: Commands,
    assets: Res<AssetServer>,

    // TODO: patch to use weak handles and remove reference counting system completely
    mut materials_and_refs: Local<HashMap<HashedMaterialData, (Handle<StandardMaterial>, u64)>>,
) {
    let mut decreased = Vec::<HashedMaterialData>::new();
    for (part, material, entity, old_mat) in parts {
        let srgb = Srgba::new(
            part.color.r,
            part.color.g,
            part.color.b,
            1.0 - part.transparency as f32,
        );
        let hashed_material = HashedMaterialData {
            color: srgb.to_u8_array(),
            metallic: (part.reflectance * 255.0).floor() as u8,
            material: part.material,
            cast_shadow: part.cast_shadow,
        };
        let handle_with_refcnt;
        if let Some(h) = materials_and_refs.get_mut(&hashed_material) {
            handle_with_refcnt = h;
        } else {
            let mut material = default_material(part.material, &assets);
            material.base_color = Color::Srgba(srgb);
            let emm = Srgba {
                red: material.emissive.red * srgb.red,
                green: material.emissive.green * srgb.green,
                blue: material.emissive.blue * srgb.blue,
                alpha: material.emissive.alpha * srgb.alpha,
            };
            material.emissive = Color::Srgba(emm).to_linear();
            material.alpha_mode = if srgb.alpha == 1.0 {
                AlphaMode::Opaque
            } else {
                AlphaMode::Blend
            };
            let handle = assets.add(material);
            handle_with_refcnt = materials_and_refs
                .entry(hashed_material)
                .or_insert((handle, 0));
        }
        handle_with_refcnt.1 += 1;
        if let Some(mut v) = old_mat {
            decreased.push(*v);
            *v = hashed_material;
        } else {
            c.entity(entity).insert(hashed_material);
        }
        if let Some(mut m) = material {
            m.0 = handle_with_refcnt.0.clone();
        } else {
            c.entity(entity)
                .insert(MeshMaterial3d(handle_with_refcnt.0.clone()));
        }
    }
    for i in decreased {
        let handle_with_ref = materials_and_refs.get_mut(&i).unwrap();
        handle_with_ref.1 -= 1;
        if handle_with_ref.1 == 0 {
            materials_and_refs.remove(&i);
        }
    }
}
