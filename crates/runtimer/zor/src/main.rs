use core::panic;

// use flume::{Receiver, Sender};
use hashbrown::{hash_map::Entry, HashMap};
use kanal::{AsyncReceiver, AsyncSender, Receiver, Sender};

#[derive(Clone, Debug)]
pub enum Message {
  Text(String),
  Fail,
  Exit,
}

#[derive(Debug)]
pub struct Process {
  pub id: PID,
  pub sender: Sender<Message>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PID(pub usize);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ProcessState {
  pub retry_count: usize,
  pub last_failure: Option<tokio::time::Instant>,
}

#[derive(Debug)]
pub struct Scheduler {
  pub processes: std::sync::Arc<tokio::sync::Mutex<HashMap<PID, ProcessState>>>,
}

impl Scheduler {
  fn new() -> Self {
    Self {
      processes: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    }
  }
}

impl Scheduler {
  const MAX_RETRIES: usize = 3;

  async fn run_process(&self, pid: PID, rx: AsyncReceiver<Message>) {
    let processes = self.processes.clone();

    tokio::spawn(async move {
      let mut interval =
        tokio::time::interval(tokio::time::Duration::from_secs(1));

      let mut rcount = 0i32;
      let mut start = tokio::time::Instant::now();

      loop {
        tokio::select! {
          Ok(msg) = rx.recv() => {
            rcount += 1i32;

            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
              match msg {
                Message::Text(_text) => {
                  // println!("Received message: {}", text);
                }
                Message::Fail => {
                  panic!("Fail = {rcount:?}");

                }
                Message::Exit => {
                  // println!("Received exit signal");
                  return Err(());
                }
              }

              Ok(())
            }));

            if let Err(_err) = res {
              // note(ivs) — at this stage, we need to save the state, relaunch
              // the server and so on.

              let mut processes = processes.lock().await;

              println!("PID = {pid:?}");

              // let state = processes.entry(pid).or_insert(ProcessState { retry_count: 0usize, last_failure: None });
              // let mut state = processes.get_mut(&pid).unwrap();
              if let Some(state) = processes.get_mut(&pid) {
                println!("State = {state:?}");

                if state.retry_count < Self::MAX_RETRIES {
                  state.retry_count += 1usize;
                  state.last_failure = Some(tokio::time::Instant::now());

                  tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                  continue;
                } else {
                  println!("Max retries reached for process {:?}", pid);
                  return;
                }
              }

              continue;
            }

            // Calculate time elapsed and print the rate
            if rcount % 1_000_000i32 == 0i32 {
              let elapsed = start.elapsed().as_secs_f64();

              println!("Processed 1.000.000 messages in {:.6} seconds", elapsed);

              start = tokio::time::Instant::now(); // reset the timer
            }
          }

          _ = interval.tick() => {
            // Do some periodic work
            // println!("Periodic task");
          }
        }
      }
    });
  }
}

#[tokio::main]
async fn main() {
  let (tx, rx) = kanal::unbounded_async();
  let scheduler = Scheduler::new();
  let pid = PID(1usize);

  scheduler.run_process(pid, rx).await;

  let msg = Message::Text("delkde".into());

  for _ in 0i32..10_000_000i32 {
    tx.send(msg.clone()).await.unwrap();
  }

  tx.send(Message::Fail).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  // tx.send(Message::Fail).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  tx.send(Message::Text("".into())).await.unwrap();
  // tx.send(Message::Fail).await.unwrap();

  // tx.send(Message::Exit).await.unwrap();
}
