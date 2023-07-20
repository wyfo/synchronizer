use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    panic,
};

pub(crate) type Location = &'static panic::Location<'static>;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct ThreadId(usize);

impl ThreadId {
    pub(crate) fn current() -> Self {
        THREAD_ID.with(|cell| cell.get())
    }

    pub(crate) fn get(self) -> usize {
        self.0
    }

    pub(crate) const DUMMY: Self = Self(usize::MAX);
}

thread_local! {
    // use https://github.com/rust-lang/rust/issues/92122 when stabilized
    static THREAD_ID: Cell<ThreadId> = Cell::new(ThreadId::DUMMY);
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Address(usize);

impl Address {
    pub(crate) fn new<T: ?Sized>(ptr: *const T) -> Self {
        Self(ptr as *const () as usize)
    }
}

impl<T: ?Sized> From<&T> for Address {
    fn from(value: &T) -> Self {
        Address::new(value)
    }
}

#[derive(Debug)]
pub struct Runtime {
    spawned_thread_count: usize,
    threads: BTreeMap<ThreadId, ThreadState>,
    steps: Vec<Step>,
    current_step: usize,
    barriers: HashMap<Address, usize>,
    mutexes: HashSet<Address>,
    onces: HashSet<Address>,
    rwlocks: HashMap<Address, isize>, // > 0 means readers, -1 means writer
}

impl Runtime {
    pub(crate) fn new() -> Self {
        let mut runtime = Self {
            spawned_thread_count: Default::default(),
            threads: Default::default(),
            steps: Default::default(),
            current_step: Default::default(),
            barriers: Default::default(),
            mutexes: Default::default(),
            onces: Default::default(),
            rwlocks: Default::default(),
        };
        runtime.add_thread();
        runtime
    }

    pub(crate) fn prepare_next_execution(&mut self) -> bool {
        self.spawned_thread_count = 0;
        self.threads.clear();
        self.barriers.clear();
        self.mutexes.clear();
        self.onces.clear();
        self.rwlocks.clear();
        self.add_thread();
        let mut running_threads: BTreeSet<_> = self.threads.keys().copied().collect();
        for step in self.steps.iter_mut().rev() {
            if step.prepare_next_execution(&mut running_threads) {
                break;
            }
            self.current_step -= 1;
        }
        self.steps.drain(self.current_step..);
        self.current_step = 0;
        !self.steps.is_empty()
    }

    fn add_thread(&mut self) {
        let thread_id = ThreadId(self.spawned_thread_count);
        THREAD_ID.with(|cell| cell.set(thread_id));
        self.threads.insert(thread_id, ThreadState::new());
        self.spawned_thread_count += 1;
    }

    fn add_step(
        &mut self,
        location: impl Into<Option<Location>>,
        blocked: Option<ThreadBlocked>,
        wakeup: impl Fn(ThreadBlocked) -> bool,
        wakeup_all: bool,
    ) -> Option<ThreadId> {
        if blocked.is_some() {
            self.threads.get_mut(&ThreadId::current()).unwrap().blocked = blocked;
        }
        let next_running_thread = if let Some(step) = self.steps.get(self.current_step) {
            match (&step.wakeup, step.wakeup_branch, wakeup_all) {
                (Some(Wakeup::One(id)), _, _) => self.threads.get_mut(id).unwrap().blocked = None,
                (Some(Wakeup::Many(ids)), _, true) => ids
                    .iter()
                    .for_each(|id| self.threads.get_mut(id).unwrap().blocked = None),
                (Some(Wakeup::Many(ids)), Some(branch), false) => {
                    self.threads.get_mut(&ids[branch]).unwrap().blocked = None;
                }
                (Some(Wakeup::Many(_)), None, false) => unreachable!(),
                (None, _, _) => {}
            }
            step.next_running_thread
        } else {
            let wakeup = self
                .threads
                .iter()
                .filter_map(|(id, thread)| wakeup(thread.blocked?).then_some(*id))
                .collect();
            if let Some(Wakeup::One(id)) = &wakeup {
                self.threads.get_mut(id).unwrap().blocked = None;
            } else if let Some(Wakeup::Many(ids)) = &wakeup {
                for id in ids {
                    self.threads.get_mut(id).unwrap().blocked = None;
                    if !wakeup_all {
                        break;
                    }
                }
            }
            let next = self
                .threads
                .iter()
                .find_map(|(id, thread)| thread.blocked.is_none().then_some(*id))?;
            self.steps.push(Step::new(
                location.into(),
                blocked.is_some(),
                wakeup,
                wakeup_all,
                next,
            ));
            next
        };
        self.current_step += 1;
        Some(next_running_thread)
    }

    #[inline]
    pub(crate) fn unpark_running_thread(&self, thread_id: ThreadId) {
        self.threads[&thread_id].thread.unpark();
    }

    #[inline]
    pub(crate) fn synchronized_operation(&mut self, location: Location) -> Option<ThreadId> {
        self.add_step(location, None, |_| false, false)
    }

    pub(crate) fn spawn(&mut self, location: Location) -> ThreadId {
        // If a step is added, its `next_running_thread` will be correct, as the first running
        // thread chosen will always be the spawner one (even if the spawned one was already
        // added to `self.threads`)
        self.add_step(location, None, |_| false, false);
        ThreadId(self.spawned_thread_count)
    }

    pub(crate) fn start_thread(&mut self) -> Option<ThreadId> {
        self.add_thread();
        // Step has been added by `Self::spawn`
        let next_running_thread = self.steps[self.current_step - 1].next_running_thread;
        Some(next_running_thread)
    }

    pub(crate) fn end_thread(&mut self) -> Option<ThreadId> {
        let thread_id = ThreadId::current();
        self.threads.remove(&thread_id);
        let wakeup = |blk| matches!(blk, ThreadBlocked::Joining(id, _) if thread_id == id);
        self.add_step(None, None, wakeup, false)
    }

    pub(crate) fn join(&mut self, thread_id: ThreadId, location: Location) -> Option<ThreadId> {
        let blocked = self
            .threads
            .get(&thread_id)
            .map(|_| ThreadBlocked::Joining(thread_id, panic::Location::caller()));
        self.add_step(location, blocked, |_| false, false)
    }

    pub(crate) fn park(&mut self, location: Location) -> Option<ThreadId> {
        let thread_id = ThreadId::current();
        let thread = &mut self.threads.get_mut(&thread_id).unwrap();
        let blocked = (!thread.unparked).then_some(ThreadBlocked::Parked(thread_id, location));
        thread.unparked = false;
        self.add_step(location, blocked, |_| false, false)
    }

    pub(crate) fn unpark(&mut self, thread_id: ThreadId, location: Location) -> Option<ThreadId> {
        if let Some(thread) = self.threads.get_mut(&thread_id) {
            if !thread.unparked && !matches!(thread.blocked, Some(ThreadBlocked::Parked(_, _))) {
                thread.unparked = true;
            }
        }
        let wakeup = |blk| matches!(blk, ThreadBlocked::Parked(id, _ ) if thread_id == id);
        self.add_step(location, None, wakeup, false)
    }

    pub(crate) fn wait_barrier(
        &mut self,
        addr: Address,
        capacity: usize,
        leader: &mut bool,
        location: Location,
    ) -> Option<ThreadId> {
        let current = self.barriers.entry(addr).or_insert(0);
        *current += 1;
        if *current >= capacity {
            *leader = true;
            self.barriers.remove(&addr);
            let wakeup =
                |blk| matches!(blk, ThreadBlocked::Barrier(blk_addr, _) if blk_addr == addr);
            self.add_step(location, None, wakeup, true)
        } else {
            let blocked = Some(ThreadBlocked::Barrier(addr, location));
            self.add_step(location, blocked, |_| false, true)
        }
    }

    pub(crate) fn wait_condvar(
        &mut self,
        condvar: Address,
        mutex: Address,
        location: Location,
    ) -> Option<ThreadId> {
        let blocked = Some(ThreadBlocked::Condvar(condvar, location));
        let wakeup = |blk| matches!(blk, ThreadBlocked::Mutex(addr, _) if addr == mutex);
        self.add_step(location, blocked, wakeup, false)
    }

    pub(crate) fn notify_condvar(
        &mut self,
        addr: Address,
        all: bool,
        location: Location,
    ) -> Option<ThreadId> {
        let wakeup = |blk| matches!(blk, ThreadBlocked::Condvar(blk_addr, _) if blk_addr == addr);
        self.add_step(location, None, wakeup, all)
    }

    pub(crate) fn acquire_mutex(&mut self, addr: Address, location: Location) -> Option<ThreadId> {
        let blocked = (!self.mutexes.insert(addr)).then_some(ThreadBlocked::Mutex(addr, location));
        self.add_step(location, blocked, |_| false, false)
    }

    pub(crate) fn try_acquire_mutex(
        &mut self,
        addr: Address,
        location: Location,
    ) -> Option<ThreadId> {
        self.mutexes.insert(addr);
        self.add_step(location, None, |_| false, false)
    }

    pub(crate) fn release_mutex(&mut self, addr: Address, location: Location) -> Option<ThreadId> {
        self.mutexes.remove(&addr);
        let wakeup = |blk| matches!(blk, ThreadBlocked::Mutex(blk_addr, _) if blk_addr == addr);
        self.add_step(location, None, wakeup, false)
    }

    pub(crate) fn acquire_once(&mut self, addr: Address, location: Location) -> Option<ThreadId> {
        let blocked = (!self.onces.insert(addr)).then_some(ThreadBlocked::Once(addr, location));
        self.add_step(location, blocked, |_| false, false)
    }

    pub(crate) fn release_once(&mut self, addr: Address) -> Option<ThreadId> {
        let wakeup = |blk| matches!(blk, ThreadBlocked::Once(blk_addr, _) if blk_addr == addr);
        self.add_step(None, None, wakeup, true)
    }

    pub(crate) fn acquire_read(&mut self, addr: Address, location: Location) -> Option<ThreadId> {
        let entry = self.rwlocks.entry(addr).or_insert(0);
        let blocked = (*entry == -1).then_some(ThreadBlocked::Read(addr, location));
        if blocked.is_none() {
            *entry += 1;
        }
        self.add_step(location, blocked, |_| false, false)
    }

    pub(crate) fn try_acquire_read(
        &mut self,
        addr: Address,
        location: Location,
    ) -> Option<ThreadId> {
        let entry = self.rwlocks.entry(addr).or_insert(0);
        if *entry != -1 {
            *entry += 1;
        }
        self.add_step(location, None, |_| false, false)
    }

    pub(crate) fn release_read(&mut self, addr: Address, location: Location) -> Option<ThreadId> {
        let entry = self.rwlocks.get_mut(&addr).unwrap();
        *entry -= 1;
        let no_more_reader = *entry == 0;
        if no_more_reader {
            self.rwlocks.remove(&addr);
        }
        self.add_step(
            location,
            None,
            |blk| {
                no_more_reader
                    && matches!(blk, ThreadBlocked::Write(blk_addr, _) if blk_addr == addr)
            },
            false,
        )
    }

    pub(crate) fn acquire_write(&mut self, addr: Address, location: Location) -> Option<ThreadId> {
        let entry = self.rwlocks.entry(addr).or_insert(0);
        let blocked = (*entry != 0).then_some(ThreadBlocked::Write(addr, location));
        if blocked.is_none() {
            *entry = -1;
        }
        self.add_step(location, blocked, |_| false, false)
    }

    pub(crate) fn try_acquire_write(
        &mut self,
        addr: Address,
        location: Location,
    ) -> Option<ThreadId> {
        let entry = self.rwlocks.entry(addr).or_insert(0);
        if *entry == 0 {
            *entry = -1;
        }
        self.add_step(location, None, |_| false, false)
    }

    pub(crate) fn release_write(&mut self, addr: Address, location: Location) -> Option<ThreadId> {
        self.rwlocks.remove(&addr);
        self.add_step(
            location,
            None,
            |blk| {
                matches!(blk, ThreadBlocked::Read(blk_addr, _) | ThreadBlocked::Write(blk_addr, _) if blk_addr == addr)
            },
            false,
        )
    }

    pub(crate) fn print(&mut self) {
        println!("{self:?}");
    }
}

#[derive(Debug)]
struct ThreadState {
    thread: std::thread::Thread,
    unparked: bool,
    blocked: Option<ThreadBlocked>,
}

impl ThreadState {
    fn new() -> Self {
        Self {
            thread: std::thread::current(),
            unparked: false,
            blocked: None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum ThreadBlocked {
    Joining(ThreadId, Location),
    Parked(ThreadId, Location),
    Barrier(Address, Location),
    Condvar(Address, Location),
    Mutex(Address, Location),
    Once(Address, Location),
    Read(Address, Location),
    Write(Address, Location),
}

#[derive(Debug)]
enum Wakeup {
    One(ThreadId),
    Many(Vec<ThreadId>),
}

impl FromIterator<ThreadId> for Option<Wakeup> {
    fn from_iter<T: IntoIterator<Item = ThreadId>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let first = iter.next()?;
        Some(match iter.next() {
            Some(second) => Wakeup::Many([first, second].into_iter().chain(iter).collect()),
            None => Wakeup::One(first),
        })
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct Step {
    thread: ThreadId,
    location: Option<Location>,
    blocked: bool,
    wakeup: Option<Wakeup>,
    wakeup_branch: Option<usize>,
    next_running_thread: ThreadId,
}

impl Step {
    fn new(
        location: Option<Location>,
        blocked: bool,
        wakeup: Option<Wakeup>,
        wakeup_all: bool,
        next_running_thread: ThreadId,
    ) -> Self {
        let wakeup_branch = matches!(&wakeup , Some(Wakeup::Many(_)) if !wakeup_all).then_some(0);
        Self {
            thread: ThreadId::current(),
            location,
            blocked,
            wakeup,
            wakeup_branch,
            next_running_thread,
        }
    }

    fn prepare_next_execution(&mut self, running_threads: &mut BTreeSet<ThreadId>) -> bool {
        if let Some(&next) = running_threads.range(self.next_running_thread..).nth(1) {
            self.next_running_thread = next;
            return true;
        }
        running_threads.insert(self.thread);
        match (&self.wakeup, self.wakeup_branch) {
            (Some(Wakeup::Many(threads)), Some(branch)) if branch + 1 != threads.len() => {
                self.wakeup_branch = Some(branch + 1);
                self.next_running_thread = *running_threads.first().unwrap();
                return true;
            }
            (Some(Wakeup::Many(threads)), Some(_)) => {
                running_threads.remove(threads.last().unwrap());
            }
            (Some(Wakeup::Many(threads)), None) => {
                for thread in threads {
                    running_threads.remove(thread);
                }
            }
            (Some(Wakeup::One(thread)), _) => {
                running_threads.remove(thread);
            }
            (None, _) => {}
        }
        false
    }
}
