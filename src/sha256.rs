//! CommonCrypto SHA-256 for libgit2, which checksums every config file it reads.
//!
//! Without the `https` feature libgit2-sys compiles libgit2's portable SHA-256; these definitions
//! take the place of `hash/builtin.c` at link time so the hardware-accelerated libSystem one is used.

use std::ffi::{c_int, c_uchar, c_void};

/// `CC_SHA256_CTX`; it fits in libgit2's `git_hash_sha256_ctx` (rfc6234 `SHA256Context`, 116 bytes).
#[repr(C)]
pub struct Ctx {
    count: [u32; 2],
    hash: [u32; 8],
    wbuf: [u32; 16],
}

const _: () = assert!(size_of::<Ctx>() <= 116);

unsafe extern "C" {
    fn CC_SHA256_Init(c: *mut Ctx) -> c_int;
    fn CC_SHA256_Update(c: *mut Ctx, data: *const c_void, len: u32) -> c_int;
    fn CC_SHA256_Final(md: *mut c_uchar, c: *mut Ctx) -> c_int;
}

#[unsafe(no_mangle)]
extern "C" fn git_hash_sha256_global_init() -> c_int {
    0
}

#[unsafe(no_mangle)]
extern "C" fn git_hash_sha256_ctx_init(ctx: *mut Ctx) -> c_int {
    git_hash_sha256_init(ctx)
}

#[unsafe(no_mangle)]
extern "C" fn git_hash_sha256_ctx_cleanup(_ctx: *mut Ctx) {}

#[unsafe(no_mangle)]
extern "C" fn git_hash_sha256_init(ctx: *mut Ctx) -> c_int {
    // SAFETY: libgit2 passes a live context at least as large as `Ctx`.
    unsafe { CC_SHA256_Init(ctx) };
    0
}

#[unsafe(no_mangle)]
extern "C" fn git_hash_sha256_update(ctx: *mut Ctx, data: *const c_void, len: usize) -> c_int {
    let mut data = data.cast::<u8>();
    let mut len = len;
    while len > 0 {
        let chunk = u32::try_from(len).unwrap_or(u32::MAX);
        // SAFETY: `data..data + len` is readable per libgit2's contract; `chunk <= len`.
        unsafe {
            CC_SHA256_Update(ctx, data.cast(), chunk);
            data = data.add(chunk as usize);
        }
        len -= chunk as usize;
    }
    0
}

#[unsafe(no_mangle)]
extern "C" fn git_hash_sha256_final(out: *mut c_uchar, ctx: *mut Ctx) -> c_int {
    // SAFETY: libgit2 passes a 32-byte output buffer and a live context.
    unsafe { CC_SHA256_Final(out, ctx) };
    0
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_known_digest() {
        let mut ctx = std::mem::MaybeUninit::<super::Ctx>::uninit();
        let mut out = [0u8; 32];
        super::git_hash_sha256_init(ctx.as_mut_ptr());
        super::git_hash_sha256_update(ctx.as_mut_ptr(), b"abc".as_ptr().cast(), 3);
        super::git_hash_sha256_final(out.as_mut_ptr(), ctx.as_mut_ptr());
        let expected = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3,
            0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(out, expected);
    }
}
