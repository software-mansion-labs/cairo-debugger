use anyhow::Result;

use crate::connection::Connection;

mod handler;

pub struct CairoDebugger {
    connection: Connection,
}

impl CairoDebugger {
    pub fn connect() -> Result<Self> {
        let connection = Connection::new()?;
        Ok(Self { connection })
    }

    pub fn run(&self) -> Result<()> {
        while let Ok(req) = self.connection.next_request() {
            self.handle_request(req)?;
        }

        Ok(())
    }
}
