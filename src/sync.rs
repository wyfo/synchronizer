use std::{
    ops::{Deref, DerefMut},
    time::{Duration, Instant},
};

use crate::synchronizer::{is_synchronized, synchronize};

#[cfg(feature = "arc")]
mod arc;
/// [`std::sync::atomic`] mock.
pub mod atomic;

#[cfg(not(feature = "arc"))]
pub use std::sync::{Arc, Weak};
pub use std::sync::{LockResult, OnceState, PoisonError, TryLockError, TryLockResult};

#[cfg(feature = "arc")]
pub use arc::{Arc, Weak};

use crate::{runtime::Address, synchronizer::synchronized};

/// [`std::sync::Barrier`] mock.
#[derive(Debug)]
pub struct Barrier {
    barrier: std::sync::Barrier,
    capa: usize,
}

/// [`std::sync::BarrierWaitResult`] mock.
#[derive(Debug)]
pub struct BarrierWaitResult(bool);

impl Barrier {
    /// [`std::sync::Barrier::new`] mock.
    pub fn new(n: usize) -> Self {
        Self {
            barrier: std::sync::Barrier::new(n),
            capa: n,
        }
    }

    /// [`std::sync::Barrier::wait`] mock.
    #[track_caller]
    pub fn wait(&self) -> BarrierWaitResult {
        let mut leader = false;
        if !synchronize(|rt, loc| rt.wait_barrier(self.into(), self.capa, &mut leader, loc)) {
            return BarrierWaitResult(self.barrier.wait().is_leader());
        }
        BarrierWaitResult(leader)
    }
}

impl BarrierWaitResult {
    /// [`std::sync::BarrierWaitResult::is_leader`] mock.
    pub fn is_leader(&self) -> bool {
        self.0
    }
}

/// [`std::sync::Condvar`] mock.
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct CondVar(std::sync::Condvar);

/// [`std::sync::WaitTimeoutResult`] mock.
#[derive(Debug)]
pub struct WaitTimeoutResult(bool);

impl CondVar {
    /// [`std::sync::Condvar::new`] mock.
    pub const fn new() -> Self {
        Self(std::sync::Condvar::new())
    }

    /// [`std::sync::Condvar::wait`] mock.
    #[track_caller]
    pub fn wait<'a, T>(&self, mut guard: MutexGuard<'a, T>) -> LockResult<MutexGuard<'a, T>> {
        let mutex = guard.mutex;
        let guard = guard.guard.take().unwrap();
        if is_synchronized() {
            drop(guard);
            synchronize(|rt, loc| rt.wait_condvar(self.into(), mutex.into(), loc));
            mutex.lock()
        } else {
            let to_guard = |guard| MutexGuard {
                mutex,
                guard: Some(guard),
            };
            self.0
                .wait(guard)
                .map(to_guard)
                .map_err(|err| PoisonError::new(to_guard(err.into_inner())))
        }
    }

    /// [`std::sync::Condvar::wait_while`] mock.
    #[track_caller]
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: MutexGuard<'a, T>,
        mut condition: F,
    ) -> LockResult<MutexGuard<'a, T>>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard)?;
        }
        Ok(guard)
    }

    /// [`std::sync::Condvar::wait_timeout`] mock.
    pub fn wait_timeout<'a, T>(
        &self,
        mut guard: MutexGuard<'a, T>,
        dur: Duration,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)> {
        let mutex = guard.mutex;
        let guard = guard.guard.take().unwrap();
        if is_synchronized() {
            drop(guard);
            synchronize(|rt, loc| rt.wait_condvar(self.into(), mutex.into(), loc));
            Ok((
                mutex.lock().map_err(|err| {
                    PoisonError::new((err.into_inner(), WaitTimeoutResult(false)))
                })?,
                WaitTimeoutResult(false),
            ))
        } else {
            let to_guard = |guard| MutexGuard {
                mutex,
                guard: Some(guard),
            };
            self.0
                .wait_timeout(guard, dur)
                .map(|(guard, timeout)| (to_guard(guard), WaitTimeoutResult(timeout.timed_out())))
                .map_err(PoisonError::into_inner)
                .map_err(|(guard, timeout)| {
                    PoisonError::new((to_guard(guard), WaitTimeoutResult(timeout.timed_out())))
                })
        }
    }

    /// [`std::sync::Condvar::wait_while_timeout`] mock.
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        mut guard: MutexGuard<'a, T>,
        dur: Duration,
        mut condition: F,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)>
    where
        F: FnMut(&mut T) -> bool,
    {
        let start = Instant::now();
        loop {
            if !condition(&mut *guard) {
                return Ok((guard, WaitTimeoutResult(false)));
            }
            let timeout = match dur.checked_sub(start.elapsed()) {
                Some(timeout) => timeout,
                None => return Ok((guard, WaitTimeoutResult(true))),
            };
            guard = self.wait_timeout(guard, timeout)?.0;
        }
    }

    /// [`std::sync::Condvar::notify_one`] mock.
    #[track_caller]
    pub fn notify_one(&self) {
        if !synchronize(|rt, loc| rt.notify_condvar(self.into(), false, loc)) {
            self.0.notify_one();
        }
    }

    /// [`std::sync::Condvar::notify_all`] mock.
    #[track_caller]
    pub fn notify_all(&self) {
        if !synchronize(|rt, loc| rt.notify_condvar(self.into(), true, loc)) {
            self.0.notify_all();
        }
    }
}

impl WaitTimeoutResult {
    /// [`std::sync::WaitTimeoutResult::timed_out`] mock.
    pub fn timed_out(&self) -> bool {
        self.0
    }
}

/// [`std::sync::Mutex`] mock.
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct Mutex<T: ?Sized>(std::sync::Mutex<T>);

/// [`std::sync::MutexGuard`] mock.
pub struct MutexGuard<'a, T: ?Sized> {
    mutex: &'a Mutex<T>,
    guard: Option<std::sync::MutexGuard<'a, T>>,
}

impl<T> Mutex<T> {
    /// [`std::sync::Mutex::new`] mock.
    pub const fn new(t: T) -> Self {
        Self(std::sync::Mutex::new(t))
    }
}

impl<T: ?Sized> Mutex<T> {
    /// [`std::sync::Mutex::lock`] mock.
    #[track_caller]
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        synchronize(|rt, loc| rt.acquire_mutex(self.into(), loc));
        let guard = Some(self.0.lock().map_err(|err| {
            PoisonError::new(MutexGuard {
                mutex: self,
                guard: Some(err.into_inner()),
            })
        })?);
        Ok(MutexGuard { mutex: self, guard })
    }

    /// [`std::sync::Mutex::try_lock`] mock.
    #[track_caller]
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        synchronize(|rt, loc| rt.try_acquire_mutex(self.into(), loc));
        let guard = Some(self.0.try_lock().map_err(|err| match err {
            TryLockError::Poisoned(err) => TryLockError::Poisoned(PoisonError::new(MutexGuard {
                mutex: self,
                guard: Some(err.into_inner()),
            })),
            TryLockError::WouldBlock => TryLockError::WouldBlock,
        })?);
        Ok(MutexGuard { mutex: self, guard })
    }

    /// [`std::sync::Mutex::is_poisoned`] mock.
    #[track_caller]
    pub fn is_poisoned(&self) -> bool {
        synchronized(self.0.is_poisoned())
    }

    /// [`std::sync::Mutex::into_inner`] mock.
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        self.0.into_inner()
    }

    /// [`std::sync::Mutex::get_mut`] mock.
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        self.0.get_mut()
    }
}

impl<T> From<T> for Mutex<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.as_deref().unwrap()
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_deref_mut().unwrap()
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    #[track_caller]
    fn drop(&mut self) {
        self.guard.take();
        synchronize(|rt, loc| rt.release_mutex(self.mutex.into(), loc));
    }
}

#[track_caller]
fn once<T>(addr: Address, f: impl FnOnce() -> T) -> T {
    struct OnceRelease(Address);
    impl Drop for OnceRelease {
        fn drop(&mut self) {
            synchronize(|rt, _| rt.release_once(self.0));
        }
    }
    synchronize(|rt, loc| rt.acquire_once(addr, loc));
    let _release = OnceRelease(addr);
    f()
}

/// [`std::sync::Once`] mock.
#[derive(Debug)]
#[repr(transparent)]
pub struct Once(std::sync::Once);

impl Once {
    /// [`std::sync::Once::new`] mock.
    pub const fn new() -> Self {
        Self(std::sync::Once::new())
    }

    /// [`std::sync::Once::call_once`] mock.
    #[track_caller]
    pub fn call_once<F: FnOnce()>(&self, f: F) {
        once(self.into(), || self.0.call_once(f));
    }

    /// [`std::sync::Once::call_once_force`] mock.
    #[track_caller]
    pub fn call_once_force<F: FnOnce(&OnceState)>(&self, f: F) {
        once(self.into(), || self.0.call_once_force(f));
    }

    /// [`std::sync::Once::is_completed`] mock.
    #[track_caller]
    pub fn is_completed(&self) -> bool {
        synchronized(self.0.is_completed())
    }
}

/// [`std::sync::OnceLock`] mock.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct OnceLock<T>(std::sync::OnceLock<T>);

impl<T> OnceLock<T> {
    /// [`std::sync::OnceLock::new`] mock.
    pub const fn new() -> Self {
        Self(std::sync::OnceLock::new())
    }

    /// [`std::sync::OnceLock::get`] mock.
    /// The mock cannot be const due to synchronization
    #[track_caller]
    pub fn get(&self) -> Option<&T> {
        synchronized(self.0.get())
    }

    /// [`std::sync::OnceLock::get_mut`] mock.
    #[track_caller]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.0.get_mut()
    }

    /// [`std::sync::OnceLock::set`] mock.
    #[track_caller]
    pub fn set(&self, value: T) -> Result<(), T> {
        once(self.into(), || self.0.set(value))
    }

    /// [`std::sync::OnceLock::get_or_init`] mock.
    #[track_caller]
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        once(self.into(), || self.0.get_or_init(f))
    }

    /// [`std::sync::OnceLock::into_inner`] mock.
    pub fn into_inner(self) -> Option<T> {
        self.0.into_inner()
    }

    /// [`std::sync::OnceLock::take`] mock.
    pub fn take(&mut self) -> Option<T> {
        self.0.take()
    }
}

impl<T> From<T> for OnceLock<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

/// [`std::sync::RwLock`] mock.
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct RwLock<T: ?Sized>(std::sync::RwLock<T>);

/// [`std::sync::RwLockReadGuard`] mock.
pub struct RwLockReadGuard<'a, T: ?Sized> {
    rwlock: &'a RwLock<T>,
    guard: Option<std::sync::RwLockReadGuard<'a, T>>,
}

/// [`std::sync::RwLockWriteGuard`] mock.
pub struct RwLockWriteGuard<'a, T: ?Sized> {
    rwlock: &'a RwLock<T>,
    guard: Option<std::sync::RwLockWriteGuard<'a, T>>,
}

impl<T> RwLock<T> {
    /// [`std::sync::Mutex::new`] mock.
    pub const fn new(t: T) -> Self {
        Self(std::sync::RwLock::new(t))
    }
}

impl<T: ?Sized> RwLock<T> {
    /// [`std::sync::RwLock::read`] mock.
    #[track_caller]
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        synchronize(|rt, loc| rt.acquire_read(self.into(), loc));
        let guard = Some(self.0.read().map_err(|err| {
            PoisonError::new(RwLockReadGuard {
                rwlock: self,
                guard: Some(err.into_inner()),
            })
        })?);
        Ok(RwLockReadGuard {
            rwlock: self,
            guard,
        })
    }

    /// [`std::sync::RwLock::try_read`] mock.
    #[track_caller]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        synchronize(|rt, loc| rt.try_acquire_read(self.into(), loc));
        let guard = Some(self.0.try_read().map_err(|err| match err {
            TryLockError::Poisoned(err) => {
                TryLockError::Poisoned(PoisonError::new(RwLockReadGuard {
                    rwlock: self,
                    guard: Some(err.into_inner()),
                }))
            }
            TryLockError::WouldBlock => TryLockError::WouldBlock,
        })?);
        Ok(RwLockReadGuard {
            rwlock: self,
            guard,
        })
    }

    /// [`std::sync::RwLock::write`] mock.
    #[track_caller]
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        synchronize(|rt, loc| rt.acquire_write(self.into(), loc));
        let guard = Some(self.0.write().map_err(|err| {
            PoisonError::new(RwLockWriteGuard {
                rwlock: self,
                guard: Some(err.into_inner()),
            })
        })?);
        Ok(RwLockWriteGuard {
            rwlock: self,
            guard,
        })
    }

    /// [`std::sync::RwLock::try_write`] mock.
    #[track_caller]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        synchronize(|rt, loc| rt.try_acquire_write(self.into(), loc));
        let guard = Some(self.0.try_write().map_err(|err| match err {
            TryLockError::Poisoned(err) => {
                TryLockError::Poisoned(PoisonError::new(RwLockWriteGuard {
                    rwlock: self,
                    guard: Some(err.into_inner()),
                }))
            }
            TryLockError::WouldBlock => TryLockError::WouldBlock,
        })?);
        Ok(RwLockWriteGuard {
            rwlock: self,
            guard,
        })
    }

    /// [`std::sync::Mutex::is_poisoned`] mock.
    #[track_caller]
    pub fn is_poisoned(&self) -> bool {
        synchronized(self.0.is_poisoned())
    }

    /// [`std::sync::Mutex::into_inner`] mock.
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        self.0.into_inner()
    }

    /// [`std::sync::Mutex::get_mut`] mock.
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        self.0.get_mut()
    }
}

impl<T> From<T> for RwLock<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.as_deref().unwrap()
    }
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.as_deref().unwrap()
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_deref_mut().unwrap()
    }
}

impl<'a, T: ?Sized> Drop for RwLockReadGuard<'a, T> {
    #[track_caller]
    fn drop(&mut self) {
        self.guard.take();
        synchronize(|rt, loc| rt.release_read(self.rwlock.into(), loc));
    }
}

impl<'a, T: ?Sized> Drop for RwLockWriteGuard<'a, T> {
    #[track_caller]
    fn drop(&mut self) {
        self.guard.take();
        synchronize(|rt, loc| rt.release_write(self.rwlock.into(), loc));
    }
}
