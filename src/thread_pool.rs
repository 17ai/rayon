use job::{Job, NULL_JOB};
use latch::Latch;
#[allow(unused_imports)]
use log::Event::*;
use rand;
use std::cell::Cell;
use std::sync::{Arc, Condvar, Mutex, Once, ONCE_INIT};
use std::thread;
use util::leak;

///////////////////////////////////////////////////////////////////////////

const NUM_CPUS: usize = 4;

pub struct Registry {
    thread_infos: Vec<ThreadInfo>,
    state: Mutex<RegistryState>,
    work_available: Condvar,
}

struct RegistryState {
    threads_at_work: usize,
    injected_jobs: Vec<*mut Job>,
}

unsafe impl Send for Registry { }
unsafe impl Sync for Registry { }

///////////////////////////////////////////////////////////////////////////
// Initialization

static mut THE_REGISTRY: Option<&'static Registry> = None;
static THE_REGISTRY_SET: Once = ONCE_INIT;

/// Starts the worker threads (if that has not already happened) and
/// returns the registry.
pub fn get_registry() -> &'static Registry {
    THE_REGISTRY_SET.call_once(|| {
        let registry = leak(Registry::new(NUM_CPUS));
        unsafe { THE_REGISTRY = Some(registry); }
    });
    unsafe { THE_REGISTRY.unwrap() }
}

impl Registry {
    fn new(num_threads: usize) -> Arc<Registry> {
        let registry = Arc::new(Registry {
            thread_infos: (0..num_threads).map(|_| ThreadInfo::new()).collect(),
            state: Mutex::new(RegistryState::new()),
            work_available: Condvar::new(),
        });

        for index in 0 .. num_threads {
            let registry = registry.clone();
            thread::spawn(move || unsafe { main_loop(registry, index) });
        }

        registry
    }

    fn num_threads(&self) -> usize {
        self.thread_infos.len()
    }

    /// Waits for the worker threads to get up and running.  This is
    /// meant to be used for benchmarking purposes, primarily, so that
    /// you can get more consistent numbers by having everything
    /// "ready to go".
    pub fn wait_until_primed(&self) {
        for info in &self.thread_infos {
            info.primed.wait();
        }
    }

    ///////////////////////////////////////////////////////////////////////////
    // MAIN LOOP
    //
    // So long as all of the worker threads are hanging out in their
    // top-level loop, there is no work to be done.

    fn start_working(&self, index: usize) {
        log!(StartWorking { index: index });
        let mut state = self.state.lock().unwrap();
        state.threads_at_work += 1;
        self.work_available.notify_all();
    }

    fn inject(&self, injected_jobs: &[*mut Job]) {
        log!(InjectJobs { count: injected_jobs.len() });
        let mut state = self.state.lock().unwrap();
        state.injected_jobs.extend(injected_jobs);
        self.work_available.notify_all();
    }

    fn wait_for_work(&self, _worker: usize, was_active: bool) -> Option<*mut Job> {
        log!(WaitForWork { worker: _worker, was_active: was_active });

        let mut state = self.state.lock().unwrap();

        if was_active {
            state.threads_at_work -= 1;
        }

        loop {
            // Otherwise, if anything was injected from outside,
            // return that.  Note that this gives preference to
            // injected items over stealing from others, which is a
            // bit dubious, but then so is the opposite.
            if let Some(job) = state.injected_jobs.pop() {
                state.threads_at_work += 1;
                self.work_available.notify_all();
                return Some(job);
            }

            // If any of the threads are running a job, we should spin
            // up, since they may generate subworkitems.
            if state.threads_at_work > 0 {
                return None;
            }

            state = self.work_available.wait(state).unwrap();
        }
    }
}

impl RegistryState {
    pub fn new() -> RegistryState {
        RegistryState {
            threads_at_work: 0,
            injected_jobs: Vec::new(),
        }
    }
}

struct ThreadInfo {
    // latch is set once thread has started and we are entering into
    // the main loop
    primed: Latch,
    deque: Mutex<ThreadDeque>,
}

impl ThreadInfo {
    fn new() -> ThreadInfo {
        ThreadInfo {
            deque: Mutex::new(ThreadDeque::new()),
            primed: Latch::new(),
        }
    }
}

struct ThreadDeque {
    bottom: *mut Job,
    top: *mut Job,
}

impl ThreadDeque {
    fn new() -> ThreadDeque {
        ThreadDeque { bottom: NULL_JOB, top: NULL_JOB }
    }
}

///////////////////////////////////////////////////////////////////////////
// WorkerThread identifiers

pub struct WorkerThread {
    registry: Arc<Registry>,
    index: usize
}

// This is a bit sketchy, but basically: the WorkerThread is
// allocated on the stack of the worker on entry and stored into this
// thread local variable. So it will remain valid at least until the
// worker is fully unwound. Using an unsafe pointer avoids the need
// for a RefCell<T> etc.
thread_local! {
    static WORKER_THREAD_STATE: Cell<*const WorkerThread> =
        Cell::new(0 as *const WorkerThread)
}

impl WorkerThread {
    /// Gets the `WorkerThread` index for the current thread; returns
    /// NULL if this is not a worker thread. This pointer is valid
    /// anywhere on the current thread.
    #[inline]
    pub unsafe fn current() -> *const WorkerThread {
        WORKER_THREAD_STATE.with(|t| t.get())
    }

    /// Sets `self` as the worker thread index for the current thread.
    /// This is done during worker thread startup.
    unsafe fn set_current(&self) {
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().is_null());
            t.set(self);
        });
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    #[inline]
    fn thread_info(&self) -> &ThreadInfo {
        &self.registry.thread_infos[self.index]
    }

    #[inline]
    pub unsafe fn push(&self, job: *mut Job) {
        let thread_info = self.thread_info();
        let mut deque = thread_info.deque.lock().unwrap();

        let top = deque.top;
        (*job).previous = top;
        if !top.is_null() {
            (*top).next = job;
        }

        deque.top = job;
        if deque.bottom.is_null() {
            deque.bottom = job;
        }
    }

    /// Pop `job` if it is still at the top of the stack.  Otherwise,
    /// some other thread has stolen this job.
    #[inline]
    pub unsafe fn pop(&self, job: *mut Job) -> bool {
        let thread_info = self.thread_info();
        let mut deque = thread_info.deque.lock().unwrap();
        if deque.top == job {
            let previous_job = (*job).previous;
            deque.top = previous_job;

            if previous_job != NULL_JOB {
                (*previous_job).next = NULL_JOB;
            } else {
                deque.bottom = NULL_JOB;
            }

            stat_popped!();

            true
        } else {
            false
        }
    }
}

///////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(registry: Arc<Registry>, index: usize) {
    let worker_thread = WorkerThread {
        registry: registry.clone(),
        index: index,
    };
    worker_thread.set_current();

    // let registry know we are ready to do work
    registry.thread_infos[index].primed.set();

    let mut was_active = false;
    loop {
        if let Some(injected_job) = registry.wait_for_work(index, was_active) {
            (*injected_job).execute();
            was_active = true;
        } else if let Some(stolen_job) = steal_work(&registry, index) {
            log!(StoleWork { worker: index, job: stolen_job });
            registry.start_working(index);
            (*stolen_job).execute();
            was_active = true;
        } else {
            was_active = false;
        }
    }
}

unsafe fn steal_work(registry: &Registry, index: usize) -> Option<*mut Job> {
    let num_threads = registry.num_threads();
    let start = rand::random::<usize>() % num_threads;
    (start .. num_threads)
        .chain(0 .. start)
        .filter(|&i| i != index)
        .filter_map(|i| steal_work_from(registry, i))
        .next()
}

unsafe fn steal_work_from(registry: &Registry, index: usize) -> Option<*mut Job> {
    let thread_info = &registry.thread_infos[index];
    let mut deque = thread_info.deque.lock().unwrap();
    if deque.bottom.is_null() {
        return None;
    }

    let job = deque.bottom;
    let next = (*job).next;
    deque.bottom = next;
    if next != NULL_JOB {
        (*next).previous = NULL_JOB;
    } else {
        deque.top = NULL_JOB;
    }
    stat_stolen!();
    Some(job)
}

pub unsafe fn inject(jobs: &[*mut Job]) {
    debug_assert!(WorkerThread::current().is_null());
    let registry = get_registry();
    registry.inject(jobs);
}
