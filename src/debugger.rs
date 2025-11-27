use dap::events::Event;
use dap::prelude::ResponseBody;

use crate::connection::Connection;

mod handler;

pub struct CairoDebugger {
    connection: Connection,
}

enum ServerResponse {
    Success(ResponseBody),
    Error(String),
    Event(Event),
    SuccessThenEvent(ResponseBody, Event),
}

impl CairoDebugger {
    pub fn connect() -> anyhow::Result<Self> {
        let connection = Connection::new()?;
        Ok(Self { connection })
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        while let Ok(req) = self.connection.next_request() {
            match handler::handle_request(&req) {
                ServerResponse::Success(body) => self.connection.send_success(req, body)?,
                ServerResponse::Error(msg) => self.connection.send_error(req, &msg)?,
                ServerResponse::Event(event) => self.connection.send_event(event)?,
                ServerResponse::SuccessThenEvent(body, event) => {
                    self.connection.send_success(req, body)?;
                    self.connection.send_event(event)?;
                }
            }
        }

        Ok(())
    }
}
