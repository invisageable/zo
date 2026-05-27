# telemetry-visualizer

Real-time compilation telemetry dashboard written in zo. Two programs communicate over TCP:

- **visualizer** (`src/main.zo`) — binds TCP 18472, receives NDJSON events, renders an animated raylib dashboard at 60 FPS with lane bars, phase colors, legend, and metrics panel.
- **driver** (`src/driver/main.zo`) — simulates 12 files through 5 timed compiler phases, streams 121 events over TCP via nursery + channel + green tasks.

## run

Terminal 1 — start the visualizer (opens a raylib window and waits for events):

```sh
cd crates/compiler-self/telemetry-visualizer/src
zo run main.zo
```

Terminal 2 — start the driver (connects to the visualizer and sends events):

```sh
zo run crates/compiler-self/telemetry-visualizer/src/driver/main.zo
```

The driver connects, sends all events, and exits. The visualizer animates the dashboard as events arrive.
