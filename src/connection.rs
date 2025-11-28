use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;

use anyhow::Context;
use anyhow::Result;
use dap::base_message::Sendable;
use dap::errors::ServerError;
use dap::prelude::{Event, Request, ResponseBody, Server};
use dap::server::{ServerReader, ServerWriter};

pub struct Connection {
    inbound_rx: mpsc::Receiver<Request>,
    outbound_tx: mpsc::Sender<Sendable>,
    _io_threads: IoThreads,
}

struct IoThreads {
    pub reader: Option<JoinHandle<()>>,
    pub writer: Option<JoinHandle<()>>,
}

impl Connection {
    pub fn new() -> Result<Self> {
        let tcp_listener = TcpListener::bind("127.0.0.1:0").map_err(ServerError::IoError)?;
        let os_assigned_port = tcp_listener.local_addr()?.port();
        // Print it so that the client can read it.
        println!("\nDEBUGGER PORT: {os_assigned_port}");

        let (stream, _client_addr) = tcp_listener.accept().map_err(ServerError::IoError)?;
        let input = BufReader::new(stream.try_clone()?);
        let output = BufWriter::new(stream);

        let (server_reader, server_writer) = Server::new(input, output).split_server();

        let (inbound_tx, inbound_rx) = mpsc::channel::<Request>();
        let (outbound_tx, outbound_rx) = mpsc::channel::<Sendable>();

        Ok(Self {
            inbound_rx,
            outbound_tx,
            _io_threads: IoThreads::spawn(server_reader, server_writer, inbound_tx, outbound_rx),
        })
    }

    pub fn next_request(&self) -> Result<Request> {
        self.inbound_rx.recv().context("Connection close")
    }

    pub fn send_event(&self, event: Event) -> Result<()> {
        self.outbound_tx
            .send(Sendable::Event(event))
            .context("Sending event to outbound channel failed")
    }

    pub fn send_success(&self, request: Request, body: ResponseBody) -> Result<()> {
        self.outbound_tx
            .send(Sendable::Response(request.success(body)))
            .context("Sending success response to outbound channel failed")
    }

    pub fn send_error(&self, request: Request, msg: &str) -> Result<()> {
        self.outbound_tx
            .send(Sendable::Response(request.error(msg)))
            .context("Sending error response to outbound channel failed")
    }
}

impl IoThreads {
    fn spawn(
        server_reader: ServerReader<TcpStream>,
        server_writer: ServerWriter<TcpStream>,
        inbound_tx: mpsc::Sender<Request>,
        outbound_rx: mpsc::Receiver<Sendable>,
    ) -> Self {
        Self {
            reader: Some(spawn_reader_thread(server_reader, inbound_tx)),
            writer: Some(spawn_writer_thread(server_writer, outbound_rx)),
        }
    }
}

impl Drop for IoThreads {
    fn drop(&mut self) {
        self.reader.take().map(|h| h.join());
        self.writer.take().map(|h| h.join());
    }
}

fn spawn_reader_thread(
    mut server_reader: ServerReader<TcpStream>,
    inbound_tx: mpsc::Sender<Request>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(Some(request)) = server_reader.poll_request() {
            if inbound_tx.send(request).is_err() {
                // TODO: Add error tracing
                break;
            }
        }
    })
}

fn spawn_writer_thread(
    mut server_writer: ServerWriter<TcpStream>,
    outbound_rx: mpsc::Receiver<Sendable>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(msg) = outbound_rx.recv() {
            match msg {
                Sendable::Response(response) => {
                    server_writer.respond(response).expect("Failed to send response")
                }
                Sendable::Event(event) => {
                    server_writer.send_event(event).expect("Failed to send event")
                }
                Sendable::ReverseRequest(_) => unreachable!(),
            }
        }
    })
}
