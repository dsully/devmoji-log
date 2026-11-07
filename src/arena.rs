//! Bump allocator for libgit2's small allocations.
//!
//! A run makes ~4k allocations of at most 256 bytes (mostly config entries copied for snapshots
//! and iterators) and frees them all again; bumping a pointer and never freeing is cheaper than
//! malloc/free for a process that exits right after. Larger blocks, and anything past the arena,
//! go to the system allocator.

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};

const SMALL: usize = 256;
/// Arena unit: one 16-byte slot, the alignment malloc guarantees.
type Slot = u128;
const SLOTS: usize = (4 << 20) / size_of::<Slot>();

struct Arena(UnsafeCell<[Slot; SLOTS]>);

// SAFETY: disjoint slot ranges are handed out through the atomic `NEXT` cursor.
unsafe impl Sync for Arena {}

static ARENA: Arena = Arena(UnsafeCell::new([0; SLOTS]));
static NEXT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

#[repr(C)]
struct GitAllocator {
    gmalloc: extern "C" fn(usize, *const c_char, c_int) -> *mut c_void,
    grealloc: extern "C" fn(*mut c_void, usize, *const c_char, c_int) -> *mut c_void,
    gfree: extern "C" fn(*mut c_void),
}

static ALLOCATOR: GitAllocator = GitAllocator { gmalloc, grealloc, gfree };

/// Route libgit2's allocations through the arena. Must run before the first git2 call.
pub fn install() {
    // SAFETY: GIT_OPT_SET_ALLOCATOR copies the struct; libgit2 keeps a custom allocator set
    // before `git_libgit2_init`, so every allocation it ever makes goes through these functions.
    unsafe {
        libgit2_sys::git_libgit2_opts(libgit2_sys::GIT_OPT_SET_ALLOCATOR.cast_signed(), &raw const ALLOCATOR);
    }
}

fn base() -> *mut Slot {
    ARENA.0.get().cast()
}

fn in_arena(ptr: *mut c_void) -> bool {
    (ptr as usize).wrapping_sub(base() as usize) < SLOTS * size_of::<Slot>()
}

/// One header slot holding the requested size, then the payload slots.
fn bump(size: usize) -> Option<*mut c_void> {
    let need = 1 + size.div_ceil(size_of::<Slot>());
    let offset = NEXT.fetch_add(need, Ordering::Relaxed);
    if offset + need > SLOTS {
        return None;
    }
    // SAFETY: `offset..offset + need` is in bounds and handed out exactly once.
    unsafe {
        let header = base().add(offset);
        header.write(size as Slot);
        Some(header.add(1).cast())
    }
}

extern "C" fn gmalloc(size: usize, _file: *const c_char, _line: c_int) -> *mut c_void {
    if size <= SMALL
        && let Some(ptr) = bump(size)
    {
        return ptr;
    }
    // SAFETY: plain libc malloc.
    unsafe { malloc(size) }
}

extern "C" fn grealloc(ptr: *mut c_void, size: usize, file: *const c_char, line: c_int) -> *mut c_void {
    if ptr.is_null() {
        return gmalloc(size, file, line);
    }
    if !in_arena(ptr) {
        // SAFETY: `ptr` came from libc malloc/realloc.
        return unsafe { realloc(ptr, size) };
    }
    // SAFETY: arena blocks carry their size in the header just before them.
    let old = unsafe { ptr.cast::<Slot>().sub(1).read() } as usize;
    if size <= old {
        return ptr;
    }
    let new = gmalloc(size, file, line);
    if !new.is_null() {
        // SAFETY: `old` bytes are readable at `ptr` and `new` has room for `size > old`.
        unsafe { std::ptr::copy_nonoverlapping(ptr.cast::<u8>(), new.cast::<u8>(), old) };
    }
    new
}

extern "C" fn gfree(ptr: *mut c_void) {
    if !ptr.is_null() && !in_arena(ptr) {
        // SAFETY: `ptr` came from libc malloc/realloc.
        unsafe { free(ptr) };
    }
}
