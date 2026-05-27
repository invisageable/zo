# concurrency

The zo runtime ignores standard state-machine `async`/`await` transforms entirely, eliminating function coloring bugs across your system. Execution runs inside native runtime-managed green threads tracking execution scope blocks called nurseries. Blocking a task triggers immediate context frame swaps inside the scheduler.

## nursery

A `nursery` container sets strict lexical boundaries for concurrent task Lifecycles. The execution block cannot exit until every spawned green thread unwinds completely.

  ```zo
  fun worker(id: int, tx: Tx<int>) {
    showln("worker: {id}");
    tx.send(id * 10);
  }

  fun main() {
    imu (tx, rx) := channel();
  
    -- The nursery handles concurrent task tracking
    -- smechanics cleanly.
    nursery {
      spawn worker(1, tx);
      spawn worker(2, tx);
    } -- exical boundary block: execution holds here
    --  until both tasks complete.

    imu res1 := rx.recv();
    imu res2 := rx.recv();
    showln("collected results: {res1}, {res2}");
  }
  ```

- **True Stackful Green Threads**: Every concurrent execution task allocates a lightweight runtime execution stack. Deep nested calls compile natively without restructuring code into complex async state loops.
- **Unified Runtime Scheduler**: The internal task coordinator monitors state mutations directly. Invoking `rx.recv()` on an empty channel yields execution, swapping out active thread contexts immediately.
- **Structural Cancellation**: Nurseries form distinct isolation islands. Triggering task cancellations propagates downstream through children stacks because the underlying runtime owns the stack handles.

## supervise

  ```zo
  ```

## select

Coordinate communication states across multiple channel references using the non-blocking select block format:

  ```zo
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
