# concurrency

  ```zo
  -! zo uses real green threads managed by a single-runtime scheduler.
  -! Forget async/await function coloring — any function can spawn.

  fun fetch_data(id: int) -> str {
    -- Simulating a blocking call. The scheduler automatically
    -- swaps to another task while this waits.
    "data"
  }

  fun main() {
    -- The nursery enforces structural scoping. Both children
    -- must complete before execution moves past the block.
    nursery {
      spawn fetch_data(1);
      spawn fetch_data(2);
    }
    
    showln("Both tasks completed successfully.");
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

## nursery

  ```zo
  ```

## select

  ```zo
  ```

## thread

  ```zo
  ```

## spawn

  ```zo
  ```

## await

  ```zo
  ```

For everything servers do (web handlers, pipelines, RPC, fan-out fan-in, pubsub, stream processing) zo's model is strictly cleaner than async/await.
