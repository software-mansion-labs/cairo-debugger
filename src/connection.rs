use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use anyhow::Context;
use anyhow::Result;
use dap::base_message::Sendable;
use dap::errors::ServerError;
use dap::prelude::{Event, Request, ResponseBody, Server};
use dap::server::{ServerReader, ServerWriter};

pub struct Connection {
    inbound_rx: mpsc::Receiver<Request>,
    outbound_tx: mpsc::Sender<Sendable>,
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

        run_reader_thread(server_reader, inbound_tx);
        run_writer_thread(server_writer, outbound_rx);

        Ok(Self { inbound_rx, outbound_tx })
    }

    pub fn next_request(&self) -> Option<Request> {
        self.inbound_rx.recv().ok()
    }

    pub fn try_next_request(&self) -> Option<Request> {
        self.inbound_rx.try_recv().ok()
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

fn run_reader_thread(
    mut server_reader: ServerReader<TcpStream>,
    inbound_tx: mpsc::Sender<Request>,
) {
    thread::spawn(move || {
        while let Ok(Some(request)) = server_reader.poll_request() {
            if inbound_tx.send(request).is_err() {
                // TODO: Add error tracing
                break;
            }
        }
    });
}

fn run_writer_thread(
    mut server_writer: ServerWriter<TcpStream>,
    outbound_rx: mpsc::Receiver<Sendable>,
) {
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
    });
}
