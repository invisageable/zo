// use std::sync::Arc;
// use std::sync::Mutex;
// use tokio::sync::mpsc;
// use tokio::task;

// enum Message {
//   Ping(String),
//   CreateActor(String),
//   Stop,
// }

// struct Actor {
//   name: String,
//   sender: kanal::AsyncSender<Message>,
// }

// impl Actor {
//   fn new(name: String) -> Arc<Mutex<Self>> {
//     let (tx, mut rx) = kanal::bounded_async::<Message>(32);
//     let actor = Arc::new(Mutex::new(Actor {
//       name: name.clone(),
//       sender: tx,
//     }));
//     let actor_clone = Arc::clone(&actor);
//     // Spawn a new asynchronous task for the actor
//     tokio::spawn(async move {
//       let mut behavior = Actor::default_behavior();
//       while let Ok(msg) = rx.recv().await {
//         let actor_guard = actor_clone.lock().unwrap();
//         match msg {
//           Message::Ping(data) => {
//             // println!("{} received: {}", actor_guard.name, data);
//             behavior(&data);
//           }
//           Message::CreateActor(name) => {
//             // println!("{} creating a new actor: {}", actor_guard.name,
// name);             Actor::new(name);
//           }
//           Message::Stop => {
//             // println!("{} is stopping.", actor_guard.name);
//             break;
//           }
//         }
//       }
//     });
//     actor
//   }

//   fn send(&self, msg: Message) {
//     let sender = self.sender.clone();
//     tokio::spawn(async move {
//       sender.send(msg).await.unwrap();
//     });
//   }

//   fn default_behavior() -> Box<dyn Fn(&str) + Send> {
//     Box::new(|msg| {
//       // println!("Default behavior: {}", msg);
//     })
//   }
// }

// #[tokio::main]
// async fn main() {
//   let start = std::time::Instant::now();

//   // Create the first actor
//   let actor1 = Actor::new("Actor1".to_string());

//   // Send messages to the actor
//   actor1
//     .lock()
//     .unwrap()
//     .send(Message::Ping("Hello from main".to_string()));

//   // Spawn 1,000,000 actors to benchmark performance
//   for i in 0..1_000_000 {
//     let name = format!("Actor{}", i);
//     let actor = Actor::new(name.clone());

//     actor
//       .lock()
//       .unwrap()
//       .send(Message::Ping(format!("Hello from {}", name)));
//   }

//   // Send a stop message to the original actor after all others are spawned
//   actor1.lock().unwrap().send(Message::Stop);

//   // Allow some time for all actors to process the messages
//   tokio::time::sleep(std::time::Duration::from_secs(1)).await;

//   let duration = start.elapsed();
//   println!("Completed in {:?}", duration);
//   let actors_per_second = 1_000_000 as f64 / duration.as_secs_f64();
//   println!("Actors per second: {}", actors_per_second);

//   if actors_per_second >= 1_000_000.0 {
//     println!("GOAT Status Achieved 🐐");
//   } else {
//     println!("Keep optimizing!");
//   }
// }

/// Encodes a number into a byte-oriented universal code.
fn encode_number(mut n: u64) -> Vec<u8> {
  let mut result = Vec::new();

  loop {
    // Encode the number in binary form, fitting into 7 bits
    let byte = (n & 0b01111111) as u8;
    result.push(byte);

    // Shift right by 7 bits to process the next part of the number
    n >>= 7;

    // If the remaining number is zero, break out of the loop
    if n == 0 {
      break;
    }

    // Set MSB of the current byte to 1 for prefix groups
    *result.last_mut().unwrap() |= 0b10000000;

    // Decrement n before the next iteration, as per the encoding scheme
    n -= 1;
  }

  // Reverse the result since we encode the number from LSB to MSB
  result.reverse();
  result
}

/// Decodes a byte-oriented universal code back into a number.
fn decode_number(mut bytes: &[u8]) -> u64 {
  let mut n = 0u64;

  for &byte in bytes.iter() {
    // Shift the current number 7 bits to the left to make room for new bits
    n = (n << 7) | (byte & 0b01111111) as u64;

    // If the MSB is 0, it's the final group, so we're done
    if byte & 0b10000000 == 0 {
      break;
    }

    // If it was a prefix group, increment n
    n += 1;
  }

  n
}

fn main() {
  let numbers = vec![0, 1, 127, 128, 129, 32767, 32768, 8388607, 8388608];

  for &number in &numbers {
    let encoded = encode_number(number);
    let decoded = decode_number(&encoded);
    println!(
      "Original: {:>10} | Encoded: {:?} | Decoded: {:>10}",
      number, encoded, decoded
    );
    assert_eq!(number, decoded);
  }
}
