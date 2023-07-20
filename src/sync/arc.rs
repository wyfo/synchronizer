use std::{any::Any, fmt, mem, mem::ManuallyDrop, ops::Deref};

use crate::synchronizer::synchronized;

/// [`std::sync::Arc`] mock.
#[derive(Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Arc<T: ?Sized>(ManuallyDrop<std::sync::Arc<T>>);

/// [`std::sync::Weak`] mock.
#[derive(Default)]
#[repr(transparent)]
pub struct Weak<T: ?Sized>(ManuallyDrop<std::sync::Weak<T>>);

impl<T> Arc<T> {
    /// [`std::sync::Arc::new`] mock.
    #[inline]
    pub fn new(data: T) -> Self {
        Self(ManuallyDrop::new(std::sync::Arc::new(data)))
    }

    /// [`std::sync::Arc::new`] mock.
    #[inline]
    pub fn new_cyclic<F: FnOnce(&Weak<T>) -> T>(data_fn: F) -> Self {
        Self(ManuallyDrop::new(std::sync::Arc::new_cyclic(move |weak| {
            // SAFETY: `Weak` is `#[repr(transparent)]`
            data_fn(unsafe { *(weak as *const _ as *const _) })
        })))
    }

    /// [`std::sync::Arc::try_unwrap`] mock.
    #[inline]
    #[track_caller]
    pub fn try_unwrap(this: Self) -> Result<T, Self> {
        // SAFETY: `Arc` is `#[repr(transparent)]`
        let arc = unsafe { mem::transmute(this) };
        synchronized(std::sync::Arc::try_unwrap(arc))
            .map_err(ManuallyDrop::new)
            .map_err(Self)
    }

    /// [`std::sync::Arc::into_inner`] mock.
    #[inline]
    #[track_caller]
    pub fn into_inner(this: Self) -> Option<T> {
        // SAFETY: `Arc` is `#[repr(transparent)]`
        let arc = unsafe { mem::transmute(this) };
        synchronized(std::sync::Arc::into_inner(ManuallyDrop::into_inner(arc)))
    }
}

impl<T: ?Sized> Arc<T> {
    /// [`std::sync::Arc::into_inner`] mock.
    #[inline]
    pub fn into_raw(this: Self) -> *const T {
        let ptr = Self::as_ptr(&this);
        mem::forget(this);
        ptr
    }

    /// [`std::sync::Arc::as_ptr`] mock.
    #[inline]
    pub fn as_ptr(this: &Self) -> *const T {
        std::sync::Arc::as_ptr(&this.0)
    }

    /// [`std::sync::Arc::from_raw`] mock.
    ///
    /// # Safety
    /// see [`std::sync::Arc::from_raw`]
    #[inline]
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        Self(ManuallyDrop::new(std::sync::Arc::from_raw(ptr)))
    }

    /// [`std::sync::Arc::downgrade`] mock.
    #[inline]
    #[track_caller]
    pub fn downgrade(this: &Self) -> Weak<T> {
        synchronized(Weak(ManuallyDrop::new(std::sync::Arc::downgrade(&this.0))))
    }

    /// [`std::sync::Arc::weak_count`] mock.
    #[inline]
    #[track_caller]
    pub fn weak_count(this: &Self) -> usize {
        synchronized(std::sync::Arc::weak_count(&this.0))
    }

    /// [`std::sync::Arc::strong_count`] mock.
    #[inline]
    #[track_caller]
    pub fn strong_count(this: &Self) -> usize {
        synchronized(std::sync::Arc::strong_count(&this.0))
    }

    /// [`std::sync::Arc::increment_strong_count`] mock.
    ///
    /// # Safety
    /// see [`std::sync::Arc::decrement_strong_count`]
    #[inline]
    #[track_caller]
    pub unsafe fn increment_strong_count(ptr: *const T) {
        std::sync::Arc::increment_strong_count(ptr);
        synchronized(());
    }

    /// [`std::sync::Arc::decrement_strong_count`] mock.
    ///
    /// # Safety
    /// see [`std::sync::Arc::decrement_strong_count`]
    #[inline]
    #[track_caller]
    pub unsafe fn decrement_strong_count(ptr: *const T) {
        std::sync::Arc::decrement_strong_count(ptr);
        synchronized(());
    }

    /// [`std::sync::Arc::ptr_eq`] mock.
    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&this.0, &other.0)
    }
}

impl<T: ?Sized> Clone for Arc<T> {
    #[inline]
    #[track_caller]
    fn clone(&self) -> Self {
        Self(synchronized(self.0.clone()))
    }
}

impl<T: ?Sized> Deref for Arc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> Arc<T> {
    /// [`std::sync::Arc::make_mut`] mock.
    #[inline]
    pub fn make_mut(this: &mut Self) -> &mut T {
        std::sync::Arc::make_mut(&mut this.0)
    }
}

impl<T: ?Sized> Arc<T> {
    /// [`std::sync::Arc::get_mut`] mock.
    #[inline]
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        std::sync::Arc::get_mut(&mut this.0)
    }
}

impl<T: ?Sized> Drop for Arc<T> {
    #[inline]
    #[track_caller]
    fn drop(&mut self) {
        // SAFETY: `self.0` is no more used
        unsafe { ManuallyDrop::drop(&mut self.0) }
        synchronized(());
    }
}

impl Arc<dyn Any + Send + Sync> {
    #[inline]
    /// [`std::sync::Arc::downcast`] mock.
    pub fn downcast<T: Any + Send + Sync>(self) -> Result<Arc<T>, Self> {
        // SAFETY: Arc is `#[repr(transparent)]`
        let arc: std::sync::Arc<dyn Any + Send + Sync> = unsafe { mem::transmute(self) };
        arc.downcast()
            .map(ManuallyDrop::new)
            .map(Arc)
            .map_err(ManuallyDrop::new)
            .map_err(Self)
    }
}

impl<T: ?Sized, U> From<U> for Arc<T>
where
    std::sync::Arc<T>: From<U>,
{
    fn from(value: U) -> Self {
        Self(ManuallyDrop::new(value.into()))
    }
}

impl<T> FromIterator<T> for Arc<[T]> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(ManuallyDrop::new(std::sync::Arc::from_iter(iter)))
    }
}

impl<T: ?Sized> fmt::Pointer for Arc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(self.0.deref(), f)
    }
}

impl<T, const N: usize> TryFrom<Arc<[T]>> for Arc<[T; N]> {
    type Error = Arc<[T]>;

    #[inline]
    fn try_from(boxed_slice: Arc<[T]>) -> Result<Self, Self::Error> {
        // SAFETY: `Arc` is `#[repr(transparent)]`
        unsafe { mem::transmute::<_, std::sync::Arc<[T]>>(boxed_slice).try_into() }
            .map(ManuallyDrop::new)
            .map(Arc)
            .map_err(ManuallyDrop::new)
            .map_err(Arc)
    }
}

impl<T> Weak<T> {
    /// [`std::sync::Weak::new`] mock.
    #[inline]
    pub fn new() -> Self {
        Self(ManuallyDrop::new(std::sync::Weak::new()))
    }
}

impl<T: ?Sized> Weak<T> {
    /// [`std::sync::Weak::as_ptr`] mock.
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }

    /// [`std::sync::Weak::into_raw`] mock.
    #[inline]
    pub fn into_raw(self) -> *const T {
        let ptr = self.as_ptr();
        mem::forget(self);
        ptr
    }
    /// [`std::sync::Weak::from_raw`] mock.
    ///
    /// # Safety
    /// see [`std::sync::Weak::from_raw`]
    #[inline]
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        Self(ManuallyDrop::new(std::sync::Weak::from_raw(ptr)))
    }

    /// [`std::sync::Weak::upgrade`] mock.
    pub fn upgrade(&self) -> Option<Arc<T>> {
        Some(Arc(ManuallyDrop::new(self.0.upgrade()?)))
    }

    /// [`std::sync::Weak::strong_count`] mock.
    #[inline]
    #[track_caller]
    pub fn strong_count(&self) -> usize {
        synchronized(self.0.strong_count())
    }

    /// [`std::sync::Weak::weak_count`] mock.
    #[inline]
    #[track_caller]
    pub fn weak_count(&self) -> usize {
        synchronized(self.0.weak_count())
    }

    /// [`std::sync::Weak::ptr_eq`] mock.
    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&this.0, &other.0)
    }
}

impl<T: ?Sized> Clone for Weak<T> {
    #[inline]
    #[track_caller]
    fn clone(&self) -> Self {
        Self(synchronized(self.0.clone()))
    }
}

impl<T: ?Sized> Drop for Weak<T> {
    #[inline]
    #[track_caller]
    fn drop(&mut self) {
        // SAFETY: `self.0` is no more used
        unsafe { ManuallyDrop::drop(&mut self.0) }
        synchronized(());
    }
}
