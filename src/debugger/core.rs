use crate::client::connection::Connection;
use crate::debugger::handlers::{HandleResult, NextAction};
use anyhow::{Result, anyhow, bail};
use dap::events::Event::{Exited, Terminated};
use dap::events::ExitedEventBody;
use dap::prelude::{Command, ResponseBody};

pub struct CairoDebugger {
    pub(crate) connection: Connection,
}

impl CairoDebugger {
    pub fn connect() -> Result<Self> {
        let connection = Connection::new()?;
        Ok(Self { connection })
    }

    pub fn run(&self) -> Result<()> {
        self.initialize()?;
        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        loop {
            let request =
                self.connection.next_request().ok_or_else(|| anyhow!("Connection closed"))?;
            match self.handle_request(request)? {
                HandleResult::Trigger(NextAction::FinishInit) => break,
                _ => continue,
            }
        }

        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        if let Some(request) = self.connection.try_next_request()
            && let HandleResult::Trigger(NextAction::Stop) = self.handle_request(request)?
        {
            self.process_until_resume()?;
        }

        Ok(())
    }

    fn process_until_resume(&self) -> Result<()> {
        loop {
            let request =
                self.connection.next_request().ok_or_else(|| anyhow!("Connection closed"))?;
            match self.handle_request(request)? {
                HandleResult::Trigger(NextAction::Resume) => break,
                HandleResult::Trigger(NextAction::FinishInit) => {
                    bail!("Unexpected request received during execution");
                }
                HandleResult::Handled | HandleResult::Trigger(NextAction::Stop) => {}
            }
        }

        Ok(())
    }
}

impl Drop for CairoDebugger {
    fn drop(&mut self) {
        // TODO: Add error tracing
        // TODO: Send correct exit code
        self.connection.send_event(Exited(ExitedEventBody { exit_code: 0 })).ok();
        self.connection.send_event(Terminated(None)).ok();

        if let Some(request) = self.connection.try_next_request()
            && let Command::Disconnect(_) = request.command
        {
            self.connection.send_success(request, ResponseBody::Disconnect).ok();
        }
    }
}
