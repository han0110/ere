//! C ABI for the unified [`Verifier`](ere_verifier::Verifier) of `ere-verifier`.
//!
//! Wraps the verifier in an opaque-handle C API. Each function returns an
//! [`i32`] status code from [`error`]; byte slices cross the boundary as
//! `(ptr, len)` pairs.
//!
//! `zkvm_kind` integer encoding (locked into the public ABI; matches the
//! declaration order of [`zkVMKind`](ere_verifier::zkVMKind)):
//!
//! - `0` = [`Airbender`](ere_verifier::zkVMKind::Airbender)
//! - `1` = [`OpenVM`](ere_verifier::zkVMKind::OpenVM)
//! - `2` = [`Risc0`](ere_verifier::zkVMKind::Risc0)
//! - `3` = [`SP1`](ere_verifier::zkVMKind::SP1)
//! - `4` = [`Zisk`](ere_verifier::zkVMKind::Zisk)
//!
//! # Thread safety
//!
//! Handles are not thread-safe. A single [`EreVerifier`] must not be passed
//! to two threads concurrently, and must not be freed while another thread is
//! using it. Independent handles in different threads are fine.
//!
//! # Usage from C
//!
//! ```c
//! EreVerifier *handle;
//! if (ere_verifier_new(kind, encoded_program_vk, encoded_program_vk_len, &handle) != ERE_OK) { ... }
//!
//! // Verify the proof AND that its public values start with `expected`
//! // (any bytes after the prefix must be zero).
//! if (ere_verifier_verify(handle,
//!                         encoded_proof, encoded_proof_len,
//!                         expected, expected_len) != ERE_OK) { ... }
//!
//! ere_verifier_free(handle);
//! ```

#![allow(non_camel_case_types)]

mod error;

use core::slice;

use ere_verifier::{Error as VerifierError, Verifier, zkVMKind};

pub use crate::error::{
    ERE_ERR_BAD_KIND, ERE_ERR_DECODE_PROGRAM_VK, ERE_ERR_DECODE_PROOF, ERE_ERR_NULL_PTR,
    ERE_ERR_PUBLIC_VALUES_MISMATCH, ERE_ERR_VERIFY, ERE_OK,
};

/// Opaque verifier handle returned by [`ere_verifier_new`] and released by
/// [`ere_verifier_free`].
pub struct EreVerifier {
    inner: Verifier,
}

fn kind_from_u32(value: u32) -> Option<zkVMKind> {
    match value {
        0 => Some(zkVMKind::Airbender),
        1 => Some(zkVMKind::OpenVM),
        2 => Some(zkVMKind::Risc0),
        3 => Some(zkVMKind::SP1),
        4 => Some(zkVMKind::Zisk),
        _ => None,
    }
}

fn kind_to_u32(kind: zkVMKind) -> u32 {
    match kind {
        zkVMKind::Airbender => 0,
        zkVMKind::OpenVM => 1,
        zkVMKind::Risc0 => 2,
        zkVMKind::SP1 => 3,
        zkVMKind::Zisk => 4,
    }
}

/// Reconstructs a `&[u8]` from a `(ptr, len)` pair, treating `(NULL, 0)` as
/// the empty slice. Returns `None` when `ptr` is null but `len > 0`, which is
/// caller error.
///
/// # Safety
///
/// When `ptr` is non-null, it must point to `len` initialised, readable bytes.
unsafe fn as_slice<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        Some(&[][..])
    } else if ptr.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(ptr, len) })
    }
}

/// Constructs a verifier bound to an encoded program verifying key.
///
/// On success, writes the new handle into `*out_handle` and returns [`ERE_OK`].
/// The caller owns the handle and must release it with [`ere_verifier_free`].
/// On error, `*out_handle` is set to null and the corresponding status code is
/// returned.
///
/// # Safety
///
/// - `encoded_program_vk` must point to `encoded_program_vk_len` readable bytes (or be null when
///   `encoded_program_vk_len == 0`).
/// - `out_handle` must be a non-null, writable `*mut *mut EreVerifier`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_new(
    zkvm_kind: u32,
    encoded_program_vk: *const u8,
    encoded_program_vk_len: usize,
    out_handle: *mut *mut EreVerifier,
) -> i32 {
    if out_handle.is_null() {
        return ERE_ERR_NULL_PTR;
    }
    unsafe { *out_handle = core::ptr::null_mut() };

    let Some(kind) = kind_from_u32(zkvm_kind) else {
        return ERE_ERR_BAD_KIND;
    };
    let Some(encoded_program_vk) =
        (unsafe { as_slice(encoded_program_vk, encoded_program_vk_len) })
    else {
        return ERE_ERR_NULL_PTR;
    };

    match Verifier::new(kind, encoded_program_vk) {
        Ok(verifier) => {
            let boxed = Box::new(EreVerifier { inner: verifier });
            unsafe { *out_handle = Box::into_raw(boxed) };
            ERE_OK
        }
        Err(VerifierError::DecodeProgramVk(_)) => ERE_ERR_DECODE_PROGRAM_VK,
        // NightlyFeatureRequired is unreachable while the workspace pulls
        // `ere-verifier` with the `nightly` feature, but fold it into the
        // bad-kind bucket if the feature is ever dropped.
        Err(_) => ERE_ERR_BAD_KIND,
    }
}

/// Verifies a proof against the verifier's program key, then asserts that the
/// proven public values start with `expected_public_values`. Returns [`ERE_OK`]
/// when both checks pass.
///
/// The actual public-values byte slice produced by the verifier may be longer
/// than `expected_public_values_len`; the trailing bytes are allowed only when
/// they are all zero (this accommodates proof systems whose public values are
/// padded to a fixed length).
///
/// Returns [`ERE_ERR_PUBLIC_VALUES_MISMATCH`] when the expected slice is
/// longer than the actual public values, is not a prefix of them, or any
/// byte after the prefix is non-zero.
///
/// # Safety
///
/// - `handle` must be a live handle returned by [`ere_verifier_new`].
/// - `encoded_proof` must point to `encoded_proof_len` readable bytes (or be null when
///   `encoded_proof_len == 0`).
/// - `expected_public_values` must point to `expected_public_values_len` readable bytes (or be null
///   when `expected_public_values_len == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_verify(
    handle: *const EreVerifier,
    encoded_proof: *const u8,
    encoded_proof_len: usize,
    expected_public_values: *const u8,
    expected_public_values_len: usize,
) -> i32 {
    if handle.is_null() {
        return ERE_ERR_NULL_PTR;
    }
    let Some(encoded_proof) = (unsafe { as_slice(encoded_proof, encoded_proof_len) }) else {
        return ERE_ERR_NULL_PTR;
    };
    let Some(expected) = (unsafe { as_slice(expected_public_values, expected_public_values_len) })
    else {
        return ERE_ERR_NULL_PTR;
    };

    let verifier = unsafe { &(*handle).inner };
    let verified = match verifier.verify(encoded_proof) {
        Ok(public_values) => public_values,
        Err(VerifierError::DecodeProof(_)) => return ERE_ERR_DECODE_PROOF,
        Err(_) => return ERE_ERR_VERIFY,
    };

    let actual: Vec<u8> = verified.into();
    if expected.len() > actual.len() {
        return ERE_ERR_PUBLIC_VALUES_MISMATCH;
    }
    let (prefix, trailing) = actual.split_at(expected.len());
    if prefix != expected || trailing.iter().any(|&b| b != 0) {
        return ERE_ERR_PUBLIC_VALUES_MISMATCH;
    }
    ERE_OK
}

/// Returns the `zkvm_kind` integer the verifier was constructed for, or
/// `u32::MAX` if `handle` is null.
///
/// # Safety
///
/// `handle` must be a live handle returned by [`ere_verifier_new`] or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_zkvm_kind(handle: *const EreVerifier) -> u32 {
    if handle.is_null() {
        return u32::MAX;
    }
    kind_to_u32(unsafe { (*handle).inner.zkvm_kind() })
}

/// Releases a verifier handle. Subsequent use of the handle is undefined; a
/// null pointer is a no-op.
///
/// # Safety
///
/// `handle` must be a live handle returned by [`ere_verifier_new`] or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ere_verifier_free(handle: *mut EreVerifier) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}
