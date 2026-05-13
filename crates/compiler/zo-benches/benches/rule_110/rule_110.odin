// rule 110 — Wolfram's Turing-complete 1D cellular automaton.
// each cell's next state = rule_table[left*4 + center*2 + right]
// rule table 01101110 (= 110) indexed 0..7.

package main

import "core:fmt"

main :: proc() {
  width :: 31
  gens :: 15
  rule := [?]int{0, 1, 1, 1, 0, 1, 1, 0}

  row: [width]int
  for i := 0; i < width; i += 1 {
    row[i] = 0
  }
  row[width - 1] = 1

  next: [width]int

  for g := 0; g < gens; g += 1 {
    for p := 0; p < width; p += 1 {
      if row[p] == 1 {
        fmt.print("#")
      } else {
        fmt.print(".")
      }
    }
    fmt.println()

    for k := 0; k < width; k += 1 {
      left := 0
      if k > 0 {
        left = row[k - 1]
      }
      center := row[k]
      right := 0
      if k < width - 1 {
        right = row[k + 1]
      }
      pattern := left * 4 + center * 2 + right
      next[k] = rule[pattern]
    }

    row = next
  }
}
