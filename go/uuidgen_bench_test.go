package hyperuuid

import "testing"

// Run with: go test -bench=. -benchmem ./...
// -benchmem is essentially free here — allocation tracking is built into testing.B, no
// extra tooling required, unlike the other bindings.

func BenchmarkNewV4(b *testing.B) {
	for b.Loop() {
		if _, err := NewV4(); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkNewV5String(b *testing.B) {
	for b.Loop() {
		if _, err := NewV5String(NamespaceDNS, "www.example.com"); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkNewV6At(b *testing.B) {
	for b.Loop() {
		if _, err := NewV6At(rfcTestVectorMs); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkNewV7At(b *testing.B) {
	for b.Loop() {
		if _, err := NewV7At(rfcTestVectorMs); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkV6IndividualX1000(b *testing.B) {
	for b.Loop() {
		for i := 0; i < 1000; i++ {
			if _, err := NewV6At(rfcTestVectorMs); err != nil {
				b.Fatal(err)
			}
		}
	}
}

func BenchmarkNewV6BatchAt1000(b *testing.B) {
	for b.Loop() {
		if _, err := NewV6BatchAt(1000, rfcTestVectorMs); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkV7IndividualX1000(b *testing.B) {
	for b.Loop() {
		for i := 0; i < 1000; i++ {
			if _, err := NewV7At(rfcTestVectorMs); err != nil {
				b.Fatal(err)
			}
		}
	}
}

func BenchmarkNewV7BatchAt1000(b *testing.B) {
	for b.Loop() {
		if _, err := NewV7BatchAt(1000, rfcTestVectorMs); err != nil {
			b.Fatal(err)
		}
	}
}
