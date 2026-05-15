//! Status codes returned by the [`ere_verifier_*`](crate) C-ABI functions.

/// Operation succeeded.
pub const ERE_OK: i32 = 0;
/// A required pointer argument was null when a value was expected.
pub const ERE_ERR_NULL_PTR: i32 = 1;
/// `zkvm_kind` was not one of the documented values.
pub const ERE_ERR_BAD_KIND: i32 = 2;
/// The program verifying key bytes failed to decode.
pub const ERE_ERR_DECODE_PROGRAM_VK: i32 = 3;
/// The proof bytes failed to decode.
pub const ERE_ERR_DECODE_PROOF: i32 = 4;
/// The proof was well-formed but failed cryptographic verification.
pub const ERE_ERR_VERIFY: i32 = 5;
/// The proof verified but its public values did not match
/// `expected_public_values`: either the expected slice is longer than the
/// actual public values, the expected slice is not a prefix of the actual
/// public values, or there are non-zero bytes after the expected prefix.
pub const ERE_ERR_PUBLIC_VALUES_MISMATCH: i32 = 6;
