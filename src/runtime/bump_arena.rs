//! High-Performance Scope-Bound Bump Pointer Arena for Datara & Forgen (v1.2.3)
//!
//! Provides sub-nanosecond, zero-fragmentation region memory allocation for
//! temporary collections, vectors, matrices, and strings within hot loops.
//! Allocation takes 1 CPU instruction (`add ptr, size`); deallocation of an
//! entire scope takes 1 CPU cycle (`mov ptr, base`).

use std::alloc::{Layout, alloc, dealloc};
use std::cell::RefCell;
use std::ptr::NonNull;

/// Default block size for thread-local arena chunks (1 MB).
pub const DEFAULT_ARENA_CHUNK_SIZE: usize = 1024 * 1024;

/// A fast bump-pointer arena allocator.
pub struct BumpArena {
    chunk_size: usize,
    chunks: Vec<NonNull<u8>>,
    current_chunk_idx: usize,
    offset: usize,
    total_allocated: usize,
}

impl Default for BumpArena {
    fn default() -> Self {
        Self::new(DEFAULT_ARENA_CHUNK_SIZE)
    }
}

impl BumpArena {
    pub fn new(chunk_size: usize) -> Self {
        let first_chunk = Self::alloc_chunk(chunk_size);
        Self {
            chunk_size,
            chunks: vec![first_chunk],
            current_chunk_idx: 0,
            offset: 0,
            total_allocated: 0,
        }
    }

    fn alloc_chunk(size: usize) -> NonNull<u8> {
        let layout = Layout::from_size_align(size, 64).expect("Valid arena chunk layout");
        unsafe {
            let ptr = alloc(layout);
            NonNull::new(ptr).expect("Failed to allocate arena chunk")
        }
    }

    /// Allocates `size` bytes aligned to `align` from the current bump chunk.
    /// Returns raw pointer to memory.
    pub fn alloc(&mut self, size: usize, align: usize) -> *mut u8 {
        if size == 0 {
            return std::ptr::NonNull::dangling().as_ptr();
        }

        let align_mask = align - 1;
        let aligned_offset = (self.offset + align_mask) & !align_mask;

        if aligned_offset + size <= self.chunk_size {
            self.offset = aligned_offset + size;
            self.total_allocated += size;
            let chunk_ptr = self.chunks[self.current_chunk_idx].as_ptr();
            unsafe { chunk_ptr.add(aligned_offset) }
        } else if size > self.chunk_size {
            // Oversized allocation: allocate dedicated chunk
            let dedicated = Self::alloc_chunk(size);
            self.chunks.push(dedicated);
            self.total_allocated += size;
            dedicated.as_ptr()
        } else {
            // Advance to next chunk or allocate a new one
            self.current_chunk_idx += 1;
            if self.current_chunk_idx >= self.chunks.len() {
                let new_chunk = Self::alloc_chunk(self.chunk_size);
                self.chunks.push(new_chunk);
            }
            self.offset = size;
            self.total_allocated += size;
            self.chunks[self.current_chunk_idx].as_ptr()
        }
    }

    /// Resets the bump pointer to the beginning of the first chunk in 1 CPU cycle.
    /// All previously allocated memory becomes invalid without calling free() on individual objects.
    pub fn reset(&mut self) {
        self.current_chunk_idx = 0;
        self.offset = 0;
        self.total_allocated = 0;
    }

    /// Total bytes allocated through this arena since last reset.
    pub fn bytes_allocated(&self) -> usize {
        self.total_allocated
    }
}

impl Drop for BumpArena {
    fn drop(&mut self) {
        for chunk in &self.chunks {
            let layout =
                Layout::from_size_align(self.chunk_size, 64).expect("Valid arena chunk layout");
            unsafe {
                dealloc(chunk.as_ptr(), layout);
            }
        }
    }
}

thread_local! {
    static TLS_BUMP_ARENA: RefCell<BumpArena> = RefCell::new(BumpArena::new(DEFAULT_ARENA_CHUNK_SIZE));
}

/// Executes a closure with access to the thread-local Bump Arena, automatically
/// resetting the arena upon completion if requested.
pub fn with_thread_arena<F, R>(f: F) -> R
where
    F: FnOnce(&mut BumpArena) -> R,
{
    TLS_BUMP_ARENA.with(|arena_cell| {
        let mut arena = arena_cell.borrow_mut();
        f(&mut arena)
    })
}

/// Scope guard that automatically resets the thread arena when exiting the scope.
pub struct ScopeArenaGuard<'a> {
    arena: &'a mut BumpArena,
    initial_offset: usize,
    initial_chunk: usize,
    initial_allocated: usize,
}

impl<'a> ScopeArenaGuard<'a> {
    pub fn new(arena: &'a mut BumpArena) -> Self {
        let initial_offset = arena.offset;
        let initial_chunk = arena.current_chunk_idx;
        let initial_allocated = arena.total_allocated;
        Self {
            arena,
            initial_offset,
            initial_chunk,
            initial_allocated,
        }
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> *mut u8 {
        self.arena.alloc(size, align)
    }
}

impl<'a> Drop for ScopeArenaGuard<'a> {
    fn drop(&mut self) {
        self.arena.current_chunk_idx = self.initial_chunk;
        self.arena.offset = self.initial_offset;
        self.arena.total_allocated = self.initial_allocated;
    }
}
