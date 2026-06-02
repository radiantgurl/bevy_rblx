use crate::{
    core::lua::{DynamicLuaVec, LuaSingleton},
    enums::{Material, RaycastFilterType},
    internal_prelude::*,
    userdata::{ObjectRef, ObjectRefCollectionExt, Vector3},
};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

#[derive(Clone, Copy, PartialEq, Reflect, FromLua, Default, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn unit(mut self) -> Self {
        self.direction = (self.direction - self.origin).normalize() + self.origin;
        self
    }
    pub fn closest_point(self, point: Vec3) -> Vec3 {
        self.origin + (point - self.origin).dot(self.direction).min(0f32) * self.direction
    }
    pub fn distance(self, point: Vec3) -> f32 {
        (self.origin + (point - self.origin).dot(self.direction).min(0f32) * self.direction)
            .distance(point)
    }
}

impl LuaUserData for Ray {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Ray");
        fields.add_field_method_get("Unit", |_, this| Ok(this.unit()));
        fields.add_field_method_get("Origin", |_, this| Ok(Vector3::from(this.origin)));
        fields.add_field_method_get("Direction", |_, this| Ok(Vector3::from(this.direction)));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("ClosestPoint", |_, this, p: Vector3| {
            Ok(Vector3::from(this.closest_point(p.into())))
        });
        methods.add_method(
            "Distance",
            |_, this, p: Vector3| Ok(this.distance(p.into())),
        );
    }
}

#[register]
impl LuaSingleton for Ray {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let ray = lua.create_table()?;
        ray.raw_set(
            "new",
            lua.create_function(|_, (o, d): (Vector3, Vector3)| {
                Ok(Ray {
                    origin: o.into(),
                    direction: d.into(),
                })
            })?,
        )?;
        ray.set_readonly(true);
        lua.globals().raw_set("Ray", ray)?;
        Ok(())
    }
}

#[derive(Clone, Debug, FromLua)]
pub struct RaycastParams {
    pub exclude_instances: Option<LuaTable>,
    pub include_instances: Option<LuaTable>,
    pub ignore_water: bool,
    pub collision_group: String,
    pub respect_can_collide: bool,
    pub brute_force_all_slow: bool,
    pub filter_descendants_instances: LuaTable,
    pub filter_type: RaycastFilterType,
}

impl LuaUserData for RaycastParams {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("ExcludeInstances", |_, t| Ok(t.exclude_instances.clone()));
        fields.add_field_method_set("ExcludeInstances", |_, t, v: Option<LuaTable>| {
            t.exclude_instances = v;
            Ok(())
        });
        fields.add_field_method_get("IncludeInstances", |_, t| Ok(t.include_instances.clone()));
        fields.add_field_method_set("IncludeInstances", |_, t, v: Option<LuaTable>| {
            t.include_instances = v;
            Ok(())
        });
        fields.add_field_method_get("IgnoreWater", |_, t| Ok(t.ignore_water));
        fields.add_field_method_set("IgnoreWater", |_, t, v: bool| {
            t.ignore_water = v;
            Ok(())
        });
        fields.add_field_method_get("RespectCanCollide", |_, t| Ok(t.respect_can_collide));
        fields.add_field_method_set("RespectCanCollide", |_, t, v: bool| {
            t.respect_can_collide = v;
            Ok(())
        });
        fields.add_field_method_get("CollisionGroup", |_, t| Ok(t.collision_group.clone()));
        fields.add_field_method_set("CollisionGroup", |_, t, v: String| {
            t.collision_group = v;
            Ok(())
        });
        fields.add_field_method_get("FilterDescendantsInstances", |_, t| {
            Ok(t.filter_descendants_instances.clone())
        });
        fields.add_field_method_set("FilterDescendantsInstances", |_, t, v: LuaTable| {
            t.filter_descendants_instances = v;
            Ok(())
        });
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("AddToFilter", |lua, t, v: LuaValue| {
            let self_table = &t.filter_descendants_instances;
            if v.is_table() {
                let table = v.as_table().unwrap();
                for i in 1..=table.raw_len() {
                    self_table.raw_push(table.raw_get::<LuaValue>(i)?)?;
                }
                Ok(())
            } else {
                self_table.raw_push(ObjectRef::from_lua(v, lua)?)?;
                Ok(())
            }
        });
    }
}

impl RaycastParams {
    pub fn as_send(&self) -> LuaResult<LuaSendRaycastParams> {
        Ok(LuaSendRaycastParams {
            exclude_instances: if let Some(t) = self.exclude_instances.as_ref() {
                Some(DynamicLuaVec::<ObjectRef>::static_from_lua(t)?)
            } else {
                None
            },
            include_instances: if let Some(t) = self.include_instances.as_ref() {
                Some(DynamicLuaVec::<ObjectRef>::static_from_lua(t)?)
            } else {
                None
            },
            ignore_water: self.ignore_water,
            collision_group: self.collision_group.clone(),
            respect_can_collide: self.respect_can_collide,
            brute_force_all_slow: self.brute_force_all_slow,
            filter_descendants_instances: DynamicLuaVec::<ObjectRef>::static_from_lua(
                &self.filter_descendants_instances,
            )?,
            filter_type: self.filter_type,
        })
    }
}

#[derive(Clone, Reflect, Debug)]
pub struct LuaSendRaycastParams {
    pub exclude_instances: Option<Vec<ObjectRef>>,
    pub include_instances: Option<Vec<ObjectRef>>,
    pub ignore_water: bool,
    pub collision_group: String,
    pub respect_can_collide: bool,
    pub brute_force_all_slow: bool,
    pub filter_descendants_instances: Vec<ObjectRef>,
    pub filter_type: RaycastFilterType,
}

impl Default for LuaSendRaycastParams {
    fn default() -> Self {
        Self {
            exclude_instances: Default::default(),
            include_instances: Default::default(),
            ignore_water: Default::default(),
            collision_group: Default::default(),
            respect_can_collide: Default::default(),
            brute_force_all_slow: Default::default(),
            filter_descendants_instances: Default::default(),
            filter_type: RaycastFilterType::Exclude,
        }
    }
}

impl FromLua for LuaSendRaycastParams {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        RaycastParams::from_lua(value, lua)?.as_send()
    }
}
impl IntoLua for LuaSendRaycastParams {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let LuaSendRaycastParams {
            exclude_instances,
            include_instances,
            ignore_water,
            collision_group,
            respect_can_collide,
            brute_force_all_slow,
            mut filter_descendants_instances,
            filter_type,
        } = self;
        RaycastParams {
            exclude_instances: if let Some(mut t) = exclude_instances {
                t.update_lua_origin(lua);
                Some(DynamicLuaVec::static_into_lua(t, lua)?)
            } else {
                None
            },
            include_instances: if let Some(mut t) = include_instances {
                t.update_lua_origin(lua);
                Some(DynamicLuaVec::static_into_lua(t, lua)?)
            } else {
                None
            },
            ignore_water,
            collision_group,
            respect_can_collide,
            brute_force_all_slow,
            filter_descendants_instances: {
                filter_descendants_instances.update_lua_origin(lua);
                DynamicLuaVec::static_into_lua(filter_descendants_instances, lua)?
            },
            filter_type,
        }
        .into_lua(lua)
    }
}

#[derive(Clone, Reflect, Debug, FromLua)]
pub struct RaycastResult {
    pub distance: f64,
    pub instance: ObjectRef,
    pub material: Material,
    pub position: Vec3,
    pub normal: Vec3,
}

impl LuaUserData for RaycastResult {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Distance", |_, t| Ok(t.distance));
        fields.add_field_method_get("Instance", |lua, t| Ok(t.instance.clone_lua(lua)));
        fields.add_field_method_get("Material", |_, t| Ok(t.material));
        fields.add_field_method_get("Position", |_, t| Ok(Into::<Vector3>::into(t.position)));
        fields.add_field_method_get("Normal", |_, t| Ok(Into::<Vector3>::into(t.normal)));
    }
}

impl RaycastResult {
    pub fn clone_lua(&self, lua: &Lua) -> Self {
        Self {
            distance: self.distance,
            instance: self.instance.clone_lua(lua),
            material: self.material,
            position: self.position,
            normal: self.normal,
        }
    }
}

#[register]
impl LuaSingleton for RaycastParams {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let table = lua.create_table()?;
        table.raw_set(
            "new",
            lua.create_function(|l, ()| LuaSendRaycastParams::default().into_lua(l))?,
        )?;
        table.set_readonly(true);
        lua.globals().raw_set("RaycastParams", table)
    }
}
