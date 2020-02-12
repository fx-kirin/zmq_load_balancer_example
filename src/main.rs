use crossbeam::channel::unbounded;
use futures::compat::{Future01CompatExt, Sink01CompatExt};
use futures::prelude::*;
use futures::try_join;
use log::{debug, error, info, trace, warn};
use std::env;
use std::{fmt, sync::Arc};
use tokio_zmq::{prelude::*, Multipart, Router};


/* --------------------------------Envelope---------------------------------- */

struct Envelope {
    addr: zmq::Message,
    empty: zmq::Message,
    request: zmq::Message,
}

impl From<Envelope> for Multipart {
    fn from(e: Envelope) -> Self {
        let mut multipart = Multipart::new();

        multipart.push_back(e.addr);
        multipart.push_back(e.empty);
        multipart.push_back(e.request);

        multipart
    }
}

/* ----------------------------------Error----------------------------------- */

#[derive(Debug)]
enum Error {
    Zmq(zmq::Error),
    TokioZmq(tokio_zmq::Error),
    WorkerSend,
    WorkerRecv,
    NotEnoughMessages,
    TooManyMessages,
    MsgNotEmpty,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Zmq(ref e) => write!(f, "Error in ZeroMQ: {}", e),
            Error::TokioZmq(ref e) => write!(f, "Error in Tokio ZMQ: {}", e),
            Error::WorkerSend => write!(f, "Error sending to worker"),
            Error::WorkerRecv => write!(f, "Error receiving from worker"),
            Error::NotEnoughMessages => write!(f, "Not enough messages"),
            Error::TooManyMessages => write!(f, "Too many messages"),
            Error::MsgNotEmpty => write!(f, "Message not empty"),
        }
    }
}

impl From<tokio_zmq::Error> for Error {
    fn from(e: tokio_zmq::Error) -> Self {
        Error::TokioZmq(e)
    }
}

impl From<zmq::Error> for Error {
    fn from(e: zmq::Error) -> Self {
        Error::Zmq(e)
    }
}

/* -----------------------------------Stop----------------------------------- */

struct Stop(&'static str, usize);

impl ControlHandler for Stop {
    fn should_stop(&mut self, _: Multipart) -> bool {
        debug!("Received stop signal! {}/{}", self.0, self.1);
        true
    }
}

/* ----------------------------------broker---------------------------------- */
async fn broker_task() {
    let context = Arc::new(zmq::Context::new());

    let frontend_fut = Router::builder(Arc::clone(&context))
        .bind("tcp://*:5672")
        .build()
        .compat();

    let backend_fut = Router::builder(context)
        .bind("tcp://*:5673")
        .build()
        .compat();

    let (frontend, backend) = try_join!(frontend_fut, backend_fut).unwrap();
    let (mut frontend_sink, mut frontend_stream) = frontend.sink_stream(25).sink_compat().split();
    let (mut backend_sink, mut backend_stream) = backend.sink_stream(25).sink_compat().split();

    let (s, r) = unbounded::<zmq::Message>();

    let front2back = async move {
        while let Some(multipart) = frontend_stream.next().await {
            let mut multipart = multipart.unwrap();
            debug!("Received {:?}", multipart);
            let worker_id = r.recv().unwrap();
            debug!("Work with {:?}", worker_id);
            let client_id = multipart
                .pop_front()
                .ok_or(Error::NotEnoughMessages)
                .unwrap();
            let empty = multipart
                .pop_front()
                .ok_or(Error::NotEnoughMessages)
                .unwrap();
            let body = multipart
                .pop_front()
                .ok_or(Error::NotEnoughMessages)
                .unwrap();
            assert!(empty.is_empty());
            let mut message = Multipart::new();
            message.push_back(worker_id);
            message.push_back(empty);
            message.push_back(client_id);
            message.push_back(body);
            backend_sink.send(message).await.unwrap();
        }
    };
    let router1_thread = tokio::spawn(front2back);

    let back2front = async move {
        while let Some(multipart) = backend_stream.next().await {
            let mut multipart = multipart.unwrap();
            debug!("Received {:?}", multipart);
            let worker_id = multipart
                .pop_front()
                .ok_or(Error::NotEnoughMessages)
                .unwrap();
            let empty = multipart
                .pop_front()
                .ok_or(Error::NotEnoughMessages)
                .unwrap();
            let body = multipart
                .pop_front()
                .ok_or(Error::NotEnoughMessages)
                .unwrap();

            assert!(empty.is_empty());
            debug!("Sent worker_id {:?}", worker_id);
            s.send(worker_id).unwrap();
            if &*body != b"READY" {
                let client_id = body;
                let result = multipart
                    .pop_front()
                    .ok_or(Error::NotEnoughMessages)
                    .unwrap();

                let mut response = Multipart::new();
                response.push_back(client_id);
                response.push_back(empty);
                response.push_back(result);
                frontend_sink.send(response).await.unwrap();
            } else {
                debug!("New worker was found.");
            }
        }
    };
    let router2_thread = tokio::spawn(back2front);
    try_join!(router1_thread, router2_thread).unwrap();
}

#[tokio::main]
async fn main() {
    let exe_path = match env::current_exe() {
        Ok(exe_path) => exe_path,
        Err(e) => panic!("failed to get current exe path: {}", e),
    };
    let exe_path = match exe_path.parent() {
        Some(exe_path) => exe_path,
        None => panic!("failed to get parent path"),
    };
    if cfg!(debug_assertions) {
        log4rs::init_file(
            exe_path.join("config/debug/log4rs.yaml"),
            Default::default(),
        ).unwrap();
    } else {
        log4rs::init_file(
            exe_path.join("config/release/log4rs.yaml"),
            Default::default(),
        ).unwrap();
    }

    broker_task().await;
}
