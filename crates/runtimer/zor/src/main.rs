use std::sync::Arc;
use std::sync::Mutex;

pub enum Message {
  Ping(String),
  CreateActor(String),
  Stop,
}

pub struct Actor {
  pub name: String,
  pub sender: kanal::AsyncSender<Message>,
}

impl Actor {
  fn new(name: String) -> Arc<Mutex<Self>> {
    let (tx, rx) = kanal::bounded_async::<Message>(32);
    let actor = Arc::new(Mutex::new(Actor { name, sender: tx }));
    let actor_clone = Arc::clone(&actor);

    // spawn a new asynchronous task for the actor.
    tokio::spawn(async move {
      let behavior = Actor::default_behavior();
      while let Ok(msg) = rx.recv().await {
        let actor_guard = actor_clone.lock().unwrap();
        match msg {
          Message::Ping(data) => {
            println!("{} received: {}", actor_guard.name, data);
            behavior(&data);
          }
          Message::CreateActor(name) => {
            println!("{} creating a new actor: {}", actor_guard.name, name);
            Actor::new(name);
          }
          Message::Stop => {
            println!("{} is stopping.", actor_guard.name);
            break;
          }
        }
      }
    });

    actor
  }

  fn send(&self, msg: Message) {
    let sender = self.sender.clone();
    tokio::spawn(async move {
      sender.send(msg).await.unwrap();
    });
  }

  fn default_behavior() -> Box<dyn Fn(&str) + Send> {
    Box::new(|msg| {
      println!("default behavior: {}", msg);
    })
  }
}

#[tokio::main]
async fn main() {
  let start = std::time::Instant::now();

  // create the first actor.
  let actor1 = Actor::new("Actor1".into());

  // send messages to the actor.
  actor1
    .lock()
    .unwrap()
    .send(Message::Ping("hello from main".into()));

  // Spawn 1,000,000 actors to benchmark performance.
  for i in 0i32..1_000_000_i32 {
    let name = format!("actor{}", i);
    let actor = Actor::new(name.clone());

    actor
      .lock()
      .unwrap()
      .send(Message::Ping(format!("hello from {}", name)));
  }

  // send a stop message to the original actor after all others are spawned.
  actor1.lock().unwrap().send(Message::Stop);

  // allow some time for all actors to process the messages.
  tokio::time::sleep(std::time::Duration::from_secs(1u64)).await;

  let duration = start.elapsed();
  println!("completed in {:?}", duration);
  let actors_per_second = 1_000_000_f64 / duration.as_secs_f64();
  println!("actors per second: {}", actors_per_second);

  if actors_per_second >= 1_000_000.0_f64 {
    println!("GOAT Status Achieved 🐐");
  } else {
    println!("keep optimizing!");
  }
}
