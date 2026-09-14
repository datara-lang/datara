//! Milestone 4 - Chase-Lev work-stealing deque concurrency audit.
//!
//! Covers the classic failure modes a lock-free deque must survive:
//! 1. Exactly-once task delivery under concurrent owner-pop / stealer-CAS races.
//! 2. The single-item CAS window (pop vs steal race at `top == bottom - 1`):
//!    exactly one racer may observe the task; the loser must observe
//!    `None` / `Steal::Abort` / `Steal::Empty` and never duplicate it.
//! 3. Ring-buffer index wraparound: counters advance monotonically past
//!    `capacity` slots, so the mask-based slot mapping stays correct and the
//!    ABA hazard is structurally impossible (indices never wrap to old values).
//! 4. Capacity saturation must return `Err`, never wrap or lose data.

use forgen::runtime::scheduler::chase_lev::{ChaseLevDeque, Steal};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

type Deque = ChaseLevDeque<u64>;
type Flags = Arc<Vec<AtomicBool>>;

fn record_once(flags: &Flags, duplicates: &AtomicUsize, popped: &AtomicUsize, task: u64) {
    if flags[task as usize].swap(true, Ordering::SeqCst) {
        duplicates.fetch_add(1, Ordering::SeqCst);
    }
    popped.fetch_add(1, Ordering::SeqCst);
}

fn spawn_stealers(
    deque: &Arc<Deque>,
    flags: &Flags,
    duplicates: &Arc<AtomicUsize>,
    stolen: &Arc<AtomicUsize>,
    done: &Arc<AtomicBool>,
    count: usize,
) -> Vec<std::thread::JoinHandle<()>> {
    let mut stealers = Vec::with_capacity(count);
    for _ in 0..count {
        let dq = Arc::clone(deque);
        let fl = Arc::clone(flags);
        let dup = Arc::clone(duplicates);
        let st = Arc::clone(stolen);
        let dn = Arc::clone(done);
        stealers.push(std::thread::spawn(move || {
            loop {
                match dq.steal() {
                    Steal::Success(task) => {
                        if fl[task as usize].swap(true, Ordering::SeqCst) {
                            dup.fetch_add(1, Ordering::SeqCst);
                        }
                        st.fetch_add(1, Ordering::SeqCst);
                    }
                    Steal::Abort => {
                        // Lost a CAS race with the owner or another stealer:
                        // retry immediately, the item is owned by someone else.
                        std::thread::yield_now();
                    }
                    Steal::Empty => {
                        if dn.load(Ordering::Acquire) {
                            break;
                        }
                        std::thread::yield_now();
                    }
                }
            }
        }));
    }
    stealers
}

// ---------------------------------------------------------------------------
// Test 1: 2048 tasks, 256-slot ring, 4 concurrent stealers, owner draining
// from the bottom. Every task must be delivered exactly once.
// ---------------------------------------------------------------------------
#[test]
fn test_exactly_once_delivery_under_contention() {
    const TASKS: usize = 2048;
    let deque = Arc::new(Deque::new(256));
    let flags: Flags = Arc::new((0..TASKS).map(|_| AtomicBool::new(false)).collect());
    let duplicates = Arc::new(AtomicUsize::new(0));
    let stolen = Arc::new(AtomicUsize::new(0));
    let popped = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let stealers = spawn_stealers(&deque, &flags, &duplicates, &stolen, &done, 4);

    for i in 0..TASKS as u64 {
        while deque.push(i).is_err() {
            // Ring momentarily full: the owner makes room by popping.
            match deque.pop() {
                Some(task) => record_once(&flags, &duplicates, &popped, task),
                None => std::thread::yield_now(),
            }
        }
    }
    // Final owner-side drain from the bottom while stealers race from the top.
    while let Some(task) = deque.pop() {
        record_once(&flags, &duplicates, &popped, task);
    }
    done.store(true, Ordering::Release);

    for s in stealers {
        s.join().expect("stealer thread panicked");
    }

    assert_eq!(
        duplicates.load(Ordering::SeqCst),
        0,
        "a task was delivered more than once"
    );
    assert_eq!(
        popped.load(Ordering::SeqCst) + stolen.load(Ordering::SeqCst),
        TASKS,
        "exactly-once delivery violated: tasks were lost or duplicated"
    );
    assert!(deque.is_empty(), "deque must be empty after a full drain");
}

// ---------------------------------------------------------------------------
// Test 2: the pop-vs-steal CAS window. Each round leaves exactly one item in
// the deque, then owner and stealer race for it through the SeqCst CAS path.
// The task must never vanish and never be handed out twice.
// ---------------------------------------------------------------------------
#[test]
fn test_last_item_cas_race_single_winner() {
    const ROUNDS: usize = 400;
    for round in 0..ROUNDS {
        let deque = Arc::new(Deque::new(8));
        let winner = Arc::new(AtomicUsize::new(0)); // bit 1: owner, bit 2: stealer
        let barrier = Arc::new(Barrier::new(2));

        let owner_dq = Arc::clone(&deque);
        let owner_barrier = Arc::clone(&barrier);
        let owner_winner = Arc::clone(&winner);
        let owner = std::thread::spawn(move || {
            owner_dq
                .push(round as u64)
                .expect("single push into empty deque");
            owner_barrier.wait();
            if owner_dq.pop().is_some() {
                owner_winner.fetch_or(1, Ordering::SeqCst);
            }
        });

        let stealer_dq = Arc::clone(&deque);
        let stealer_barrier = Arc::clone(&barrier);
        let stealer_winner = Arc::clone(&winner);
        let stealer = std::thread::spawn(move || {
            stealer_barrier.wait();
            loop {
                match stealer_dq.steal() {
                    Steal::Success(_) => {
                        stealer_winner.fetch_or(2, Ordering::SeqCst);
                        break;
                    }
                    Steal::Abort => continue,
                    Steal::Empty => break,
                }
            }
        });

        owner.join().expect("owner panicked");
        stealer.join().expect("stealer panicked");

        let w = winner.load(Ordering::SeqCst);
        assert!(
            w == 1 || w == 2,
            "round {}: exactly one racer must win, got winner bits {}",
            round,
            w
        );
        assert_eq!(deque.len(), 0, "round {}: deque must be empty", round);
    }
}

// ---------------------------------------------------------------------------
// Test 3: ring wraparound. 512 tasks through a 64-slot ring with live
// stealers forces the monotonic counters far past `capacity`, exercising the
// mask-based slot mapping on every wrap (ABA-proof index design).
// ---------------------------------------------------------------------------
#[test]
fn test_ring_index_wraparound_exactly_once() {
    const TASKS: usize = 512;
    const CAPACITY: usize = 64;
    let deque = Arc::new(Deque::new(CAPACITY));
    let flags: Flags = Arc::new((0..TASKS).map(|_| AtomicBool::new(false)).collect());
    let duplicates = Arc::new(AtomicUsize::new(0));
    let stolen = Arc::new(AtomicUsize::new(0));
    let popped = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let stealers = spawn_stealers(&deque, &flags, &duplicates, &stolen, &done, 2);

    let mut pushed = 0u64;
    while (pushed as usize) < TASKS {
        if deque.push(pushed).is_ok() {
            pushed += 1;
        } else {
            match deque.pop() {
                Some(task) => record_once(&flags, &duplicates, &popped, task),
                None => std::thread::yield_now(),
            }
        }
    }
    while let Some(task) = deque.pop() {
        record_once(&flags, &duplicates, &popped, task);
    }
    done.store(true, Ordering::Release);

    for s in stealers {
        s.join().expect("stealer thread panicked");
    }

    assert_eq!(
        duplicates.load(Ordering::SeqCst),
        0,
        "duplicate delivery after wrap"
    );
    assert_eq!(
        popped.load(Ordering::SeqCst) + stolen.load(Ordering::SeqCst),
        TASKS,
        "lost or duplicated tasks after index wraparound"
    );
    assert!(
        deque.is_empty(),
        "deque must be empty after wraparound drain"
    );
}

// ---------------------------------------------------------------------------
// Test 4: saturation reporting and post-drain reusability.
// ---------------------------------------------------------------------------
#[test]
fn test_capacity_saturation_is_reported_not_lost() {
    let deque = Deque::new(4);
    for i in 0..4u64 {
        assert!(
            deque.push(i).is_ok(),
            "push {} into non-full deque must succeed",
            i
        );
    }
    assert!(
        deque.push(99).is_err(),
        "push into a saturated deque must report Err, not wrap or drop"
    );
    assert_eq!(
        deque.len(),
        4,
        "saturated deque must still report its capacity worth of items"
    );

    let mut drained = Vec::new();
    while let Some(task) = deque.pop() {
        drained.push(task);
    }
    assert_eq!(drained.len(), 4);
    assert!(deque.is_empty());
    assert_eq!(
        deque.steal(),
        Steal::Empty,
        "steal on drained deque must be Empty"
    );

    // Reusability after a full drain: slots must be writable again.
    assert!(deque.push(7).is_ok(), "push after drain must succeed");
    assert_eq!(
        deque.pop(),
        Some(7),
        "post-drain pop must return the pushed task"
    );
}
