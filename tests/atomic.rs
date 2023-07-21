use std::sync::{atomic::AtomicU8, Arc};

use synchronizer::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

#[test]
#[should_panic]
fn simple_invalid() {
    synchronizer::run(|| {
        let b = Arc::new(AtomicBool::default());
        let b2 = b.clone();
        let thread = thread::spawn(move || {
            b2.store(true, Ordering::Relaxed);
        });
        assert!(!b.load(Ordering::Relaxed));
        thread.join().map_err(std::panic::resume_unwind).unwrap();
    });
}

#[test]
fn simple_valid() {
    synchronizer::run(|| {
        let int = Arc::new(AtomicU8::default());
        let int2 = int.clone();
        let thread = thread::spawn(move || {
            assert!(int2.fetch_add(1, Ordering::Relaxed) <= 2);
        });
        assert!(int.fetch_add(1, Ordering::Relaxed) <= 2);
        thread.join().map_err(std::panic::resume_unwind).unwrap();
        assert_eq!(int.load(Ordering::Relaxed), 2);
    });
}
