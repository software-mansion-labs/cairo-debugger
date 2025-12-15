use anyhow::Result;
use cairo_vm::vm::vm_core::VirtualMachine;
use camino::Utf8Path;
use dap::events::ExitedEventBody;
use dap::prelude::Event::{Exited, Terminated};
use dap::prelude::Request;
use tracing::error;

use crate::connection::Connection;
use crate::debugger::context::Context;
use crate::debugger::state::State;

mod context;
mod handler;
mod state;
mod vm;

pub struct CairoDebugger {
    connection: Connection,
    ctx: Context,
    state: State,
}

impl CairoDebugger {
    pub fn connect_and_initialize(sierra_path: &Utf8Path) -> Result<Self> {
        let connection = Connection::new()?;
        let ctx = Context::new(sierra_path);

        let mut debugger = Self { connection, ctx, state: State::new() };
        debugger.initialize()?;

        Ok(debugger)
    }

    fn initialize(&mut self) -> Result<()> {
        while !self.state.is_configuration_done() {
            // TODO(#35)
            let request = self.connection.next_request()?;
            self.process_request(request)?;
        }

        Ok(())
    }

    fn sync_with_vm(&mut self, _vm: &VirtualMachine) -> Result<()> {
        while let Some(request) = self.connection.try_next_request()? {
            self.process_request(request)?;

            if self.state.is_execution_stopped() {
                self.process_until_resume()?;
            }
        }

        Ok(())
    }

    fn process_until_resume(&mut self) -> Result<()> {
        while self.state.is_execution_stopped() {
            let request = self.connection.next_request()?;
            self.process_request(request)?;
        }

        Ok(())
    }

    fn process_request(&mut self, request: Request) -> Result<()> {
        let response = handler::handle_request(&request, &mut self.state, &self.ctx)?;
        if let Some(event) = response.event {
            self.connection.send_event(event)?;
        }
        self.connection.send_success(request, response.response_body)?;

        Ok(())
    }
}

impl Drop for CairoDebugger {
    fn drop(&mut self) {
        if let Err(err) = self.connection.send_event(Terminated(None)) {
            error!("Sending terminated event failed: {}", err);
        }

        // TODO(#34): Send correct exit code
        if let Err(err) = self.connection.send_event(Exited(ExitedEventBody { exit_code: 0 })) {
            error!("Sending exit event failed: {}", err);
        }
    }
}
