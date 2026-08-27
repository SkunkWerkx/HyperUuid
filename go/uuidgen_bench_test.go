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

// Extraction benchmarks: google/uuid is already a real dependency here and has its own
// timestamp-extraction method (uuid.UUID.Time(), documented as defined for versions 1, 2, 6,
// and 7) — these measure HyperUuid's V6Timestamp/V7Timestamp head-to-head against it on the
// same pre-generated UUID, generation excluded from the timed loop.

func BenchmarkV6TimestampExtraction(b *testing.B) {
	id, err := NewV6At(rfcTestVectorMs)
	if err != nil {
		b.Fatal(err)
	}
	for b.Loop() {
		if _, err := V6Timestamp(id); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkV7TimestampExtraction(b *testing.B) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		b.Fatal(err)
	}
	for b.Loop() {
		if _, err := V7Timestamp(id); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkGoogleUuidV6TimeExtraction(b *testing.B) {
	id, err := NewV6At(rfcTestVectorMs)
	if err != nil {
		b.Fatal(err)
	}
	for b.Loop() {
		_ = id.Time()
	}
}

func BenchmarkGoogleUuidV7TimeExtraction(b *testing.B) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		b.Fatal(err)
	}
	for b.Loop() {
		_ = id.Time()
	}
}
