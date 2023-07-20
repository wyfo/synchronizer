pub use std::thread::{available_parallelism, panicking, AccessError, LocalKey, Result, ThreadId};
use std::time::Duration;

use crate::{
    runtime::Runtime,
    synchronizer::{synchronize, synchronized},
};

/// [`std::thread::Builder`] mock.
pub struct Builder(std::thread::Builder);
impl Builder {
    /// [`std::thread::Builder::new`] mock.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(std::thread::Builder::new())
    }

    /// [`std::thread::Builder::name`] mock.
    pub fn name(self, name: String) -> Self {
        Self(self.0.name(name))
    }

    /// [`std::thread::Builder::stack_size`] mock.
    pub fn stack_size(self, size: usize) -> Self {
        Self(self.0.stack_size(size))
    }

    /// [`std::thread::Builder::spawn`] mock.
    #[track_caller]
    pub fn spawn<F, T>(self, f: F) -> std::io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let (thread_id, handle) = crate::synchronizer::spawn(self.0, f);
        let handle = handle?;
        let thread = Thread {
            thread_id,
            thread: handle.thread().clone(),
        };
        Ok(JoinHandle {
            thread_id,
            handle,
            thread,
        })
    }
}

/// [`std::thread::spawn`] mock.
#[track_caller]
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f).expect("failed to spawn thread")
}

#[derive(Debug)]
/// [`std::thread::Thread`] mock.
pub struct Thread {
    thread_id: crate::runtime::ThreadId,
    thread: std::thread::Thread,
}

impl Thread {
    /// [`std::thread::Thread::unpark`] mock.
    #[track_caller]
    pub fn unpark(&self) {
        synchronize(|rt, loc| rt.unpark(self.thread_id, loc));
    }

    /// [`std::thread::Thread::id`] mock.
    #[must_use]
    pub fn id(&self) -> ThreadId {
        self.thread.id()
    }

    /// [`std::thread::Thread::name`] mock.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.thread.name()
    }
}

/// [`std::thread::JoinHandle`] mock.
#[derive(Debug)]
pub struct JoinHandle<T> {
    thread_id: crate::runtime::ThreadId,
    handle: std::thread::JoinHandle<T>,
    thread: Thread,
}

impl<T> JoinHandle<T> {
    /// [`std::thread::JoinHandle::thread`] mock.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    /// [`std::thread::JoinHandle::join`] mock.
    #[track_caller]
    pub fn join(self) -> Result<T> {
        synchronize(|rt, loc| rt.join(self.thread_id, loc));
        self.handle.join()
    }

    /// [`std::thread::JoinHandle::thread`] mock.
    #[track_caller]
    pub fn is_finished(&self) -> bool {
        synchronized(self.handle.is_finished())
    }
}

/// [`std::thread::Scope`] mock.
pub struct Scope<'scope, 'env>(std::thread::Scope<'scope, 'env>);

impl<'scope, 'env> Scope<'scope, 'env> {
    /// [`std::thread::Scope::spawn`] mock.
    pub fn spawn<F, T>(&'scope self, f: F) -> ScopedJoinHandle<'scope, T>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let (thread_id, handle) = crate::synchronizer::spawn_scoped(&self.0, f);
        let thread = Thread {
            thread_id,
            thread: handle.thread().clone(),
        };
        ScopedJoinHandle {
            thread_id,
            handle,
            thread,
        }
    }
}

/// [`std::thread::ScopedJoinHandle`] mock.
pub struct ScopedJoinHandle<'scope, T> {
    thread_id: crate::runtime::ThreadId,
    handle: std::thread::ScopedJoinHandle<'scope, T>,
    thread: Thread,
}

impl<'scope, T> ScopedJoinHandle<'scope, T> {
    /// [`std::thread::ScopedJoinHandle::thread`] mock.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    /// [`std::thread::ScopedJoinHandle::join`] mock.
    #[track_caller]
    pub fn join(self) -> Result<T> {
        synchronize(|rt, loc| rt.join(self.thread_id, loc));
        self.handle.join()
    }

    /// [`std::thread::ScopedJoinHandle::thread`] mock.
    #[track_caller]
    pub fn is_finished(&self) -> bool {
        synchronized(self.handle.is_finished())
    }
}

/// [`std::thread::park`] mock.
#[track_caller]
pub fn park() {
    if !synchronize(Runtime::park) {
        std::thread::park();
    }
}

/// [`std::thread::park_timeout`] mock.
#[track_caller]
pub fn park_timeout(dur: Duration) {
    if !synchronize(Runtime::park) {
        std::thread::park_timeout(dur);
    }
}

/// [`std::thread::sleep`] mock.
#[track_caller]
pub fn sleep(dur: Duration) {
    if !synchronize(Runtime::synchronized_operation) {
        std::thread::sleep(dur);
    }
}

/// [`std::thread::yield_now`] mock.
#[track_caller]
pub fn yield_now() {
    synchronized(());
}
