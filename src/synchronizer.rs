use std::{
    cell::RefCell,
    fmt::Debug,
    io::Write,
    panic,
    panic::{catch_unwind, RefUnwindSafe, UnwindSafe},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use crate::runtime::{Location, Runtime, ThreadId};

const PARK_SPIN: usize = 100;

thread_local! {
    static SYNCHRONIZER: RefCell<Option<Arc<Synchronizer>>> = Default::default();
}

struct Synchronizer {
    runtime: Mutex<Runtime>,
    running_thread: AtomicUsize,
}

impl Synchronizer {
    fn new() -> Self {
        Self {
            runtime: Mutex::new(Runtime::new()),
            running_thread: AtomicUsize::new(ThreadId::DUMMY.get()),
        }
    }

    fn run<R: TestResult>(&self, test: impl Fn() -> R + UnwindSafe) -> bool {
        if let Err(err) = catch_unwind(test) {
            self.runtime.lock().unwrap().print();
            panic::resume_unwind(err);
        }
        self.runtime.lock().unwrap().prepare_next_execution()
    }

    #[inline]
    fn update_running_thread(&self, update_runtime: impl FnOnce(&mut Runtime) -> Option<ThreadId>) {
        let mut runtime = self.runtime.lock().unwrap();
        let Some(next) = update_runtime(&mut runtime) else {
            drop(runtime);
            panic!("all threads are blocked");
        };
        self.running_thread.store(next.get(), Ordering::Relaxed);
        runtime.unpark_running_thread(next);
    }

    #[inline]
    fn park(&self) {
        let thread_id = ThreadId::current();
        loop {
            for _ in 0..PARK_SPIN {
                if self.running_thread.load(Ordering::Relaxed) == thread_id.get() {
                    return;
                }
                std::hint::spin_loop();
            }
            std::thread::park();
        }
    }

    fn spawn<T: Send + 'static>(
        self: &Arc<Self>,
        builder: std::thread::Builder,
        f: impl FnOnce() -> T + Send + 'static,
        location: Location,
    ) -> (ThreadId, std::io::Result<std::thread::JoinHandle<T>>) {
        let synchronizer = self.clone();
        let spawned_id = synchronizer.runtime.lock().unwrap().spawn(location);
        self.running_thread
            .store(ThreadId::DUMMY.get(), Ordering::Relaxed);
        let handle = builder.spawn(move || {
            SYNCHRONIZER.with(|cell| cell.replace(Some(synchronizer)));
            synchronize(|rt, _| rt.start_thread());
            let _end_thread = EndThread;
            f()
        });
        self.park();
        (spawned_id, handle)
    }

    fn spawn_scoped<'scope, 'env, T: Send + 'scope>(
        self: &Arc<Self>,
        scope: &'scope std::thread::Scope<'scope, 'env>,
        f: impl FnOnce() -> T + Send + 'scope,
        location: Location,
    ) -> (ThreadId, std::thread::ScopedJoinHandle<'scope, T>) {
        let synchronizer = self.clone();
        let spawned_id = synchronizer.runtime.lock().unwrap().spawn(location);
        self.running_thread
            .store(ThreadId::DUMMY.get(), Ordering::Relaxed);
        let handle = scope.spawn(|| {
            SYNCHRONIZER.with(|cell| cell.replace(Some(synchronizer)));
            synchronize(|rt, _| rt.start_thread());
            let _end_thread = EndThread;
            f()
        });
        self.park();
        (spawned_id, handle)
    }
}

#[inline]
pub(crate) fn is_synchronized() -> bool {
    SYNCHRONIZER.with(|cell| cell.borrow().is_some())
}

#[inline]
#[track_caller]
pub(crate) fn synchronize(
    update_runtime: impl FnOnce(&mut Runtime, Location) -> Option<ThreadId>,
) -> bool {
    let location = panic::Location::caller();
    SYNCHRONIZER.with(|cell| {
        if let Some(synchronizer) = cell.borrow().as_ref() {
            synchronizer.update_running_thread(|rt| update_runtime(rt, location));
            synchronizer.park();
            return true;
        }
        false
    })
}

pub(crate) fn synchronized<T>(value: T) -> T {
    synchronize(Runtime::synchronized_operation);
    value
}

#[inline]
#[track_caller]
pub(crate) fn spawn<T: Send + 'static>(
    builder: std::thread::Builder,
    f: impl FnOnce() -> T + Send + 'static,
) -> (ThreadId, std::io::Result<std::thread::JoinHandle<T>>) {
    let location = panic::Location::caller();
    SYNCHRONIZER.with(|cell| {
        if let Some(synchronizer) = cell.borrow().as_ref() {
            synchronizer.spawn(builder, f, location)
        } else {
            (ThreadId::DUMMY, builder.spawn(f))
        }
    })
}

#[inline]
#[track_caller]
pub(crate) fn spawn_scoped<'scope, 'env, T: Send + 'scope>(
    scope: &'scope std::thread::Scope<'scope, 'env>,
    f: impl FnOnce() -> T + Send + 'scope,
) -> (ThreadId, std::thread::ScopedJoinHandle<'scope, T>) {
    let location = panic::Location::caller();
    SYNCHRONIZER.with(|cell| {
        if let Some(synchronizer) = cell.borrow().as_ref() {
            synchronizer.spawn_scoped(scope, f, location)
        } else {
            (ThreadId::DUMMY, scope.spawn(f))
        }
    })
}

struct EndThread;

impl Drop for EndThread {
    fn drop(&mut self) {
        SYNCHRONIZER.with(|cell| {
            let borrow = cell.borrow();
            let synchronizer = borrow.as_ref().unwrap();
            synchronizer.update_running_thread(|rt| rt.end_thread());
        });
    }
}

pub trait TestResult {
    fn unwrap(self);
}

impl TestResult for () {
    fn unwrap(self) {}
}

impl<E: Debug> TestResult for Result<(), E> {
    fn unwrap(self) {
        self.unwrap();
    }
}

/// Run a test, executing all possible concurrency scenarios.
pub fn run<R: TestResult>(test: impl Fn() -> R + RefUnwindSafe) {
    SYNCHRONIZER.with(|cell| {
        cell.replace(Some(Arc::new(Synchronizer::new())));
        // clone to release borrowing
        let synchronizer = cell.borrow().as_ref().unwrap().clone();
        let mut execution_count = 0;
        while synchronizer.run(|| test().unwrap()) {
            execution_count += 1;
            if execution_count % 10 == 0 {
                print!(".");
                std::io::stdout().flush().unwrap();
            }
        }
    });
}

/// Print the current synchronized execution.
pub fn print_execution() {
    SYNCHRONIZER.with(|cell| {
        let borrow = cell.borrow();
        let synchronizer = borrow.as_ref().unwrap();
        synchronizer.runtime.lock().unwrap().print();
    });
}
