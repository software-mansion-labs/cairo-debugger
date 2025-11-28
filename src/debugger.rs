use anyhow::Result;
use tracing::debug;

use crate::connection::Connection;
use crate::debugger::handler::{HandleResult, NextAction};

mod handler;
mod log;

pub struct CairoDebugger {
    connection: Connection,
}

impl CairoDebugger {
    pub fn connect_and_initialize() -> Result<Self> {
        let connection = Connection::new()?;
        let debugger = Self { connection };
        debugger.initialize()?;

        Ok(debugger)
    }

    fn initialize(&self) -> Result<()> {
        loop {
            // TODO(#35)
            let request = self.connection.next_request()?;
            if let HandleResult::Trigger(NextAction::FinishInit) = self.handle_request(request)? {
                debug!("Initialization finished");
                break;
            }
        }

        Ok(())
    }

    pub fn run(&self) -> Result<()> {
        while let Ok(req) = self.connection.next_request() {
            self.handle_request(req)?;
        }

        Ok(())
    }

    pub fn init_logging() -> Option<impl Drop> {
        log::init_logging()
    }
}
