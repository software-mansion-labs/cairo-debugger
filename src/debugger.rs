use anyhow::{Result, bail};
use cairo_vm::vm::vm_core::VirtualMachine;

use crate::connection::Connection;
use crate::debugger::handler::{HandleResult, NextAction};

mod handler;
mod log;
mod vm;

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
            let request = self.connection.next_request()?;
            if let HandleResult::Trigger(NextAction::FinishInit) = self.handle_request(request)? {
                break;
            }
        }

        Ok(())
    }

    fn sync(&self, _vm: &VirtualMachine) -> Result<()> {
        if let Some(request) = self.connection.try_next_request()?
            && let HandleResult::Trigger(NextAction::Stop) = self.handle_request(request)?
        {
            self.process_until_resume()?;
        }

        Ok(())
    }

    fn process_until_resume(&self) -> Result<()> {
        loop {
            let request = self.connection.next_request()?;
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

    pub fn init_logging() -> Option<impl Drop> {
        log::init_logging()
    }
}
