use std::borrow::Cow;
use std::cell::RefCell;
use std::pin::Pin;
use std::sync::{Arc, atomic::{AtomicU8, Ordering}, OnceLock, Weak};
use std::thread::Thread;

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::{self, introspect::{Introspector, SinkInfo, SinkInputInfo}, subscribe::{Facility, InterestMaskSet, Operation}, Context, State};
use libpulse_binding::def::{BufferAttr, PortAvailable, Retval};
use libpulse_binding::mainloop::standard::Mainloop;
use libpulse_binding::proplist::{properties::APPLICATION_NAME, Proplist};
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::{Stream, self, PeekResult};
use libpulse_binding::volume::ChannelVolumes;

use derive_more::derive::{Deref, From};
use parking_lot::{Mutex, MutexGuard};
use smallvec::SmallVec;

use tokio::sync::watch;

use super::error::{Error, PulseError};
use super::{AudioServer, Kind, Message, MessageClient, MessageOutput, Output, OutputClient, Sender, Volume, VolumeLevels};

const DEFAULT_PEAK_RATE: u32 = 30;

type Pb<T> = Pin<Box<T>>;
type Peakers = Vec<Pb<Stream>>;

type WeakContext = Weak<Mutex<RefCell<Context>>>;
type WeakPeakers = Weak<Mutex<RefCell<Peakers>>>;

pub struct Pulse {
    context: Arc<Mutex<RefCell<Context>>>,
    peakers: Arc<Mutex<RefCell<Peakers>>>,
    state:   Arc<AtomicU8>,
    lock:    watch::Sender<Lock>,
    thread:  Mutex<Thread>,
    running: Mutex<()>,
}

#[repr(u8)]
#[derive(PartialEq)]
enum Lock {
    Unlocked = 0,
    Locked   = 1,
    Aquire   = 2,
}

impl Pulse {
    thread_local! {
        static MAINLOOP: RefCell<Mainloop> = RefCell::new(Mainloop::new().unwrap());
    }

    pub fn new() -> Self {
        let context = Pulse::MAINLOOP.with_borrow(|mainloop| {
            Context::new(mainloop, "Mixxc Context").unwrap()
        });

        Self {
            context: Arc::new(Mutex::new(RefCell::new(context))),
            peakers: Arc::new(Mutex::new(RefCell::new(Vec::with_capacity(8)))),
            state:   Arc::new(AtomicU8::new(0)),
            lock:    watch::channel(Lock::Unlocked).0,
            thread:  Mutex::new(std::thread::current()),
            running:      Mutex::new(()),
        }
    }

    #[inline]
    fn set_state(&self, state: context::State) {
        self.state.store(state as u8, Ordering::Release);
    }

    #[inline]
    fn is_connected(&self) -> bool {
        self.state.load(Ordering::Acquire) == State::Ready as u8
    }

    #[inline]
    fn is_terminated(&self) -> bool {
        self.state.load(Ordering::Acquire) == State::Terminated as u8
    }

    async fn lock(&self) -> ContextRef<'_> {
        self.lock.send_replace(Lock::Aquire);
        self.lock.subscribe().wait_for(|lock| *lock == Lock::Locked).await.unwrap();

        let context = self.context.try_lock().unwrap();
        let thread = self.thread.try_lock().unwrap();

        ContextRef {
            context,
            lock: &self.lock,
            thread,
        }
    }

    fn lock_blocking(&self) -> ContextRef<'_> {
        self.lock.send_replace(Lock::Aquire);
        while *self.lock.borrow() != Lock::Locked { std::hint::spin_loop(); }

        let context = self.context.try_lock().unwrap();
        let thread = self.thread.try_lock().unwrap();

        ContextRef {
            context,
            lock: &self.lock,
            thread,
        }
    }

    fn iterate(timeout: &Duration) -> Result<u32, PulseError> {
        Self::MAINLOOP.with_borrow_mut(|mainloop| {
            mainloop.prepare(timeout.into()).map_err(PulseError::from)?;
            mainloop.poll().map_err(PulseError::from)?;
            mainloop.dispatch().map_err(PulseError::from)
        })
    }

    fn is_locked(&self) -> bool {
        self.lock.send_if_modified(|lock| {
            match *lock == Lock::Aquire {
                true => {
                    *lock = Lock::Locked;
                    true
                }
                false => false,
            }
        })
    }

    fn quit() {
        Self::MAINLOOP.with_borrow_mut(|mainloop| mainloop.quit(Retval(0)));
    }
}

impl AudioServer for Pulse {
    fn connect(&self, sender: impl Into<Sender<Message>>) -> Result<(), Error> {
        if self.is_connected() {
            return Err(Error::AlreadyConnected)
        }

        let mut proplist = Proplist::new().unwrap();
        proplist.set_str(APPLICATION_NAME, crate::APP_NAME).unwrap();

        self.context.lock().replace(Pulse::MAINLOOP.with_borrow(|mainloop| {
            Context::new_with_proplist(mainloop, "Mixxc Context", &proplist).unwrap()
        }));

        let sender: Sender<Message> = sender.into();

        let state_callback = Box::new({
            let context = Arc::downgrade(&self.context);
            let state = Arc::downgrade(&self.state);
            let sender = sender.clone();

            move || state_callback(&context, &state, &sender)
        });

        {
            let guard = self.context.lock();
            let mut context = guard.borrow_mut();

            // Manually calls state_callback and sets state to Connecting on success
            context.connect(None, context::FlagSet::NOAUTOSPAWN, None)
                .map_err(PulseError::from)?;

            self.set_state(State::Connecting);

            context.set_state_callback(Some(state_callback));
        }

        *self.thread.lock() = std::thread::current();

        let timeout = std::time::Duration::from_millis(1).into();
        let _running = self.running.lock();

        loop {
            match Pulse::iterate(&timeout) {
                Ok(_) => {},
                Err(PulseError::MainloopQuit) => break,
                Err(e) => sender.emit(Message::Error(e.into())),
            };

            if self.is_locked() {
                std::thread::park();

                if self.is_terminated() {
                    self.peakers.lock().borrow_mut().clear();
                    Pulse::quit()
                }
            }
        }

        Ok(())
    }

    fn disconnect(&self) {
        {
            let guard = self.lock_blocking();
            let mut context = guard.borrow_mut();

            context.set_state_callback(None);
            context.disconnect();

            // Context::disconnect manually calls state_callback and sets state to Terminated
            self.set_state(State::Terminated);
        }

        let _running = self.running.lock();
    }

    async fn request_software(&self, sender: impl Into<Sender<Message>>) -> Result<(), Error> {
        if !self.is_connected() {
            return Err(PulseError::NotConnected.into())
        }

        let sender = sender.into();

        let input_callback = {
            let context = Arc::downgrade(&self.context);
            let peakers = Arc::downgrade(&self.peakers);

            move |info: ListResult<&SinkInputInfo>| {
                add_sink_input(info, &context, &sender, &peakers);
            }
        };

        let context = self.lock().await;
        context.introspect().get_sink_input_info_list(input_callback);

        Ok(())
    }

    async fn request_outputs(&self, sender: impl Into<Sender<Message>>) -> Result<(), Error> {
        if !self.is_connected() {
            return Err(PulseError::NotConnected.into())
        }

        let sender = sender.into();

        let sink_info_callback = move |info: ListResult<&SinkInfo>| {
            let ListResult::Item(info) = info else {
                return
            };

            let Some(output_name) = &info.name else {
                let e = PulseError::NamelessSink(info.index).into();
                sender.emit(Message::Error(e));

                return;
            };

            let ports = info.ports.iter()
                .filter(|p| p.available != PortAvailable::No);

            for port in ports {
                let Some(port_name) = &port.name else {
                    let e = PulseError::NamelessPort(info.index).into();
                    sender.emit(Message::Error(e));

                    continue;
                };

                let output = Output {
                    name: output_name.to_string(),
                    port: port_name.to_string(),
                    master: false,
                };

                let msg: Message = MessageOutput::New(output).into();
                sender.emit(msg);
            }
        };

        let guard = self.lock().await;
        let introspect = guard.introspect();

        introspect.get_sink_info_list(sink_info_callback);

        Ok(())
    }

    async fn request_master(&self, sender: impl Into<Sender<Message>>) -> Result<(), Error> {
        if !self.is_connected() {
            return Err(PulseError::NotConnected.into())
        }

        let sender = sender.into();

        let sink_callback = move |info: ListResult<&SinkInfo>| {
            if let ListResult::Item(info) = info {
                let client: Box<OutputClient> = Box::new(info.into());
                let msg: Message = MessageClient::New(client).into();

                sender.emit(msg);

{