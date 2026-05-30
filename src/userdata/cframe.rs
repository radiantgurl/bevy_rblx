use crate::{
    core::lua::LuaSingleton, enums::RotationOrder, internal_prelude::*, userdata::Vector3,
};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

#[derive(Clone, Copy, FromLua, PartialEq, Debug, Default, Reflect)]
#[repr(transparent)]
pub struct CFrame(Transform);

impl From<Transform> for CFrame {
    fn from(value: Transform) -> Self {
        CFrame(value)
    }
}
impl Into<Transform> for CFrame {
    fn into(self) -> Transform {
        self.0
    }
}

impl LuaUserData for CFrame {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Position", |_lua, p| Ok(Vector3::from(p.0.translation)));
        fields.add_field_method_get("Rotation", |_lua, p| {
            Ok(CFrame(Transform {
                translation: Vec3::ZERO,
                rotation: p.0.rotation,
                scale: Vec3::ONE,
            }))
        });
        fields.add_field_method_get("X", |_lua, p| Ok(p.0.translation.x));
        fields.add_field_method_get("Y", |_lua, p| Ok(p.0.translation.y));
        fields.add_field_method_get("Z", |_lua, p| Ok(p.0.translation.z));
        fields.add_field_method_get("LookVector", |_lua, p| Ok(Vector3::from(*p.0.forward())));
        fields.add_field_method_get("RightVector", |_lua, p| Ok(Vector3::from(*p.0.right())));
        fields.add_field_method_get("UpVector", |_lua, p| Ok(Vector3::from(*p.0.up())));
        fields.add_field_method_get("XVector", |_lua, p| {
            let v = p.0.to_matrix().x_axis;
            Ok(Vector3::from(Vec3 {
                x: v.x,
                y: v.y,
                z: v.z,
            }))
        });
        fields.add_field_method_get("YVector", |_lua, p| {
            let v = p.0.to_matrix().y_axis;
            Ok(Vector3::from(Vec3 {
                x: v.x,
                y: v.y,
                z: v.z,
            }))
        });
        fields.add_field_method_get("ZVector", |_lua, p| {
            let v = p.0.to_matrix().z_axis;
            Ok(Vector3::from(Vec3 {
                x: v.x,
                y: v.y,
                z: v.z,
            }))
        });
        fields.add_meta_field("__type", "CFrame");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Inverse", |_lua, this, ()| {
            Ok(Self(Transform::from_matrix(this.0.to_matrix().inverse())))
        });
        methods.add_method("Lerp", |_lua, this, (o, a): (CFrame, f64)| {
            Ok(Self(Transform {
                translation: this.0.translation.lerp(o.0.translation, a as f32),
                rotation: this.0.rotation.lerp(o.0.rotation, a as f32),
                scale: this.0.scale.lerp(o.0.scale, a as f32),
            }))
        });
        methods.add_method("Orthonormalize", |_lua, this, ()| {
            Ok(*this) // already orthonormalized
        });
        methods.add_method("ToWorldSpace", |_lua, this, values: Vec<CFrame>| {
            let mut mat = this.0.to_matrix();
            for i in values.iter().skip(1) {
                mat = mat.mul_mat4(&i.0.to_matrix());
            }
            Ok(CFrame(Transform::from_matrix(mat)))
        });
        methods.add_method("ToObjectSpace", |_lua, this, values: Vec<CFrame>| {
            let mut mat = this.0.to_matrix().inverse();
            for i in values.iter().skip(1) {
                mat = mat.mul_mat4(&i.0.to_matrix());
            }
            Ok(CFrame(Transform::from_matrix(mat)))
        });
        methods.add_method("PointToWorldSpace", |_lua, this, values: Vec<Vector3>| {
            let mut vec: Vec<Vector3> = Vec::new();
            let mat = this.0.to_matrix();
            for i in values {
                vec.push(mat.transform_point3(i.into()).into());
            }
            Ok(vec)
        });
        methods.add_method("PointToObjectSpace", |_lua, this, values: Vec<Vector3>| {
            let mut vec: Vec<Vector3> = Vec::new();
            let mat = this.0.to_matrix().inverse();
            for i in values {
                vec.push(mat.transform_point3(i.into()).into());
            }
            Ok(vec)
        });
        methods.add_method("VectorToWorldSpace", |_lua, this, values: Vec<Vector3>| {
            let mut vec: Vec<Vector3> = Vec::new();
            let mat = this.0.to_matrix();
            for i in values {
                vec.push(mat.transform_vector3(i.into()).into());
            }
            Ok(vec)
        });
        methods.add_method("VectorToObjectSpace", |_lua, this, values: Vec<Vector3>| {
            let mut vec: Vec<Vector3> = Vec::new();
            let mat = this.0.to_matrix().inverse();
            for i in values {
                vec.push(mat.transform_vector3(i.into()).into());
            }
            Ok(vec)
        });
        methods.add_method("GetComponents", |_lua, this, ()| {
            let mat = this.0.compute_affine();
            Ok((
                mat.translation.x,
                mat.translation.y,
                mat.translation.z,
                mat.matrix3.x_axis.x,
                mat.matrix3.x_axis.y,
                mat.matrix3.x_axis.z,
                mat.matrix3.y_axis.x,
                mat.matrix3.y_axis.y,
                mat.matrix3.y_axis.z,
                mat.matrix3.z_axis.x,
                mat.matrix3.z_axis.y,
                mat.matrix3.z_axis.z,
            ))
        });
        methods.add_method("components", |_lua, this, ()| {
            let mat = this.0.compute_affine();
            Ok((
                mat.translation.x,
                mat.translation.y,
                mat.translation.z,
                mat.matrix3.x_axis.x,
                mat.matrix3.x_axis.y,
                mat.matrix3.x_axis.z,
                mat.matrix3.y_axis.x,
                mat.matrix3.y_axis.y,
                mat.matrix3.y_axis.z,
                mat.matrix3.z_axis.x,
                mat.matrix3.z_axis.y,
                mat.matrix3.z_axis.z,
            ))
        });
        methods.add_method(
            "ToEulerAngles",
            |_lua, this, (order,): (Option<RotationOrder>,)| {
                Ok(this
                    .0
                    .rotation
                    .to_euler(order.unwrap_or(RotationOrder::XYZ).into()))
            },
        );
        methods.add_method("ToEulerAnglesXYZ", |_lua, this, ()| {
            Ok(this.0.rotation.to_euler(RotationOrder::XYZ.into()))
        });
        methods.add_method("ToEulerAnglesYXZ", |_lua, this, ()| {
            Ok(this.0.rotation.to_euler(RotationOrder::YXZ.into()))
        });
        methods.add_method("ToOrientation", |_lua, this, ()| {
            Ok(this.0.rotation.to_euler(RotationOrder::YXZ.into()))
        });
        methods.add_method("ToAxisAngle", |_lua, _this, ()| -> LuaResult<()> {
            lua_todo!();
        });
        methods.add_method("FuzzyEq", |_lua, this, (o, e): (CFrame, f64)| {
            Ok(this.0.to_matrix().abs_diff_eq(o.0.to_matrix(), e as f32))
        });
        methods.add_method("AngleBetween", |_lua, this, (o,): (CFrame,)| {
            Ok(this.0.rotation.angle_between(o.0.rotation))
        });
        methods.add_meta_method("__add", |_lua, this, o: Vector3| {
            let mut cf = *this;
            cf.0.translation += Into::<Vec3>::into(o);
            Ok(cf)
        });
        methods.add_meta_method("__sub", |_lua, this, o: Vector3| {
            let mut cf = *this;
            cf.0.translation -= Into::<Vec3>::into(o);
            Ok(cf)
        });
        methods.add_meta_method("__mul", |lua, this, o: LuaAnyUserData| {
            if let Ok(ud) = o.borrow::<CFrame>() {
                Self(Transform::from_matrix(
                    this.0.to_matrix().mul_mat4(&ud.0.to_matrix()),
                ))
                .into_lua(lua)
            } else {
                let ud: LuaUserDataRef<Vector3> = o.borrow_typed()?;
                let v: Vec3 = (*ud).into();
                let v: Vector3 = this.0.to_matrix().transform_point3(v).into();
                v.into_lua(lua)
            }
        });
    }
}

impl CFrame {
    pub fn new(lua: &Lua, args: LuaMultiValue) -> LuaResult<Self> {
        match args.len() {
            0 => Ok(Default::default()),
            1 => {
                let pos: LuaUserDataRef<Vector3> = args[0].borrow_typed()?;
                Ok(Self(Transform::from_translation((*pos).into())))
            }
            2 => {
                let pos: LuaUserDataRef<Vector3> = args[0].borrow_typed()?;
                let looking_at: LuaUserDataRef<Vector3> = args[1].borrow_typed()?;
                Ok(Self(
                    Transform::from_translation((*pos).into())
                        .looking_at((*looking_at).into(), Vec3::ZERO),
                ))
            }
            3 => {
                let (x, y, z) = <(f32, f32, f32)>::from_lua_multi(args, lua)?;
                Ok(Self(Transform::from_translation(Vec3 { x, y, z })))
            }
            7 => {
                let (x, y, z, qx, qy, qz, qw) =
                    <(f32, f32, f32, f32, f32, f32, f32)>::from_lua_multi(args, lua)?;
                Ok(Self(
                    Transform::from_translation(Vec3 { x, y, z })
                        .with_rotation(Quat::from_xyzw(qx, qy, qz, qw)),
                ))
            }
            12 => {
                let v = <[f32; 12]>::from_lua_multi(args, lua)?;
                let mat = Mat4::from_mat3_translation(
                    Mat3::from_cols_slice(&v[3..]),
                    Vec3 {
                        x: v[0],
                        y: v[1],
                        z: v[2],
                    },
                );
                Ok(Self(Transform::from_matrix(mat)))
            }
            l => Err(LuaError::runtime(format!(
                "expected 0, 1, 2, 3, 7 or 12 arguments, got {l}"
            ))),
        }
    }
}

#[register]
impl LuaSingleton for CFrame {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let table = lua.create_table()?;
        table.raw_set("identity", CFrame(Transform::IDENTITY))?;
        table.raw_set("new", lua.create_function(CFrame::new)?)?;
        table.set_readonly(true);
        lua.globals().raw_set("CFrame", table)?;
        Ok(())
    }
}
