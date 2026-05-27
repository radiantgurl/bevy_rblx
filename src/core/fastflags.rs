use std::{
    hash::Hash,
    mem::ManuallyDrop,
    sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering::Relaxed},
};

use bevy::log::debug;
use lazy_static::lazy_static;
use parking_lot::Mutex;

union FastFlagInternalValue {
    string: ManuallyDrop<Mutex<String>>,
    boolean: ManuallyDrop<AtomicBool>,
    int: ManuallyDrop<AtomicI64>,
    uint: ManuallyDrop<AtomicU64>,
    float: ManuallyDrop<AtomicU64>, // F64
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
            FastFlagType::String => f.write_str("str"),
            FastFlagType::Boolean => f.write_str("bool"),
            FastFlagType::Int => f.write_str("int"),
            FastFlagType::Uint => f.write_str("uint"),
            FastFlagType::Float => f.write_str("float"),
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

impl std::fmt::Display for FastFlagValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FastFlagValue::String(s) => write!(f, "{s:#?}"),
            FastFlagValue::Boolean(b) => write!(f,"{b}"),
            FastFlagValue::Int(i) => write!(f,"{i}"),
            FastFlagValue::Uint(i) => write!(f,"{i}"),
            FastFlagValue::Float(n) => write!(f,"{n}"),
        }
    }
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
    values: Vec<FastFlagInternalValue>,
    types: Vec<FastFlagType>,
    names: Vec<&'static str>,
}

impl Default for FastFlags {
    fn default() -> Self {
        Self {
            values: Default::default(),
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
    unsafe fn replace_fastflag(self, value: &FastFlagInternalValue);
    unsafe fn fetch(value: &FastFlagInternalValue) -> Self;
}

impl FastFlagAllowedType for String {
    const TYPE: FastFlagType = FastFlagType::String;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue {
            string: ManuallyDrop::new(Mutex::new(self)),
        }
    }

    unsafe fn replace_fastflag(self, value: &FastFlagInternalValue) {
        let mut s = unsafe { value.string.lock() };
        *s = self;
    }

    unsafe fn fetch(value: &FastFlagInternalValue) -> Self {
        unsafe { value.string.lock().clone() }
    }
}
impl FastFlagAllowedType for u64 {
    const TYPE: FastFlagType = FastFlagType::Uint;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue {
            uint: ManuallyDrop::new(AtomicU64::new(self)),
        }
    }

    unsafe fn replace_fastflag(self, value: &FastFlagInternalValue) {
        unsafe { value.uint.store(self, Relaxed) }
    }

    unsafe fn fetch(value: &FastFlagInternalValue) -> Self {
        unsafe { value.uint.load(Relaxed) }
    }
}
impl FastFlagAllowedType for i64 {
    const TYPE: FastFlagType = FastFlagType::Int;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue {
            int: ManuallyDrop::new(AtomicI64::new(self)),
        }
    }

    unsafe fn fetch(value: &FastFlagInternalValue) -> Self {
        unsafe { value.int.load(Relaxed) }
    }

    unsafe fn replace_fastflag(self, value: &FastFlagInternalValue) {
        unsafe { value.int.store(self, Relaxed) }
    }
}
impl FastFlagAllowedType for bool {
    const TYPE: FastFlagType = FastFlagType::Boolean;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue {
            boolean: ManuallyDrop::new(AtomicBool::new(self)),
        }
    }

    unsafe fn fetch(value: &FastFlagInternalValue) -> Self {
        unsafe { value.boolean.load(Relaxed) }
    }

    unsafe fn replace_fastflag(self, value: &FastFlagInternalValue) {
        unsafe { value.boolean.store(self, Relaxed) }
    }
}
impl FastFlagAllowedType for f64 {
    const TYPE: FastFlagType = FastFlagType::Float;

    fn create(self) -> FastFlagInternalValue {
        FastFlagInternalValue {
            float: ManuallyDrop::new(AtomicU64::new(self.to_bits())),
        }
    }

    unsafe fn fetch(value: &FastFlagInternalValue) -> Self {
        unsafe { Self::from_bits(value.float.load(Relaxed)) }
    }

    unsafe fn replace_fastflag(self, value: &FastFlagInternalValue) {
        unsafe { value.float.store(self.to_bits(), Relaxed) }
    }
}

impl FastFlagKeyInserter {
    pub fn insert_key<T: FastFlagKey>(&mut self) -> &mut Self {
        unsafe { T::set_internal_id(self.1) }
        self.0.types.push(T::Target::TYPE);
        self.0.values.push(T::default_value().create());
        self.0.names.push(T::NAME);

        debug!(target: "bevy_rblx::fastflags", "Adding fast flag {} with type {} and internal id {}", T::NAME, T::Target::TYPE, self.1);
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
        let data = unsafe {
            let ff_value = &self.values[T::fetch_internal_id()];
            T::Target::fetch(ff_value)
        };

        data
    }
    pub fn store<T: FastFlagKey>(&self, value: T::Target) {
        let ff_value = &self.values[T::fetch_internal_id()];
        unsafe {
            value.replace_fastflag(ff_value);
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
            let data = unsafe {
                let ff_value = &self.values[idx];
                match ff_type {
                    FastFlagType::String => FastFlagValue::String(ff_value.string.lock().clone()),
                    FastFlagType::Boolean => FastFlagValue::Boolean(ff_value.boolean.load(Relaxed)),
                    FastFlagType::Int => FastFlagValue::Int(ff_value.int.load(Relaxed)),
                    FastFlagType::Uint => FastFlagValue::Uint(ff_value.uint.load(Relaxed)),
                    FastFlagType::Float => {
                        FastFlagValue::Float(f64::from_bits(ff_value.float.load(Relaxed)))
                    }
                }
            };

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

        unsafe {
            let ff_value = &self.values[idx];
            match value {
                FastFlagValue::String(s) => s.replace_fastflag(ff_value),
                FastFlagValue::Boolean(b) => b.replace_fastflag(ff_value),
                FastFlagValue::Int(i) => i.replace_fastflag(ff_value),
                FastFlagValue::Uint(u) => u.replace_fastflag(ff_value),
                FastFlagValue::Float(f) => f.replace_fastflag(ff_value),
            };
        };
    }
}
