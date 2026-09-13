//! Wait-Free & Lock-Free Work-Stealing Deque (Chase-Lev Algorithm) (v1.2.3)
//!
//! Provides the core task-scheduling primitive for Datara & Forgen's M:N fiber engine:
//! - Owner operations (`push`, `pop`) operate on the bottom index with zero atomic
//!   read-modify-write (RMW) instructions on the uncontended fast-path.
//! - Concurrent worker threads steal tasks from the top index using a single atomic CAS.
//! - Eliminates all OS mutex locks, kernel transitions, and cache thrashing.

use std::sync::atomic::{AtomicIsize, Ordering};

/// A fixed-capacity ring buffer for Chase-Lev task slots.
pub struct TaskBuffer<T> {
    buffer: Vec<Option<T>>,
    capacity: usize,
    mask: usize,
}

impl<T> TaskBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity.is_power_of_two(),
            "Capacity must be a power of two"
        );
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(None);
        }
        Self {
            buffer,
            capacity,
            mask: capacity - 1,
        }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    pub unsafe fn read(&self, index: isize) -> Option<T>
    where
        T: Clone,
    {
        let slot = (index as usize) & self.mask;
        self.buffer[slot].clone()
    }

    #[inline(always)]
    pub unsafe fn write(&mut self, index: isize, value: Option<T>) {
        let slot = (index as usize) & self.mask;
        self.buffer[slot] = value;
    }
}

/// The result of attempting to steal a task.
#[derive(Debug, PartialEq, Eq)]
pub enum Steal<T> {
    Empty,
    Abort,
    Success(T),
}

/// Chase-Lev work-stealing double-ended queue.
pub struct ChaseLevDeque<T> {
    top: AtomicIsize,
    bottom: AtomicIsize,
    buffer: std::cell::UnsafeCell<TaskBuffer<T>>,
}

unsafe impl<T: Send> Send for ChaseLevDeque<T> {}
unsafe impl<T: Send> Sync for ChaseLevDeque<T> {}

impl<T: Clone + Send> ChaseLevDeque<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            top: AtomicIsize::new(0),
            bottom: AtomicIsize::new(0),
            buffer: std::cell::UnsafeCell::new(TaskBuffer::new(capacity)),
        }
    }

    /// Owner thread pushes a task to the bottom of the deque.
    /// Fast-path: single relaxed load and release store (0 locked bus cycles).
    pub fn push(&self, task: T) -> Result<(), &'static str> {
        let b = self.bottom.load(Ordering::Relaxed);
        let t = self.top.load(Ordering::Acquire);
        let buf = unsafe { &mut *self.buffer.get() };

        let size = b - t;
        if size >= buf.capacity() as isize {
            return Err("Deque capacity saturated");
        }

        unsafe {
            buf.write(b, Some(task));
        }
        self.bottom.store(b + 1, Ordering::Release);
        Ok(())
    }

    /// Owner thread pops a task from the bottom of the deque.
    pub fn pop(&self) -> Option<T> {
        let b = self.bottom.load(Ordering::Relaxed) - 1;
        self.bottom.store(b, Ordering::SeqCst);
        let t = self.top.load(Ordering::SeqCst);

        let buf = unsafe { &mut *self.buffer.get() };

        if b < t {
            // Queue was already empty
            self.bottom.store(t, Ordering::Relaxed);
            None
        } else if b > t {
            // More than 1 item remains: uncontended pop
            let task = unsafe { buf.read(b) };
            task
        } else {
            // Exactly 1 item remains: compete with concurrent stealers via CAS
            let task = unsafe { buf.read(b) };
            if self
                .top
                .compare_exchange(t, t + 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                self.bottom.store(t + 1, Ordering::Relaxed);
                task
            } else {
                // Lost race to stealer
                self.bottom.store(t + 1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Concurrent worker threads steal a task from the top of the deque.
    pub fn steal(&self) -> Steal<T> {
        let t = self.top.load(Ordering::Acquire);
        let b = self.bottom.load(Ordering::Acquire);

        if t >= b {
            return Steal::Empty;
        }

        let buf = unsafe { &*self.buffer.get() };
        let task = unsafe { buf.read(t) };

        if let Some(item) = task {
            if self
                .top
                .compare_exchange(t, t + 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                Steal::Success(item)
            } else {
                Steal::Abort
            }
        } else {
            Steal::Empty
        }
    }

    /// Number of tasks currently in the deque.
    pub fn len(&self) -> usize {
        let b = self.bottom.load(Ordering::Relaxed);
        let t = self.top.load(Ordering::Relaxed);
        (b - t).max(0) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
