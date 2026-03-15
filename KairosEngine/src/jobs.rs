use std::{sync::{Arc, Barrier, atomic::{AtomicBool, Ordering}}, thread};


pub trait Job: Send + 'static {
    fn execute(&mut self);
}

#[derive(Clone)]
pub struct JobHandle {
    barrier: Arc<Barrier>,
    is_completed: Arc<AtomicBool>
}
impl JobHandle {
    pub fn compelete(self) {
        self.barrier.wait();
        self.is_completed.store(true, Ordering::SeqCst);
    }

    pub fn is_completed(&self) -> bool {
        self.is_completed.load(Ordering::SeqCst)
    }
}

pub fn schedule<J: Job>(job: J) -> JobHandle {
    let barrier = Arc::new(Barrier::new(2));
    let is_completed = Arc::new(AtomicBool::new(false));
    let job_barrier = Arc::clone(&barrier);
    let job_is_completed = Arc::clone(&is_completed);

    thread::spawn(move || {
        let mut job = job;
        job.execute();
        job_barrier.wait();
        job_is_completed.store(true, Ordering::SeqCst);
    });

    JobHandle { barrier, is_completed }
}