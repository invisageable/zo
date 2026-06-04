// threadring (benchmarksgame) — Gleam / BEAM, the language's home
// turf: 503 lightweight processes wired into a ring, one integer
// token hopping node-to-node and decremented each hop. The node
// that receives 0 prints its 1-based label (498 for N=1000).
//
// BEAM subjects are owner-bound — a process can only receive on a
// subject it created — so the ring is built in two phases: every
// node registers its own subject with main, then main wires each
// node to its successor before injecting the token.

import gleam/dict.{type Dict}
import gleam/erlang/process.{type Subject}
import gleam/int
import gleam/io

const nodes = 503

const passes = 1000

type Msg {
  Wire(Subject(Msg))
  Token(Int)
}

fn forward(
  label: Int,
  me: Subject(Msg),
  next: Subject(Msg),
  done: Subject(Int),
) -> Nil {
  case process.receive_forever(me) {
    Token(0) -> process.send(done, label)
    Token(t) -> {
      process.send(next, Token(t - 1))
      forward(label, me, next, done)
    }
    Wire(_) -> forward(label, me, next, done)
  }
}

// Phase 1: spawn nodes 1..nodes; each owns and registers its
// subject so main can wire predecessors to it afterwards.
fn spawn_ring(
  label: Int,
  reg: Subject(#(Int, Subject(Msg))),
  done: Subject(Int),
) -> Nil {
  case label > nodes {
    True -> Nil
    False -> {
      process.spawn(fn() {
        let me = process.new_subject()

        process.send(reg, #(label, me))

        let next = case process.receive_forever(me) {
          Wire(n) -> n
          Token(_) -> me
        }

        forward(label, me, next, done)
      })

      spawn_ring(label + 1, reg, done)
    }
  }
}

fn collect(
  reg: Subject(#(Int, Subject(Msg))),
  remaining: Int,
  acc: Dict(Int, Subject(Msg)),
) -> Dict(Int, Subject(Msg)) {
  case remaining {
    0 -> acc
    _ -> {
      let #(label, subject) = process.receive_forever(reg)

      collect(reg, remaining - 1, dict.insert(acc, label, subject))
    }
  }
}

// Phase 2: hand each node its successor's subject (503 closes
// back to 1), turning the chain into a real cycle.
fn wire_ring(label: Int, subjects: Dict(Int, Subject(Msg))) -> Nil {
  case label > nodes {
    True -> Nil
    False -> {
      let assert Ok(me) = dict.get(subjects, label)
      let successor = case label == nodes {
        True -> 1
        False -> label + 1
      }
      let assert Ok(next) = dict.get(subjects, successor)

      process.send(me, Wire(next))

      wire_ring(label + 1, subjects)
    }
  }
}

pub fn main() {
  let reg = process.new_subject()
  let done = process.new_subject()

  spawn_ring(1, reg, done)

  let subjects = collect(reg, nodes, dict.new())

  wire_ring(1, subjects)

  let assert Ok(first) = dict.get(subjects, 1)

  process.send(first, Token(passes))

  io.println(int.to_string(process.receive_forever(done)))
}
