use std::time::Instant;
use std::thread::{self, JoinHandle};

use crossbeam_channel::Sender;
use crossbeam_queue::ArrayQueue;

static mut GLOBAL: Option<Tracer> = None;

pub struct Tracer {
    pub start: Instant,
    sender: Option<Sender<Message>>,
}

enum Message {
    Span(Event),
    Terminate,
}

pub struct TracerGuard {
    sender: Sender<Message>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for TracerGuard {
    fn drop(&mut self) {
        self.sender.send(Message::Terminate).unwrap();
        self.handle.take().map(JoinHandle::join).unwrap().unwrap();
    }
}

impl Tracer {

    pub fn init() -> TracerGuard {
        let mut tracer = Tracer {
            start: Instant::now(),
            sender: None,
        };

        let (tx, rx) = crossbeam_channel::unbounded();
        tracer.sender = Some(tx.clone());

        let handle = thread::spawn(move || {
            let queue = ArrayQueue::new(1);

            while let Ok(Message::Span(value)) = rx.recv() {
                if let Some(_) = queue.force_push(value) {
                    //println!("{}", v);
                }
            }

            if let Some(v) = queue.pop() {
                println!("{:?}", v);
            }
        });

        unsafe { GLOBAL = Some(tracer) };

        TracerGuard {
            sender: tx,
            handle: Some(handle),
        }
    }

    pub fn trace(&self, v: Event) {
        let _ = self.sender
            .as_ref()
            .map(|tx| tx.send(Message::Span(v)));
    }
}

#[inline]
pub fn get_global() -> &'static Option<Tracer> {
    unsafe { &crate::tracer::GLOBAL }
}

#[derive(Debug)]
pub struct Event {
    pub ts: std::time::Duration,
    pub name: &'static str,
    pub i: &'static str,
    pub f: &'static str,
}

#[macro_export]
macro_rules! event {
    ($($key:ident = $value:expr),*) => {
        let tracer = $crate::get_global();
        if let Some(tracer) = tracer {
            
            let mut event = $crate::Event {
                ts: tracer.start.elapsed(),
                name: "",
                i: "",
                f: "",
            };

            // $(
            //     match stringify!($key) {
            //         "name" => event.name = Some($value.to_string()),
            //         _ => (),
            //     }
            // )*
            $(
                event.$key = stringify!($value);
            )*
            tracer.trace(event);
        }
    };
}

#[macro_export]
macro_rules! tostring {
    ($($key:ident = $value:expr),*) => {
        let tracer = $crate::get_global();
        if let Some(tracer) = tracer {
            let a = "a".to_string();
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn span() {
        let _guard = crate::tracer::Tracer::init();
        let a = 5;
        event!(name = hello, i=1, f=3.4);
        event!(name = hello, i=2, f=4.1);
    }

    #[test]
    fn ignore_span() {
        event!(name = "hello");
    }

}