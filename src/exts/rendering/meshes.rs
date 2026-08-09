use crate::{
    core::FAST_FLAGS,
    enums::PartType,
    instance::{BasePartMembers, PartMembers},
    internal_prelude::*,
    userdata::Vector3,
};
use bevy::{platform::collections::HashMap, prelude::*};
use bevy_rblx_derive::fast_flag;

#[derive(Hash, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Reflect)]
#[repr(transparent)]
pub struct ScaleAxum(u32);

impl Into<ScaleAxum> for f32 {
    fn into(self) -> ScaleAxum {
        match self {
            ..=0.01 => ScaleAxum(1),
            x @ 0.01..=2048.0 => ScaleAxum((x * 100.0).floor() as u32),
            2048.0.. => ScaleAxum(204800),
            _ => unreachable!(),
        }
    }
}
impl Into<f32> for ScaleAxum {
    fn into(self) -> f32 {
        self.0 as f32 / 100.0
    }
}

impl Default for ScaleAxum {
    fn default() -> Self {
        Self(100)
    }
}

impl std::fmt::Debug for ScaleAxum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let float = self.0 as f32 / 100.0;
        f.debug_tuple("ScaleAxum").field(&float).finish()
    }
}

impl std::fmt::Display for ScaleAxum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0 as f32 / 100.0)
    }
}

#[derive(Hash, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub struct Scale {
    pub x: ScaleAxum,
    pub y: ScaleAxum,
    pub z: ScaleAxum,
}

impl Into<Scale> for Vector3 {
    fn into(self) -> Scale {
        <Vector3 as Into<Vec3>>::into(self).into()
    }
}

impl Into<Scale> for Vec3 {
    fn into(self) -> Scale {
        Scale {
            x: self.x.into(),
            y: self.y.into(),
            z: self.z.into(),
        }
    }
}

impl Into<Vec3> for Scale {
    fn into(self) -> Vec3 {
        Vec3 {
            x: self.x.into(),
            y: self.y.into(),
            z: self.z.into(),
        }
    }
}

impl std::fmt::Display for Scale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[derive(Hash, Clone, Copy, Component, PartialEq, Eq, Reflect)]
pub enum HashedMesh {
    Ball(Scale),
    Block(Scale),
    Cylinder(Scale),
    Wedge(Scale),
    CornerWedge(Scale),
    Mesh {
        scale: Scale,
        double_sided: bool,
        texture_id: u64,
        mesh_id: u64,
    },
}

pub fn assign_hash_meshes(
    pending_parts: Query<
        (
            &BasePartMembers,
            &PartMembers,
            Option<&mut HashedMesh>,
            Entity,
        ),
        Or<(Changed<PartMembers>, Changed<BasePartMembers>)>,
    >,
    // pending_mesh_parts: Query<&
    mut commands: Commands,
) {
    for (base_part, part, mesh, e) in pending_parts {
        let scale = <Vector3 as Into<Scale>>::into(base_part.size);
        let hashed_mesh = match part.part_type {
            PartType::Ball => HashedMesh::Ball(scale),
            PartType::Block => HashedMesh::Block(scale),
            PartType::Cylinder => HashedMesh::Cylinder(scale),
            PartType::Wedge => HashedMesh::Wedge(scale),
            PartType::CornerWedge => HashedMesh::CornerWedge(scale),
        };
        if let Some(mut m) = mesh {
            if *m != hashed_mesh {
                *m = hashed_mesh;
            }
        } else {
            commands.entity(e).insert(hashed_mesh);
        }
    }
}

fn wedge(x: f32, y: f32, z: f32) -> Mesh {
    todo!()
}
fn corner_wedge(x: f32, y: f32, z: f32) -> Mesh {
    todo!()
}

pub fn assign_meshes(
    pending_parts: Query<(Entity, &HashedMesh, Option<&mut Mesh3d>), Changed<HashedMesh>>,

    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut local_storage: Local<HashMap<HashedMesh, AssetId<Mesh>>>,
) {
    for (e, hashed_mesh, mesh3d) in pending_parts {
        let mesh = if let Some(handle) = local_storage
            .get(hashed_mesh)
            .and_then(|v| meshes.get_strong_handle(*v))
        {
            handle
        } else {
            let handle = match hashed_mesh {
                HashedMesh::Ball(scale) => meshes.add(Sphere::new(
                    <ScaleAxum as Into<f32>>::into(scale.x)
                        .min(scale.y.into())
                        .min(scale.z.into()),
                )),
                HashedMesh::Block(scale) => {
                    meshes.add(Cuboid::new(scale.x.into(), scale.y.into(), scale.z.into()))
                }
                HashedMesh::Cylinder(scale) => meshes.add(Cylinder::new(
                    <ScaleAxum as Into<f32>>::into(scale.x).min(scale.z.into()),
                    scale.y.into(),
                )),
                HashedMesh::Wedge(scale) => {
                    meshes.add(wedge(scale.x.into(), scale.y.into(), scale.z.into()))
                }
                HashedMesh::CornerWedge(scale) => {
                    meshes.add(corner_wedge(scale.x.into(), scale.y.into(), scale.z.into()))
                }
                HashedMesh::Mesh { .. } => todo!(),
            };
            local_storage.insert(*hashed_mesh, handle.id());
            if FAST_FLAGS.fetch::<FFEnableRaytracing>() {
                meshes.get_mut(handle.id()).unwrap().enable_raytracing = true;
            }
            handle
        };
        if let Some(mut m) = mesh3d {
            m.0 = mesh;
        } else {
            commands.entity(e).insert(Mesh3d(mesh));
        }
    }
}

fast_flag!(FFEnableRaytracing: bool = false);
