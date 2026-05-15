package ereverifier

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

// workspaceRoot walks up from this source file until it finds a Cargo.toml
// that declares `[workspace]`, returning that directory's path.
func workspaceRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	for dir := filepath.Dir(file); ; {
		candidate := filepath.Join(dir, "Cargo.toml")
		if data, err := os.ReadFile(candidate); err == nil && strings.Contains(string(data), "[workspace]") {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("could not locate workspace Cargo.toml")
		}
		dir = parent
	}
}

type fixture struct {
	name string
	kind ZkVMKind
}

var fixtures = []fixture{
	{"airbender", Airbender},
	{"openvm", OpenVM},
	{"risc0", Risc0},
	{"sp1", SP1},
	{"zisk", Zisk},
}

func fixtureDir(root string, fx fixture) string {
	return filepath.Join(root, "crates", "verifier", fx.name, "tests", "fixtures")
}

// Fixture files are tracked in git, so a missing file indicates repo
// corruption or an unexpected sparse checkout -- fail loudly rather than skip.
func TestAllZkvmFixturesVerify(t *testing.T) {
	root := workspaceRoot(t)
	for _, fx := range fixtures {
		t.Run(fx.name, func(t *testing.T) {
			dir := fixtureDir(root, fx)
			encodedProgramVK := mustRead(t, filepath.Join(dir, "program_vk.bin"))
			encodedProof := mustRead(t, filepath.Join(dir, "proof.bin"))
			expectedPublicValues := mustRead(t, filepath.Join(dir, "public_values.bin"))

			v, err := New(fx.kind, encodedProgramVK)
			if err != nil {
				t.Fatalf("New: %v", err)
			}
			defer v.Close()

			if got := v.Kind(); got != fx.kind {
				t.Fatalf("Kind() = %v, want %v", got, fx.kind)
			}
			if err := v.Verify(encodedProof, expectedPublicValues); err != nil {
				t.Fatalf("Verify: %v", err)
			}
		})
	}
}

func TestBadKindRejected(t *testing.T) {
	_, err := New(ZkVMKind(999), nil)
	if !errors.Is(err, ErrBadKind) {
		t.Fatalf("expected ErrBadKind, got %v", err)
	}
}

func TestWrongPublicValuesRejected(t *testing.T) {
	root := workspaceRoot(t)
	dir := fixtureDir(root, fixtures[0]) // airbender
	encodedProgramVK := mustRead(t, filepath.Join(dir, "program_vk.bin"))
	encodedProof := mustRead(t, filepath.Join(dir, "proof.bin"))
	wrongPublicValues := mustRead(t, filepath.Join(dir, "public_values.bin"))
	wrongPublicValues[0] ^= 0xff

	v, err := New(Airbender, encodedProgramVK)
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer v.Close()

	if err := v.Verify(encodedProof, wrongPublicValues); !errors.Is(err, ErrPublicValuesMismatch) {
		t.Fatalf("expected ErrPublicValuesMismatch, got %v", err)
	}
}

func TestExpectedLongerThanActualRejected(t *testing.T) {
	root := workspaceRoot(t)
	dir := fixtureDir(root, fixtures[0]) // airbender
	encodedProgramVK := mustRead(t, filepath.Join(dir, "program_vk.bin"))
	encodedProof := mustRead(t, filepath.Join(dir, "proof.bin"))
	tooLong := append(mustRead(t, filepath.Join(dir, "public_values.bin")), 0x01)

	v, err := New(Airbender, encodedProgramVK)
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer v.Close()

	if err := v.Verify(encodedProof, tooLong); !errors.Is(err, ErrPublicValuesMismatch) {
		t.Fatalf("expected ErrPublicValuesMismatch, got %v", err)
	}
}

func TestCloseNilReceiverIsNoop(t *testing.T) {
	var v *Verifier
	v.Close()
}

func mustRead(t *testing.T, path string) []byte {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return b
}
