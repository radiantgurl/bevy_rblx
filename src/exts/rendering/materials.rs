use crate::{enums::Material, instance::BasePartMembers};
use bevy::prelude::*;

pub fn default_material(material: Material, assets: &AssetServer) -> StandardMaterial {
    match material {
        Material::SmoothPlastic => StandardMaterial::default(),
        Material::Plastic => StandardMaterial {
            metallic_roughness_texture: Some(
                assets.load("assets/textures/plastic_roughness_metallic.png"),
            ),
            ..default()
        },
        Material::Neon => todo!(),
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

#[derive(Hash, Debug, Reflect, Clone)]
pub struct RenderMaterialData {
    pub rgba: [u8; 4],
}

fn assign_materials(
    parts: Query<(&BasePartMembers, Entity), Changed<BasePartMembers>>,
    mut c: Commands,
    assets: AssetServer,
) {
    for (part, entity) in parts {
        let mut material = default_material(part.material, &assets);
    }
}
