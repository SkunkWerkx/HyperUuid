//go:build !tinygo

package hyperuuid

import (
	"bytes"
	"runtime"
	"sync"
	"testing"
	"time"
)

func TestStatelessCallsRunWhileThePrimaryIsHeld(t *testing.T) {
	base := withFreshInstances(t) // the pool is empty: nothing but a new instance can serve
	p := primary()
	p.mu.Lock()
	locked := true
	var wg sync.WaitGroup
	t.Cleanup(func() {
		if locked {
			p.mu.Unlock()
		}
		wg.Wait()
	})
	done := make(chan error, 1)
	wg.Add(1)
	go func() {
		defer wg.Done()
		_, err := NewV4()
		done <- err
	}()
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("NewV4 did not return while the primary was held: no pooled instance served it")
	}
	p.mu.Unlock()
	locked = false
	if d := instancesCreated.Load() - base; d != 1 {
		t.Fatalf("instances created while the primary was held: %d, want 1", d)
	}
}

func TestUncontendedCallsStayOnThePrimary(t *testing.T) {
	base := withFreshInstances(t)
	old := runtime.GOMAXPROCS(1)
	t.Cleanup(func() { runtime.GOMAXPROCS(old) })
	for range 1000 {
		if _, err := NewV4(); err != nil {
			t.Fatal(err)
		}
	}
	if d := instancesCreated.Load() - base; d != 0 {
		t.Fatalf("uncontended calls created %d instances, want 0", d)
	}
}

func TestGrownInstanceIsNotPooled(t *testing.T) {
	base := withFreshInstances(t)
	name := bytes.Repeat([]byte{7}, 65505)
	for range 2 {
		if _, err := NewV5(NamespaceOID, name); err != nil {
			t.Fatal(err)
		}
	}
	if d := instancesCreated.Load() - base; d != 2 {
		t.Fatalf("two v5 calls past the base page created %d instances, want 2 (one-off each)", d)
	}
}
