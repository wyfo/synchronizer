use synchronizer::{
    sync::{Arc, Mutex},
    thread,
};

#[test]
#[should_panic]
fn all_thread_blocked() {
    synchronizer::run(|| {
        let mutex = Arc::new(Mutex::new(()));
        let main_thread = thread::current();
        let _guard = mutex.lock().unwrap();
        let mutex2 = mutex.clone();
        let thread = thread::spawn(move || {
            mutex2.lock().unwrap();
            main_thread.unpark();
        });
        thread.join().unwrap();
    })
}
