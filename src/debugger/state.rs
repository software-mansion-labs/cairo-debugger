use tracing::trace;

pub struct State {
    configuration_done: bool,
    execution_stopped: bool,
}

impl State {
    pub fn new() -> Self {
        Self { configuration_done: false, execution_stopped: false }
    }

    pub fn is_configuration_done(&self) -> bool {
        self.configuration_done
    }

    pub fn set_configuration_done(&mut self) {
        trace!("Configuration done");
        self.configuration_done = true;
    }

    pub fn is_execution_stopped(&self) -> bool {
        self.execution_stopped
    }

    pub fn stop_execution(&mut self) {
        trace!("Execution stopped");
        self.execution_stopped = true;
    }

    pub fn resume_execution(&mut self) {
        trace!("Execution resumed");
        self.execution_stopped = false;
    }
}
