// threadring (benchmarksgame) — Rust / std::sync::mpsc.
//
// 503 threads wired into a ring of channels. A single integer
// token hops node-to-node, decremented each hop; the node that
// receives 0 prints its 1-based label and exits the process.
// The entry sender is cloned so node 503 forwards back into
// node 1, closing the ring.

use std::env;
use std::process;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

const NTHREADS: usize = 503;

fn main() {
    let n: i32 = env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1000);

    let mut senders: Vec<Sender<i32>> = Vec::with_capacity(NTHREADS);
    let mut receivers: Vec<Option<Receiver<i32>>> = Vec::with_capacity(NTHREADS);

    for _ in 0..NTHREADS {
        let (tx, rx) = channel();
        senders.push(tx);
        receivers.push(Some(rx));
    }

    for i in 0..NTHREADS {
        let rx = receivers[i].take().unwrap();
        let next = senders[(i + 1) % NTHREADS].clone();
        let label = (i + 1) as i32;

        thread::spawn(move || loop {
            let token = rx.recv().unwrap();
            if token == 0 {
                println!("{}", label);
                process::exit(0);
            }
            next.send(token - 1).unwrap();
        });
    }

    senders[0].send(n).unwrap();

    loop {
        thread::park();
    }
}
