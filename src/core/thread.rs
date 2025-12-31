use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crossbeam_deque::{Injector, Steal, Stealer, Worker as Stack};

pub trait WorkerState<W>: Sized {
    fn work(worker: &mut Worker<Self, W>, work: W);

    fn stop(&mut self);
}

pub fn worker_pool<S, W>(
    num_workers: usize,
    init_work: impl IntoIterator<Item = W>,
    mut build_worker: impl FnMut(usize) -> S,
    wait: impl FnOnce(),
) where
    S: WorkerState<W> + Send,
    W: Send,
{
    let injector = crossbeam_deque::Injector::new();
    for work in init_work {
        injector.push(Msg::Work(work));
    }

    let alive_workers = AtomicUsize::new(num_workers);
    let mut stealers = Vec::with_capacity(num_workers);
    let mut workers = (0..num_workers)
        .map(|i| {
            let state = build_worker(i);

            let stack = Stack::new_lifo();
            stealers.push(stack.stealer());

            Worker {
                worker_index: i,
                num_workers,
                alive_threads: &alive_workers,
                stack,
                injector: &injector,
                stealers: &[],
                state,
            }
        })
        .collect::<Vec<_>>();

    for worker in workers.iter_mut() {
        worker.stealers = &stealers;
    }

    std::thread::scope(|scope| {
        for mut worker in workers {
            scope.spawn(move || worker.start());
        }

        wait();
    });
}

pub struct Worker<'a, S, W> {
    worker_index: usize,
    num_workers: usize,
    alive_threads: &'a AtomicUsize,
    stack: Stack<Msg<W>>,
    injector: &'a Injector<Msg<W>>,
    stealers: &'a [Stealer<Msg<W>>],
    pub state: S,
}

pub enum Msg<T> {
    Work(T),
    Stop,
}

impl<S: WorkerState<W>, W> Worker<'_, S, W> {
    fn start(&mut self) {
        while let Msg::Work(work) = self.receive_msg() {
            S::work(self, work)
        }

        self.state.stop();
    }

    fn receive_msg(&self) -> Msg<W> {
        if let Some(msg) = self.stack.pop() {
            return msg;
        }

        if let Some(msg) = self.try_steal() {
            return msg;
        }

        // Couldn't receive item directly, this worker is blocking.
        let alive = self.alive_threads.fetch_sub(1, Ordering::SeqCst) - 1;
        if alive == 0 {
            // All other workers are already waiting, so send the
            // stop signal.
            for _ in 1..self.num_workers {
                self.injector.push(Msg::Stop);
            }
            return Msg::Stop;
        }

        let msg = loop {
            if let Some(msg) = self.try_steal() {
                break msg;
            }
            std::thread::sleep(Duration::from_millis(1));
        };

        self.alive_threads.fetch_add(1, Ordering::SeqCst);

        msg
    }

    fn try_steal(&self) -> Option<Msg<W>> {
        if let Steal::Success(msg) = self.injector.steal() {
            return Some(msg);
        }

        for i in 0..self.num_workers {
            let idx = (i + self.worker_index) % self.num_workers;
            if let Steal::Success(msg) = self.stealers[idx].steal() {
                return Some(msg);
            }
        }

        None
    }

    /// Queue work to be processed by the worker pool.
    pub fn push_work(&mut self, work: W) {
        self.stack.push(Msg::Work(work));
    }
}
