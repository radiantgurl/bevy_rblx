use std::{cell::UnsafeCell, hash::Hash, mem::ManuallyDrop};

use lazy_static::lazy_static;
use parking_lot::{RawRwLock, lock_api::RawRwLock as _};

union FastFlagInternalValue {
    string: ManuallyDrop<String>,
    boolean: bool,
    int: i64,
    uint: u64,
    float: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum FastFlagType {
    String,
    Boolean,
    Int,
    Uint,
    Float,
}

impl std::fmt::Display for FastFlagType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FastFlagType::String => f.write_str("String"),
            FastFlagType::Boolean => f.write_str("Boolean"),
            FastFlagType::Int => f.write_str("Int"),
            FastFlagType::Uint => f.write_str("Uint"),
            FastFlagType::Float => f.write_str("Float"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum FastFlagValue {
    String(String),
    Boolean(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
}

impl FastFlagValue {
    pub fn get_type(&self) -> FastFlagType {
        match self {
            FastFlagValue::String(_) => FastFlagType::String,
            FastFlagValue::Boolean(_) => FastFlagType::Boolean,
            FastFlagValue::Int(_) => FastFlagType::Int,
            FastFlagValue::Uint(_) => FastFlagType::Uint,
            FastFlagValue::Float(_) => FastFlagType::Float,
        }
    }
}

pub struct FastFlags {
    values: UnsafeCell<Vec<FastFlagInternalValue>>,
    values_rwlock: RawRwLock,
    types: Vec<FastFlagType>,
    names: Vec<&'static str>,
}

impl Default for FastFlags {
    fn default() -> Self {
        Self {
            values: Default::default(),
            values_rwlock: RawRwLock::INIT,
            types: Default::default(),
            names: Default::default(),
        }
    }
}

pub trait FastFlagKey: Sized + Clone + Copy + Hash {
    #[expect(private_bounds)]
    type Target: FastFlagAllowedType;
    const NAME: &'static str;

    fn default_value() -> Self::Target;

    #[doc(hidden)]
    fn fetch_internal_id() -> usize;
    #[doc(hidden)]
    unsafe fn set_internal_id(id: usize);
}

pub struct FastFlagKeyInserter(FastFlags, usize);
#[derive(Clone, Copy)]
pub struct FastFlagKeyInsert(pub fn(&mut FastFlagKeyInserter));

#[diagnostic::on_unimplemented(
    message = "{Self} is not a valid type for FastFlagKey",
    label = "{Self} is not allowed here",
    note = "only bool, u64, i64, f64 and String are valid types"
)]
trait FastFlagAllowedType: Sized {
    const TYPE: FastFlagType;

    fn create(self) -> FastFlagInternalValue;
    unsafe fn replace_fastflag(self, value: &mut FastFlagInternalValue) {
        *value = self.create();
    }
    unsafe fn fetch(data: &FastFlagInternalValue) -> Self;
}

impl FastFlagAllowedType for String {
    const TYPE: FastFlagType = FastFlagType::String;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue {
            string: ManuallyDrop::new(self),
        }
    }

    unsafe fn replace_fastflag(self, value: &mut FastFlagInternalValue) {
        unsafe { ManuallyDrop::drop(&mut value.string) };
        *value = FastFlagInternalValue {
            string: ManuallyDrop::new(self),
        }
    }

    unsafe fn fetch(data: &FastFlagInternalValue) -> Self {
        unsafe { (*data.string).clone() }
    }
}
impl FastFlagAllowedType for u64 {
    const TYPE: FastFlagType = FastFlagType::Uint;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue { uint: self }
    }

    unsafe fn fetch(data: &FastFlagInternalValue) -> Self {
        unsafe { data.uint }
    }
}
impl FastFlagAllowedType for i64 {
    const TYPE: FastFlagType = FastFlagType::Int;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue { int: self }
    }

    unsafe fn fetch(data: &FastFlagInternalValue) -> Self {
        unsafe { data.int }
    }
}
impl FastFlagAllowedType for bool {
    const TYPE: FastFlagType = FastFlagType::Boolean;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue { boolean: self }
    }

    unsafe fn fetch(data: &FastFlagInternalValue) -> Self {
        unsafe { data.boolean }
    }
}
impl FastFlagAllowedType for f64 {
    const TYPE: FastFlagType = FastFlagType::Float;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue { float: self }
    }

    unsafe fn fetch(data: &FastFlagInternalValue) -> Self {
        unsafe { data.float }
    }
}

impl FastFlagKeyInserter {
    pub fn insert_key<T: FastFlagKey>(&mut self) -> &mut Self {
        self.0.types.push(T::Target::TYPE);
        self.0.values.get_mut().push(T::default_value().create());
        self.0.names.push(T::NAME);
        self.1 += 1;
        self
    }
}

inventory::collect!(FastFlagKeyInsert);

lazy_static! {
    pub static ref FAST_FLAGS: FastFlags = {
        let mut fastflags_inserter = FastFlagKeyInserter(FastFlags::default(), 0);

        for i in inventory::iter::<FastFlagKeyInsert>() {
            i.0(&mut fastflags_inserter);
        }

        fastflags_inserter.0
    };
}

unsafe impl Send for FastFlags {}
unsafe impl Sync for FastFlags {}

impl FastFlags {
    pub fn fetch<T: FastFlagKey>(&self) -> T::Target {
        self.values_rwlock.lock_shared();

        let data = unsafe {
            let ff_value = &(&*self.values.get().cast_const())[T::fetch_internal_id()];
            T::Target::fetch(ff_value)
        };

        unsafe {
            self.values_rwlock.unlock_shared();
        }
        data
    }
    pub fn store<T: FastFlagKey>(&self, value: T::Target) {
        self.values_rwlock.lock_exclusive();

        unsafe {
            let ff_value = &mut (&mut *self.values.get())[T::fetch_internal_id()];
            value.replace_fastflag(ff_value);
        };

        unsafe {
            self.values_rwlock.unlock_exclusive();
        }
    }

    pub fn names_and_types(&self) -> impl Iterator<Item = (&'static str, FastFlagType)> {
        self.names.iter().copied().zip(self.types.iter().copied())
    }

    pub fn fetch_dyn(&self, key: impl std::fmt::Display) -> Option<FastFlagValue> {
        let str_name = key.to_string();
        let i = self
            .names
            .iter()
            .enumerate()
            .find(|(_, x)| **x == &str_name)
            .map(|(i, _)| i);
        if let Some(idx) = i {
            let ff_type = self.types[idx];
            self.values_rwlock.lock_shared();
            let data = unsafe {
                let ff_value = &(&*self.values.get().cast_const())[idx];

                match ff_type {
                    FastFlagType::String => {
                        FastFlagValue::String(ManuallyDrop::into_inner(ff_value.string.clone()))
                    }
                    FastFlagType::Boolean => FastFlagValue::Boolean(ff_value.boolean),
                    FastFlagType::Int => FastFlagValue::Int(ff_value.int),
                    FastFlagType::Uint => FastFlagValue::Uint(ff_value.uint),
                    FastFlagType::Float => FastFlagValue::Float(ff_value.float),
                }
            };
            unsafe {
                self.values_rwlock.unlock_shared();
            }

            Some(data)
        } else {
            None
        }
    }
    pub fn store_dyn(&self, key: impl std::fmt::Display, value: FastFlagValue) {
        let str_name = key.to_string();
        let idx = self
            .names
            .iter()
            .enumerate()
            .find(|(_, x)| **x == &str_name)
            .map(|(i, _)| i)
            .expect("fast flag key does not exist");
        let ff_type = self.types[idx];
        if ff_type != value.get_type() {
            panic!(
                "invalid type for {key}: expected {ff_type} got {}",
                value.get_type()
            );
        }

        self.values_rwlock.lock_exclusive();
        unsafe {
            let ff_value = &mut (&mut *self.values.get())[idx];

            *ff_value = match value {
                FastFlagValue::String(s) => FastFlagInternalValue {
                    string: ManuallyDrop::new(s),
                },
                FastFlagValue::Boolean(b) => FastFlagInternalValue { boolean: b },
                FastFlagValue::Int(i) => FastFlagInternalValue { int: i },
                FastFlagValue::Uint(u) => FastFlagInternalValue { uint: u },
                FastFlagValue::Float(f) => FastFlagInternalValue { float: f },
            };
            self.values_rwlock.unlock_exclusive();
        };
    }
}
