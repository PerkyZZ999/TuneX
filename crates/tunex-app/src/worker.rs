//! One background worker thread behind a request/answer channel pair (C12).
//!
//! Art covers and lyrics share this shape; debounced search keeps its own
//! stateful worker instead: its debounce loop is a stream processor (quiet
//! windows, superseded generations), not a pure function of one request,
//! and forcing it into this mold would move policy without removing any
//! machinery.

use std::sync::mpsc as std_mpsc;

/// Background worker: requests in, answers out, off the Qt thread.
///
/// Dropping releases the request channel and joins the thread, so no worker
/// outlives its handle. Owners hold this in an `Option` and respawn on
/// send failure (a dead worker never fails forever).
#[derive(Debug)]
pub struct Worker<Request, Answer> {
    requests: Option<std_mpsc::Sender<Request>>,
    answers: std_mpsc::Receiver<Answer>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl<Request, Answer> Drop for Worker<Request, Answer> {
    fn drop(&mut self) {
        // Release the request sender first so a blocked `recv` wakes up.
        drop(self.requests.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl<Request, Answer> Worker<Request, Answer>
where
    Request: Send + 'static,
    Answer: Send + 'static,
{
    /// Start the thread running `work` per request until the request sender
    /// is released. Returns `None` when the OS refuses the thread; owners
    /// degrade (settled-empty, surfaced error) instead of aborting.
    /// `work` must be a pure function of one request: owners keep caches
    /// and generations on the Qt side.
    pub fn spawn(name: &str, work: impl Fn(Request) -> Answer + Send + 'static) -> Option<Self> {
        let (request_tx, request_rx) = std_mpsc::channel::<Request>();
        let (answer_tx, answer_rx) = std_mpsc::channel::<Answer>();
        let thread = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    if answer_tx.send(work(request)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self {
            requests: Some(request_tx),
            answers: answer_rx,
            thread: Some(thread),
        })
    }

    /// Send one request without blocking. Returns false when the worker is
    /// dead; owners respawn through [`Worker::spawn`] and retry or degrade.
    pub fn request(&self, request: Request) -> bool {
        self.requests
            .as_ref()
            .is_some_and(|requests| requests.send(request).is_ok())
    }

    /// Drain settled answers without blocking.
    #[must_use]
    pub fn drain(&self) -> Vec<Answer> {
        let mut settled = Vec::new();
        while let Ok(answer) = self.answers.try_recv() {
            settled.push(answer);
        }
        settled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_drain_round_trips_in_order() {
        let worker = Worker::spawn("tunex-test", |value: i32| value * 2).expect("thread spawns");
        assert!(worker.request(21));
        assert!(worker.request(22));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let settled = loop {
            let settled = worker.drain();
            if settled.len() == 2 || std::time::Instant::now() >= deadline {
                break settled;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(settled, vec![42, 44]);
    }

    #[test]
    fn drop_joins_without_hanging_on_idle_worker() {
        let worker = Worker::spawn("tunex-test-idle", |value: i32| value).expect("thread spawns");
        drop(worker);
    }
}
