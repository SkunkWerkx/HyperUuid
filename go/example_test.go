package hyperuuid_test

import (
	"errors"
	"fmt"
	"log"
	"time"

	"github.com/google/uuid"

	// The import path ends in /go, so the package name has to be spelled out.
	hyperuuid "github.com/SkunkWerkx/HyperUuid/go"
)

// RFC 9562 Appendix A.6's instant, 2022-02-22T19:22:22Z, as the explicit timestamp every
// time-based generator here accepts.
var appendixA6 = time.Date(2022, 2, 22, 19, 22, 22, 0, time.UTC)

func ExampleNewV5String() {
	// RFC 9562 Appendix A.4's test vector: deterministic, so the same pair always gives
	// the same UUID — in every binding of this core.
	id, err := hyperuuid.NewV5String(hyperuuid.NamespaceDNS, "www.example.com")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(id)
	// Output: 2ed6657d-e927-568b-95e1-2665a8aea6a2
}

func ExampleNewV5() {
	// A name is bytes, not text: nothing requires it to be valid UTF-8.
	id, err := hyperuuid.NewV5(hyperuuid.NamespaceOID, []byte{0x2b, 0x06, 0x01, 0xff})
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(id.Version(), id == uuid.NewSHA1(hyperuuid.NamespaceOID, []byte{0x2b, 0x06, 0x01, 0xff}))
	// Output: VERSION_5 true
}

func ExampleNewV7AtTime() {
	id, err := hyperuuid.NewV7AtTime(appendixA6)
	if err != nil {
		log.Fatal(err)
	}
	created, err := hyperuuid.V7Timestamp(id)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(id.Version(), created)
	// Output: VERSION_7 2022-02-22 19:22:22 +0000 UTC
}

func ExampleNewV7BatchAt() {
	// One native call, one timestamp capture and one contiguous block of the monotonic
	// counter: the batch comes back already in creation order.
	ids, err := hyperuuid.NewV7BatchAt(1000, uint64(appendixA6.UnixMilli()))
	if err != nil {
		log.Fatal(err)
	}
	sorted := true
	for i := 1; i < len(ids); i++ {
		if ids[i-1].String() >= ids[i].String() {
			sorted = false
		}
	}
	fmt.Println(len(ids), sorted)
	// Output: 1000 true
}

func ExampleFillV7() {
	// A destination the caller owns, reused across calls: nothing is allocated per batch.
	dst := make([]uuid.UUID, 4)
	for round := 0; round < 3; round++ {
		if err := hyperuuid.FillV7(dst); err != nil {
			log.Fatal(err)
		}
	}
	fmt.Println(len(dst), dst[0].Version())
	// Output: 4 VERSION_7
}

func ExampleFillV7BytesAt() {
	// Raw RFC 9562-ordered bytes, sixteen per UUID — a wire buffer or a database parameter.
	buf := make([]byte, 3*16)
	if err := hyperuuid.FillV7BytesAt(buf, uint64(appendixA6.UnixMilli())); err != nil {
		log.Fatal(err)
	}
	// Anything but a whole number of UUIDs is refused, not truncated.
	err := hyperuuid.FillV7BytesAt(buf[:20], uint64(appendixA6.UnixMilli()))
	fmt.Println(errors.Is(err, hyperuuid.ErrBufferNotWholeUUIDs))
	// Output: true
}

func ExampleGetTimestamp() {
	v6, err := hyperuuid.NewV6AtTime(appendixA6)
	if err != nil {
		log.Fatal(err)
	}
	created, err := hyperuuid.GetTimestamp(v6)
	fmt.Println(created, err)

	// Version-agnostic: a UUID with no embedded timestamp is an error, not a wrong answer.
	v4, err := hyperuuid.NewV4()
	if err != nil {
		log.Fatal(err)
	}
	_, err = hyperuuid.GetTimestamp(v4)
	fmt.Println(errors.Is(err, hyperuuid.ErrNotTimeBased))
	// Output:
	// 2022-02-22 19:22:22 +0000 UTC <nil>
	// true
}

func ExampleV7ToSqlOrder() {
	id, err := hyperuuid.NewV7AtTime(appendixA6)
	if err != nil {
		log.Fatal(err)
	}
	// The byte order SQL Server's uniqueidentifier needs to sort by creation order...
	sqlOrdered, err := hyperuuid.V7ToSqlOrder(id)
	if err != nil {
		log.Fatal(err)
	}
	// ...and back again when the value is read.
	roundTripped, err := hyperuuid.V7FromSqlOrder(sqlOrdered)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(sqlOrdered != id, roundTripped == id)
	// Output: true true
}

func ExampleNewV7At_outOfRange() {
	// Version 7 holds 48 bits of Unix milliseconds; one past that is an error, not a wrap.
	_, err := hyperuuid.NewV7At(1 << 48)
	fmt.Println(errors.Is(err, hyperuuid.ErrTimestampOutOfRange))
	// Output: true
}

func ExampleLoadError() {
	// At startup, before the first ID: nil means the native core loaded, and anything else
	// is the reason it did not — the same error every function here would return.
	if err := hyperuuid.LoadError(); err != nil {
		if errors.Is(err, hyperuuid.ErrNativeUnavailable) {
			log.Fatalf("no HyperUuid core on this platform: %v", err)
		}
	}
	fmt.Println(hyperuuid.Available())
	// Output: true
}

func ExampleNativeVersion() {
	// The version the loaded core reports about itself, "major.minor.patch".
	version, err := hyperuuid.NativeVersion()
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("hyperuuid core %s", version)
}
