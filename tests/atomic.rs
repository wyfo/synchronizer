use synchronizer::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
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
        thread.join().unwrap();
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
        thread.join().unwrap();
        assert_eq!(int.load(Ordering::Relaxed), 2);
    });
}
