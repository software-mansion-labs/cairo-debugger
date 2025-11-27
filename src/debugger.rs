use anyhow::{Result, bail};
use cairo_vm::vm::vm_core::VirtualMachine;
use tracing::debug;

use crate::connection::Connection;
use crate::debugger::handler::{HandleResult, NextAction};

mod handler;
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
            // TODO(#35)
            let request = self.connection.next_request()?;
            if let HandleResult::Trigger(NextAction::FinishInit) = self.handle_request(request)? {
                debug!("Initialization finished");
                break;
            }
        }

        Ok(())
    }

    fn sync_with_vm(&self, _vm: &VirtualMachine) -> Result<()> {
        while let Some(request) = self.connection.try_next_request()?
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
}
