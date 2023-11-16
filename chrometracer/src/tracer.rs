use crossbeam_channel::Sender;
use crossbeam_queue::ArrayQueue;
use derive_builder::Builder;
use std::{
    cell::RefCell,
    fs::File,
    io::{self, BufWriter, Write},
    thread::{self, JoinHandle, ThreadId},
    time::Instant, os::unix::prelude::OsStrExt,
};
use tracing_chrometrace::{ChromeEvent, ChromeEventBuilder, EventType};



#[derive(Debug)]
pub struct SimpleEvent {
    pub name: &'static str,
    pub from: std::time::Duration,
    pub to: std::time::Duration,
    pub is_async: bool,
    pub tid: u64,
}

impl SimpleEvent {
    fn write_json<W>(self, writer: &mut W)
    where
        W: std::io::Write
    {
        let pid = std::process::id();
        let json = if self.is_async {
            let begin = self.from.as_nanos() as f64 / 1000.0;
            let end = self.to.as_nanos() as f64 / 1000.0;
            format!("{{\"name\":\"{}\",\"ts\":{},\"pid\":{},\"tid\":{},\"id\":{},\"ph\":\"b\",\"cat\":\"async\"}},\n{{\"name\":\"{}\",\"ts\":{},\"pid\":{},\"tid\":{},\"id\":{},\"ph\":\"e\",\"cat\":\"async\"}}", self.name, begin, pid, self.tid, self.from.as_nanos(), self.name, end, pid, self.tid, self.from.as_nanos())
            
        } else {
            let ts = self.from.as_nanos() as f64 / 1000.0;
            let dur = (self.to.as_nanos() - self.from.as_nanos()) as f64 / 1000.0;
            format!("{{\"name\":\"{}\",\"ts\":{},\"dur\":{},\"pid\":{},\"tid\":{},\"ph\":\"X\"}}", self.name, ts, dur, std::process::id(), self.tid)
        };
        writer.write_all(json.as_bytes()).unwrap();
    } 
}


thread_local! {
    static CURRENT: RefCell<Option<ChromeTracer>> = RefCell::new(None);
}

static mut GLOBAL: Option<ChromeTracer> = None;

#[derive(Builder, Clone)]
#[builder(custom_constructor, build_fn(private, name = "_build"))]
pub struct ChromeTracer {
    #[builder(default = "Instant::now()")]
    pub start: Instant,

    #[builder(setter(skip))]
    sender: Option<Sender<ChromeTracerMessage>>,

    #[builder(default = "std::thread::current().id().as_u64().into()")]
    pub tid: u64,
}

#[allow(clippy::large_enum_variant)]
enum ChromeTracerMessage {
    ChromeEvent(SimpleEvent/* , ThreadId*/),
    Terminate,
}

pub struct ChromeTracerGuard {
    sender: Sender<ChromeTracerMessage>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for ChromeTracerGuard {
    fn drop(&mut self) {
        self.sender.send(ChromeTracerMessage::Terminate).unwrap();
        self.handle.take().map(JoinHandle::join).unwrap().unwrap();
    }
}

impl ChromeTracerBuilder {
    pub fn init(&self) -> ChromeTracerGuard {
        CURRENT.with(|c| {
            if unsafe { GLOBAL.is_some() } {
                panic!("Unable to intialize ChromeTracer. A chrometracer already been set");
            } else {
                let mut tracer = self._build().expect("All required fields were initialized");
                let guard = tracer.init();

                unsafe { GLOBAL = Some(tracer.clone()) };
                *c.borrow_mut() = Some(tracer);

                guard
            }
        })
    }
}

pub fn builder() -> ChromeTracerBuilder {
    ChromeTracerBuilder::create_empty()
}

impl ChromeTracer {
    fn init(&mut self) -> ChromeTracerGuard {
        let (sender, receiver) = crossbeam_channel::unbounded();
        self.sender = Some(sender.clone());

        let handle = Some(thread::spawn(move || {
            let mut writer = BufWriter::new(File::create("trace.json").unwrap());

            let queue = ArrayQueue::new(1);

            writer.write_all(b"[\n").unwrap();

            while let Ok(ChromeTracerMessage::ChromeEvent(event)) = receiver.recv() {
                if let Some(e) = queue.force_push(event) {
                    e.write_json(&mut writer);
                    writer.write_all(b",\n").unwrap();
                };
            }

            if let Some(e) = queue.pop() {
                e.write_json(&mut writer);
                writer.write_all(b"\n").unwrap();
            }

            writer.write_all(b"]").unwrap();
        }));

        ChromeTracerGuard { sender, handle }
    }

    #[inline]
    pub fn trace(&self, event: SimpleEvent) {
        let _ = self
            .sender
            .as_ref()
            .map(|sender| sender.send(ChromeTracerMessage::ChromeEvent(event)));
    }
}

#[inline]
pub fn current<T, F>(mut f: F) -> T
where
    F: FnMut(Option<&ChromeTracer>) -> T,
{
    CURRENT.with(|c| {
        let mut tracer = c.borrow_mut();
        if tracer.is_none() {
            *tracer = unsafe { GLOBAL.clone() };
            tracer.as_mut().map(|t| t.tid = std::thread::current().id().as_u64().into());
        }

        f(tracer.as_ref())
    })
}

#[macro_export]
macro_rules! event {
    (name: $name:expr, from: $from:expr, to: $to:expr, is_async: $is_async:expr) => {

        $crate::current(|tracer| {
            if let Some(tracer) = tracer {
                // use $crate::Recordable as _;

                let event = $crate::SimpleEvent {
                    name: $name,
                    from: $from,
                    to: $to,
                    is_async: $is_async,
                    tid: tracer.tid,
                    //tid: std::thread::current().id(),
                };

                // let mut builder = $crate::ChromeEvent::builder(tracer.start);
                // $name.record(&mut builder, "name");
                // $(
                //     $value.record(&mut builder, stringify!($key));
                // )*

                // let event = builder.build().unwrap();
                tracer.trace(event);
            }
        })
    };
}

pub trait Recordable {
    type Item;

    fn record(self, builder: &mut ChromeEventBuilder, name: &'static str);
}

impl Recordable for u64 {
    type Item = u64;

    fn record(self, builder: &mut ChromeEventBuilder, name: &'static str) {
        match name {
            "tid" => builder.tid(self),
            "pid" => builder.pid(self),
            _ => builder.arg((name.to_string(), self.to_string())),
        };
    }
}

impl Recordable for &'static str {
    type Item = &'static str;

    fn record(self, builder: &mut ChromeEventBuilder, name: &'static str) {
        match name {
            "name" => builder.name(self),
            "cat" => builder.cat(self),
            "id" => builder.id(self),
            _ => builder.arg((name.to_string(), self.to_string())),
        };
    }
}

impl Recordable for String {
    type Item = String;

    fn record(self, builder: &mut ChromeEventBuilder, name: &'static str) {
        match name {
            "name" => builder.name(self),
            "cat" => builder.cat(self),
            "id" => builder.id(self),
            _ => builder.arg((name.to_string(), self)),
        };
    }
}

impl Recordable for f64 {
    type Item = f64;

    fn record(self, builder: &mut ChromeEventBuilder, name: &'static str) {
        match name {
            "ts" => builder.ts(self),
            "dur" => builder.dur(Some(self)),
            "tts" => builder.tts(Some(self)),
            _ => builder.arg((name.to_string(), self.to_string())),
        };
    }
}

impl Recordable for EventType {
    type Item = EventType;

    fn record(self, builder: &mut ChromeEventBuilder, name: &'static str) {
        match name {
            "ph" => builder.ph(self),
            _ => builder.arg((name.to_string(), self.as_ref().to_string())),
        };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn event() {
        let _guard = crate::builder().init();

        event!(name: "hello", from: std::time::Duration::from_secs(1), to: std::time::Duration::from_secs(2), is_async: true);
    }

    #[test]
    fn without_init() {
        event!(name: "hello", from: std::time::Duration::from_secs(1), to: std::time::Duration::from_secs(2), is_async: false);
    }
}