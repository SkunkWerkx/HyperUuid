//go:build tinygo

package hyperuuid

import "testing"

func TestEverythingSerializesOnThePrimary(t *testing.T) {
	base := withFreshInstances(t)
	if _, err := NewV4(); err != nil {
		t.Fatal(err)
	}
	if _, err := NewV5(NamespaceDNS, make([]byte, 1024)); err != nil {
		t.Fatal(err)
	}
	u, err := NewV7()
	if err != nil {
		t.Fatal(err)
	}
	if Version(u) != 7 || Variant(u) != VariantRfc9562 {
		t.Fatalf("inspection of %s: version %d, variant %v", u, Version(u), Variant(u))
	}
	if d := instancesCreated.Load() - base; d != 0 {
		t.Fatalf("TinyGo created %d instances beyond the primary, want 0", d)
	}
}
