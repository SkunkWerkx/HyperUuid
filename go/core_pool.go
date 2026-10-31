//go:build !tinygo

package hyperuuid

import "sync"

// The pool of instances for stateless calls (v4, v5, inspection, the SQL-order transforms):
// the primary when its lock is free, else an instance from a sync.Pool whose New creates
// one. sync.Pool bounds nothing: it is per-P with a victim cache cleared across two GCs, and
// a goroutine preempted inside a call holds its instance meanwhile, so the live count tracks
// the goroutines concurrently inside stateless calls, approximately GOMAXPROCS. An instance
// returns to the pool only while its scratch is still the base page. Instances in the pool
// never run a v7 call, so their counters are never used.

var pool = newPool()

func newPool() *sync.Pool {
	return &sync.Pool{New: func() any {
		in, ok := newInstance(basePages)
		if !ok {
			panic("hyperuuid: the core's memory cannot grow by one page on this host")
		}
		return in
	}}
}

// resetPool empties the pool: a test hook, called with no call in flight.
func resetPool() {
	pool = newPool()
}

// acquire returns an instance for one stateless call: the primary, locked, when it is
// uncontended, else a pooled one. pooled says which, for release.
func acquire() (in *instance, pooled bool) {
	p := primary()
	if p.mu.TryLock() {
		return p, false
	}
	return pool.Get().(*instance), true
}

// release gives the instance back: the primary's lock, or the pool when the instance is
// still base-sized.
func release(in *instance, pooled bool) {
	if !pooled {
		in.mu.Unlock()
		return
	}
	if in.pages == basePages {
		pool.Put(in)
	}
}
