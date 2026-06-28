use std::ops::{Deref, DerefMut};

#[repr(transparent)]
#[derive(Clone, Copy, Eq, Ord, Hash, Debug)]
pub struct Takeable<T> {
    inner: Option<T>,
}

impl<T: PartialEq> PartialEq for Takeable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}
impl<T: PartialEq> PartialEq<T> for Takeable<T> {
    fn eq(&self, other: &T) -> bool {
        self.deref().eq(other)
    }
}
impl<T: PartialOrd> PartialOrd for Takeable<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}
impl<T: PartialOrd> PartialOrd<T> for Takeable<T> {
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.deref().partial_cmp(other)
    }
}

impl<T: Default> Default for Takeable<T> {
    fn default() -> Self {
        Self {
            inner: Some(T::default()),
        }
    }
}

impl<T> Deref for Takeable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("value already taken")
    }
}
impl<T> DerefMut for Takeable<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("value already taken")
    }
}

impl<T> Takeable<T> {
    #[inline]
    pub const fn take(&mut self) -> T {
        self.inner.take().expect("value already taken")
    }
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { inner: Some(value) }
    }
}
