// threadring (benchmarksgame) — Odin / core:thread + core:sync.
//
// 503 threads in a ring, each holding a mutex used as a baton.
// A node's run() blocks on its own mutex; the previous node's
// put() sets the value and unlocks it. When the value reaches 0
// the holder prints its label and exits the process.

package main

import "core:fmt"
import "core:os"
import "core:strconv"
import "core:sync"
import "core:thread"
import "core:time"

NTHREADS :: 503

T :: struct {
	next:  ^T,
	label: int,
	value: int,
	mux:   sync.Mutex,
}

channels: [NTHREADS]T

put :: proc(w: ^T, v: int) {
	w.value = v
	if v == 0 {
		fmt.println(w.label)
		os.exit(0)
	}
	sync.mutex_unlock(&w.mux)
}

run :: proc(w: ^T) {
	for {
		sync.mutex_lock(&w.mux)
		put(w.next, w.value - 1)
	}
}

main :: proc() {
	n := 1000
	if len(os.args) > 1 {
		if v, ok := strconv.parse_int(os.args[1]); ok {
			n = v
		}
	}

	for i in 0 ..< NTHREADS {
		channels[i].label = i + 1
		channels[i].next = &channels[(i + 1) % NTHREADS]
		sync.mutex_lock(&channels[i].mux)
		thread.create_and_start_with_poly_data(&channels[i], run)
	}

	put(&channels[0], n)

	for {
		time.sleep(time.Second)
	}
}
