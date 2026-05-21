# concurrency

zo avoids standard async/await architectures entirely, preventing language function coloring. It operates native
runtime-managed green threads executing safely within structural lexical limits called a nursery . When tasks
block on channels, the scheduler automatically shifts context execution frames.

## nursery

  ```zo
  -! Continuous green-thread orchestration pipelines.
  -! The nursery container guarantees structured tracking lifecycle limits.

  fun worker(id: int, ch: Tx<int>) {
    showln("worker {id} spinning up");
    tx.send(id * 10);
  }

  fun main() {
    imu (tx, rx) := channel();
  
    -- Nursery block isolates processing scopes structurally.
    nursery {
      spawn worker(1, tx);
      spawn worker(2, tx);
    } -- Lexical exit point guarantees both operations have fully wound down.

    imu res1 := rx.recv();
    imu res2 := rx.recv();
    showln("collected results: {res1}, {res2}");
  }
  ```

- structured concurrency. `nursery { spawn a(); spawn b(); }` means both children must finish before the block exits.
- `Channels` + `select` give you composition. The scheduler picks another task whenever one blocks on `chan.recv()`
- There's no callback hell to escape from in the first place.

description:
  - green and os threads
    - Real green threads. Each task has a stack; you can call deeply-nested code without a state-machine transform.
    - No function coloring. Every function can spawn. No async fn / fn divide.
    - Cancellation propagates structurally. The nursery is the cancellation scope; task.cancel() works because the runtime owns the green-thread stack.
  - scheduler
    - Single-runtime. zo has one scheduler
    - `chan.recv()` blocks the task, the scheduler swaps.

## supervise

  ```zo
  ```

## select

  ```zo
  -- Coordinate channel state changes reactively.
  select {
    rx1 => fn(value: int) => showln("chan1: {value}"),
    rx2 => fn(value: int) => showln("chan2: {value}"),
  }
  ```

## thread

  ```zo
  ```

## await

  ```zo
  ```
