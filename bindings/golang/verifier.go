// Package ereverifier exposes the ere zkVM verifier to Go via cgo.
//
// The package links the staticlib produced by the `ere-binding-c` crate
// (`libere_binding_c.a`) and the cbindgen-generated header
// (`ere_verifier.h`). Both are produced by
// `bindings/scripts/compile.sh golang` and dropped under
// `bindings/golang/build/{target}/`.
//
// # Thread safety
//
// A single [Verifier] is not safe for concurrent use. Independent verifiers
// in different goroutines are fine.
package ereverifier

/*
#cgo CFLAGS: -I${SRCDIR}/build
#cgo linux,amd64   LDFLAGS: ${SRCDIR}/build/x86_64-unknown-linux-gnu/libere_binding_c.a   -lm -lpthread -ldl
#cgo linux,arm64   LDFLAGS: ${SRCDIR}/build/aarch64-unknown-linux-gnu/libere_binding_c.a  -lm -lpthread -ldl
#cgo darwin,amd64  LDFLAGS: ${SRCDIR}/build/x86_64-apple-darwin/libere_binding_c.a
#cgo darwin,arm64  LDFLAGS: ${SRCDIR}/build/aarch64-apple-darwin/libere_binding_c.a
#cgo windows,amd64 LDFLAGS: ${SRCDIR}/build/x86_64-pc-windows-gnu/libere_binding_c.a -lws2_32 -lntdll -luserenv -lbcrypt

#include "ere_verifier.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
	"unsafe"
)

// ZkVMKind is the integer discriminant the C ABI uses to pick a verifier.
// Values are part of the public ABI and must match the declaration order of
// the Rust `ere_verifier::zkVMKind` enum.
type ZkVMKind uint32

const (
	Airbender ZkVMKind = 0
	OpenVM    ZkVMKind = 1
	Risc0     ZkVMKind = 2
	SP1       ZkVMKind = 3
	Zisk      ZkVMKind = 4
)

// String implements [fmt.Stringer].
func (k ZkVMKind) String() string {
	switch k {
	case Airbender:
		return "airbender"
	case OpenVM:
		return "openvm"
	case Risc0:
		return "risc0"
	case SP1:
		return "sp1"
	case Zisk:
		return "zisk"
	default:
		return fmt.Sprintf("unknown(%d)", uint32(k))
	}
}

// Errors mapped from the integer status codes in `ere_verifier.h`.
var (
	ErrNullPtr              = errors.New("ere: null pointer")
	ErrBadKind              = errors.New("ere: unsupported zkvm_kind")
	ErrDecodeProgramVK      = errors.New("ere: failed to decode program verifying key")
	ErrDecodeProof          = errors.New("ere: failed to decode proof")
	ErrVerify               = errors.New("ere: proof failed verification")
	ErrPublicValuesMismatch = errors.New("ere: public values mismatch")
)

func statusToError(rc C.int32_t) error {
	switch rc {
	case C.ERE_OK:
		return nil
	case C.ERE_ERR_NULL_PTR:
		return ErrNullPtr
	case C.ERE_ERR_BAD_KIND:
		return ErrBadKind
	case C.ERE_ERR_DECODE_PROGRAM_VK:
		return ErrDecodeProgramVK
	case C.ERE_ERR_DECODE_PROOF:
		return ErrDecodeProof
	case C.ERE_ERR_VERIFY:
		return ErrVerify
	case C.ERE_ERR_PUBLIC_VALUES_MISMATCH:
		return ErrPublicValuesMismatch
	default:
		return fmt.Errorf("ere: unknown error code %d", int32(rc))
	}
}

// bytePtr returns the address of the first element of buf as `*C.uint8_t`,
// or nil for the empty slice. Required because indexing `buf[0]` panics on
// the empty slice.
func bytePtr(buf []byte) *C.uint8_t {
	if len(buf) == 0 {
		return nil
	}
	return (*C.uint8_t)(unsafe.Pointer(&buf[0]))
}

// Verifier is a handle to a zkVM verifier bound to a specific program
// verifying key. Construct with [New]; release with [Verifier.Close].
type Verifier struct {
	handle *C.EreVerifier
}

// New constructs a verifier bound to encodedProgramVK. The returned handle is
// released either explicitly via [Verifier.Close] or by the runtime finalizer.
func New(kind ZkVMKind, encodedProgramVK []byte) (*Verifier, error) {
	var handle *C.EreVerifier
	rc := C.ere_verifier_new(
		C.uint32_t(kind),
		bytePtr(encodedProgramVK), C.uintptr_t(len(encodedProgramVK)),
		&handle,
	)
	// Keep the backing array of encodedProgramVK alive until after the C call
	// returns: cgo's pointer rules guarantee the slice header isn't moved, but
	// not its underlying storage.
	runtime.KeepAlive(encodedProgramVK)
	if err := statusToError(rc); err != nil {
		return nil, err
	}
	v := &Verifier{handle: handle}
	runtime.SetFinalizer(v, func(x *Verifier) { x.Close() })
	return v, nil
}

// Close releases the underlying verifier. Safe to call more than once; safe
// on a nil receiver.
func (v *Verifier) Close() {
	if v == nil || v.handle == nil {
		return
	}
	C.ere_verifier_free(v.handle)
	v.handle = nil
	runtime.SetFinalizer(v, nil)
}

// Kind returns the zkVM the verifier was constructed for. For a nil receiver
// or a verifier that has been [Verifier.Close]d, returns
// `ZkVMKind(math.MaxUint32)`, matching the underlying C ABI sentinel.
func (v *Verifier) Kind() ZkVMKind {
	if v == nil || v.handle == nil {
		return ZkVMKind(^uint32(0))
	}
	return ZkVMKind(C.ere_verifier_zkvm_kind(v.handle))
}

// Verify checks encodedProof against the verifier's program verifying key
// and asserts the proven public values start with expectedPublicValues. The
// actual public-values byte slice may be longer than expectedPublicValues as
// long as the trailing bytes are all zero, which accommodates proof systems
// that pad public values to a fixed size.
//
// Returns [ErrPublicValuesMismatch] when expectedPublicValues is longer than
// the actual public values, is not a prefix of them, or any byte after the
// prefix is non-zero.
func (v *Verifier) Verify(encodedProof []byte, expectedPublicValues []byte) error {
	if v == nil || v.handle == nil {
		return ErrNullPtr
	}
	rc := C.ere_verifier_verify(
		v.handle,
		bytePtr(encodedProof), C.uintptr_t(len(encodedProof)),
		bytePtr(expectedPublicValues), C.uintptr_t(len(expectedPublicValues)),
	)
	runtime.KeepAlive(encodedProof)
	runtime.KeepAlive(expectedPublicValues)
	return statusToError(rc)
}
