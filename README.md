# synchronizer
Another concurrency permutation testing tool for Rust. 

## Example

The following program can fail if the thread run before the assertion. *synchronizer* will trigger the failure
```
use std::sync::{atomic::AtomicU8, Arc};

use synchronizer::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

synchronizer::run(|| {
    let b = Arc::new(AtomicBool::default());
    let b2 = b.clone();
    let thread = thread::spawn(move || {
        b2.store(true, Ordering::Relaxed);
    });
    assert!(!b.load(Ordering::Relaxed));
    thread.join().map_err(std::panic::resume_unwind).unwrap();
}); // panics
```

## How it works

*synchronizer* wraps modules `std::thread` and `std::thread` and add thread synchronization for each operation (atomic, thread spawning, mutex, etc.); only one thread run between each operation.

The execution steps/running threads are tracked, and permuted at the end of the execution for the next one.

In case of failure, *synchronize* will print the execution summary, with threads' state and synchronized operations in execution order, to help understanding the failure.

## Difference with *loom*

I don't know how *loom* works in detail, but *synchronizer* don't do causality calculus, it just runs the program one thread at a time.

Also, in unsynchronized-context (when `synchronizer::run` is not called), *synchronizer* wrapper functions/types just works as the wrapped functions/types. In fact, most of the wrapped types are `#[repr(transparent)]`. 
So you can just add *synchronizer* to `[dev-dependencies]` and use it in normal tests; there is no need of `--cfg synchronizer`.

By the way, library with low-level synchronization primitive could add *synchronizer* as a feature to enable using them in synchronized tests.

I haven't measured performance yet, so I cannot compare with *loom* in terms of execution rapidity.

## TODO

- The execution summary is quite crude, it's just a `#[derive(Debug)]` for now
- Spurious wakeups are not handled yet (but it's not really difficult and should be done soon)
- Add asynchronous support? just a `block_on` method?
- Add cell borrowing tracking? Actually, it could be done in an independent library, and IMO it should be independent.