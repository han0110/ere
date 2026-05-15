//! Integration tests for the C ABI. Drives the same `extern "C"` symbols
//! every foreign-language binding calls, against the per-zkVM fixtures
//! vendored under `crates/verifier/{zkvm}/tests/fixtures/`. Missing fixture
//! files cause the test to fail loudly: the fixtures are tracked in git, so
//! a missing file indicates repo corruption or an unexpected sparse
//! checkout.

use std::{fs, path::PathBuf, ptr};

use ere_binding_c::{
    ERE_ERR_BAD_KIND, ERE_ERR_NULL_PTR, ERE_ERR_PUBLIC_VALUES_MISMATCH, ERE_OK, EreVerifier,
    ere_verifier_free, ere_verifier_new, ere_verifier_verify, ere_verifier_zkvm_kind,
};

struct Case {
    name: &'static str,
    kind: u32,
}

const CASES: &[Case] = &[
    Case {
        name: "airbender",
        kind: 0,
    },
    Case {
        name: "openvm",
        kind: 1,
    },
    Case {
        name: "risc0",
        kind: 2,
    },
    Case {
        name: "sp1",
        kind: 3,
    },
    Case {
        name: "zisk",
        kind: 4,
    },
];

fn fixture_dir(case: &Case) -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.join("crates/verifier")
        .join(case.name)
        .join("tests/fixtures")
}

fn run_case(case: &Case) {
    let dir = fixture_dir(case);
    let encoded_program_vk = fs::read(dir.join("program_vk.bin")).expect("read program_vk.bin");
    let encoded_proof = fs::read(dir.join("proof.bin")).expect("read proof.bin");
    let expected_public_values =
        fs::read(dir.join("public_values.bin")).expect("read public_values.bin");

    let mut handle: *mut EreVerifier = ptr::null_mut();
    let rc = unsafe {
        ere_verifier_new(
            case.kind,
            encoded_program_vk.as_ptr(),
            encoded_program_vk.len(),
            &mut handle,
        )
    };
    assert_eq!(rc, ERE_OK, "[{}] ere_verifier_new", case.name);
    assert!(
        !handle.is_null(),
        "[{}] handle is null after new",
        case.name
    );

    assert_eq!(
        unsafe { ere_verifier_zkvm_kind(handle) },
        case.kind,
        "[{}] zkvm_kind round-trip",
        case.name
    );

    let rc = unsafe {
        ere_verifier_verify(
            handle,
            encoded_proof.as_ptr(),
            encoded_proof.len(),
            expected_public_values.as_ptr(),
            expected_public_values.len(),
        )
    };
    assert_eq!(rc, ERE_OK, "[{}] ere_verifier_verify", case.name);

    unsafe { ere_verifier_free(handle) };
}

#[test]
fn all_zkvm_fixtures_verify() {
    for case in CASES {
        run_case(case);
    }
}

#[test]
fn bad_kind_rejected() {
    let mut handle: *mut EreVerifier = ptr::null_mut();
    let rc = unsafe { ere_verifier_new(999, ptr::null(), 0, &mut handle) };
    assert_eq!(rc, ERE_ERR_BAD_KIND);
    assert!(handle.is_null());
}

#[test]
fn null_out_handle_rejected() {
    let rc = unsafe { ere_verifier_new(0, ptr::null(), 0, ptr::null_mut()) };
    assert_eq!(rc, ERE_ERR_NULL_PTR);
}

#[test]
fn wrong_public_values_rejected() {
    // Flip the first byte of the expected public values; verify must reject.
    let case = &CASES[0]; // airbender
    let dir = fixture_dir(case);
    let encoded_program_vk = fs::read(dir.join("program_vk.bin")).expect("read program_vk.bin");
    let encoded_proof = fs::read(dir.join("proof.bin")).expect("read proof.bin");
    let mut wrong_public_values =
        fs::read(dir.join("public_values.bin")).expect("read public_values.bin");
    wrong_public_values[0] ^= 0xff;

    let mut handle: *mut EreVerifier = ptr::null_mut();
    let rc = unsafe {
        ere_verifier_new(
            case.kind,
            encoded_program_vk.as_ptr(),
            encoded_program_vk.len(),
            &mut handle,
        )
    };
    assert_eq!(rc, ERE_OK);

    let rc = unsafe {
        ere_verifier_verify(
            handle,
            encoded_proof.as_ptr(),
            encoded_proof.len(),
            wrong_public_values.as_ptr(),
            wrong_public_values.len(),
        )
    };
    assert_eq!(rc, ERE_ERR_PUBLIC_VALUES_MISMATCH);

    unsafe { ere_verifier_free(handle) };
}

#[test]
fn expected_longer_than_actual_rejected() {
    // Appending a non-zero byte to the expected public values makes it longer
    // than the actual; verify must reject.
    let case = &CASES[0]; // airbender
    let dir = fixture_dir(case);
    let encoded_program_vk = fs::read(dir.join("program_vk.bin")).expect("read program_vk.bin");
    let encoded_proof = fs::read(dir.join("proof.bin")).expect("read proof.bin");
    let mut too_long = fs::read(dir.join("public_values.bin")).expect("read public_values.bin");
    too_long.push(0x01);

    let mut handle: *mut EreVerifier = ptr::null_mut();
    let rc = unsafe {
        ere_verifier_new(
            case.kind,
            encoded_program_vk.as_ptr(),
            encoded_program_vk.len(),
            &mut handle,
        )
    };
    assert_eq!(rc, ERE_OK);

    let rc = unsafe {
        ere_verifier_verify(
            handle,
            encoded_proof.as_ptr(),
            encoded_proof.len(),
            too_long.as_ptr(),
            too_long.len(),
        )
    };
    assert_eq!(rc, ERE_ERR_PUBLIC_VALUES_MISMATCH);

    unsafe { ere_verifier_free(handle) };
}

#[test]
fn free_null_is_noop() {
    unsafe { ere_verifier_free(ptr::null_mut()) };
}
