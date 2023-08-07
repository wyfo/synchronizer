use synchronizer::{
    sync::{Arc, Mutex},
    thread,
};

#[test]
#[should_panic]
fn simple_invalid() {
    synchronizer::run(|| {
        let b = Arc::new(Mutex::new(false));
        let b2 = b.clone();
        let thread = thread::spawn(move || {
            *b2.lock().unwrap() = true;
        });
        assert!(!*b.lock().unwrap());
        thread.join().map_err(std::panic::resume_unwind).unwrap();
    });
}

#[test]
#[should_panic]
fn simple_all_blocked() {
    synchronizer::run(|| {
        let int = Arc::new(Mutex::new(0));
        let int2 = int.clone();
        let thread = thread::spawn(move || {
            let mut lock = int2.lock().unwrap();
            *lock += 1;
            assert!(*lock <= 2);
        });
        let mut lock = int.lock().unwrap();
        *lock += 1;
        assert!(*lock <= 2);
        // don't drop the lock before joining
        thread.join().map_err(std::panic::resume_unwind).unwrap();
        assert_eq!(*int.lock().unwrap(), 2);
    });
}

#[test]
fn simple_valid() {
    synchronizer::run(|| {
        let int = Arc::new(Mutex::new(0));
        let int2 = int.clone();
        let thread = thread::spawn(move || {
            let mut lock = int2.lock().unwrap();
            *lock += 1;
            assert!(*lock <= 2);
        });
        let mut lock = int.lock().unwrap();
        *lock += 1;
        assert!(*lock <= 2);
        drop(lock);
        thread.join().map_err(std::panic::resume_unwind).unwrap();
        assert_eq!(*int.lock().unwrap(), 2);
    });
}
