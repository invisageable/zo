use std::collections::HashMap;
use std::sync::{Arc, Mutex};
// use tokio::sync::mpsc::{self, Receiver, Sender};
use wasmtime::*;

use kanal::{AsyncSender, Receiver, Sender};

#[derive(Debug)]
pub enum Message {
  Text(String),
  Exit,
}

#[derive(Debug)]
pub struct Process {
  pub id: PID,
  pub sender: AsyncSender<Message>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PID(pub usize);

struct MyState {
  pub name: String,
  pub count: usize,
}

struct Runtime<'conf> {
  processes: Arc<Mutex<HashMap<PID, Process>>>,
  config: &'conf mut Config,
}

impl<'conf> Runtime<'conf> {
  pub fn new(config: &'conf mut Config) -> Self {
    Self {
      processes: Arc::new(Mutex::new(HashMap::new())),
      config,
    }
  }

  pub async fn new_async(config: &'conf mut Config) -> Self {
    Self {
      processes: Arc::new(Mutex::new(HashMap::new())),
      config,
    }
  }

  async fn spawn(&mut self, wat: &str) -> PID {
    let (tx, rx) = kanal::unbounded_async();
    let processes = Arc::clone(&self.processes);
    let engine = Engine::new(&self.config).unwrap();

    let pid = {
      let mut processes_guard = processes.lock().unwrap();
      let pid = PID(processes_guard.len() + 1);

      processes_guard.insert(
        pid,
        Process {
          id: pid,
          sender: tx.clone(),
        },
      );

      pid
    };

    let wat = wat.to_owned();

    tokio::spawn(async move {
      let mut store = Store::new(
        &engine,
        MyState {
          name: "hello, world!".to_string(),
          count: 0,
        },
      );

      let module = Module::new(&store.engine(), wat).unwrap();

      let hello = Func::wrap(&mut store, |mut caller: Caller<'_, MyState>| {
        // println!("Calling back...");
        // println!("> {}", caller.data().name);
        caller.data_mut().count += 1 * 200 + 400 / 2;
      });

      let instance = Instance::new_async(&mut store, &module, &[hello.into()])
        .await
        .unwrap();

      let run = instance
        .get_typed_func::<(), ()>(&mut store, "run")
        .unwrap();

      while let Ok(msg) = rx.recv().await {
        match msg {
          Message::Text(text) => {
            // println!("Text = {}", text);
            run.call_async(&mut store, ()).await.unwrap();
          }
          Message::Exit => {
            println!("Exit");
            break;
          }
        }
      }

      let mut processes_guard = processes.lock().unwrap();

      processes_guard.remove(&pid);
    });

    pid
  }

  async fn send(&self, pid: PID, msg: Message) {
    let processes = self.processes.lock().unwrap();

    if let Some(process) = processes.get(&pid) {
      process.sender.send(msg).await.unwrap();
    } else {
      eprintln!("PID(not-found) = {pid:?}");
    }
  }
}

#[tokio::main]
async fn main() {
  let mut config = Config::new();

  config.async_support(true);

  let mut runtime = Runtime::new_async(&mut config).await;

  let wat = r#"
(module
  (func $hello (import "" "hello"))
  (func (export "run") (call $hello))
)
  "#;

  let pid = runtime.spawn(wat).await;

  let start = std::time::Instant::now();

  for _ in 0..1_000_000 {
    runtime
      .send(pid, Message::Text(String::from("hello!")))
      .await;
  }

  runtime.send(pid, Message::Exit).await;

  let end = start.elapsed();

  println!("{}", end.as_secs());

  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}
