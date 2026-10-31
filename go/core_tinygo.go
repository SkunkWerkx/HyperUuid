//go:build tinygo

package hyperuuid

// Under TinyGo every call serializes on the primary: its scheduler is single-threaded, its
// sync.Mutex has no TryLock, and a second instance would buy nothing.

// resetPool is a no-op here: there is no pool.
func resetPool() {}

// acquire locks and returns the primary; pooled is always false.
func acquire() (in *instance, pooled bool) {
	p := primary()
	p.mu.Lock()
	return p, false
}

func release(in *instance, _ bool) {
	in.mu.Unlock()
}
