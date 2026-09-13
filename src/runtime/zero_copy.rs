//! Zero-Copy Memory Primitives: Span<T> and StrView
//!
//! Provides stack-allocated, non-owning fat pointers `(ptr, len)` for strings
//! and homogeneous contiguous collections.
//!
//! Features:
//! - Zero heap allocation (no malloc/free, no refcount manipulation)
//! - Constant-time O(1) sub-slicing
//! - Zero-cost conversion from Datara native arrays and string literals
//! - 100% interoperable with C and LLVM pointer conventions

use std::fmt;
use std::ops::Index;

/// A non-owning contiguous view of elements of type `T`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Span<T> {
    pub ptr: *const T,
    pub len: usize,
}

unsafe impl<T: Sync> Sync for Span<T> {}
unsafe impl<T: Send> Send for Span<T> {}

impl<T> Span<T> {
    #[inline(always)]
    pub const fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    #[inline(always)]
    pub const fn from_slice(slice: &[T]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            len: slice.len(),
        }
    }

    #[inline(always)]
    pub const fn from_raw_parts(ptr: *const T, len: usize) -> Self {
        Self { ptr, len }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    #[inline(always)]
    pub fn subspan(&self, start: usize, end: usize) -> Option<Self> {
        if start <= end && end <= self.len {
            Some(Self {
                ptr: unsafe { self.ptr.add(start) },
                len: end - start,
            })
        } else {
            None
        }
    }

    #[inline(always)]
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        unsafe { &*self.ptr.add(index) }
    }
}

impl<T> Index<usize> for Span<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        assert!(
            index < self.len,
            "Span index out of bounds: {} >= {}",
            index,
            self.len
        );
        unsafe { &*self.ptr.add(index) }
    }
}

/// A zero-copy string slice pointing directly into memory without allocation.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrView {
    pub ptr: *const u8,
    pub len: usize,
}

unsafe impl Sync for StrView {}
unsafe impl Send for StrView {}

impl StrView {
    #[inline(always)]
    pub const fn from_static(s: &'static str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    #[inline(always)]
    pub fn from_str(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        if self.ptr.is_null() || self.len == 0 {
            ""
        } else {
            unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.ptr, self.len)) }
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn substr(&self, start: usize, end: usize) -> Option<Self> {
        if start <= end && end <= self.len {
            Some(Self {
                ptr: unsafe { self.ptr.add(start) },
                len: end - start,
            })
        } else {
            None
        }
    }
}

impl fmt::Debug for StrView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl fmt::Display for StrView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
