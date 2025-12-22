use std::collections::HashSet;
use std::path::Path;

use tracing::{debug, trace};

use crate::debugger::context::{Context, Line};

pub struct State {
    configuration_done: bool,
    execution_stopped: bool,
    pub breakpoints: HashSet<usize>,
    pub current_pc: usize,
}

impl State {
    pub fn new() -> Self {
        Self {
            configuration_done: false,
            execution_stopped: false,
            breakpoints: HashSet::default(),
            current_pc: 0,
        }
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

    pub fn set_breakpoint(&mut self, source: String, line: Line, ctx: &Context) {
        debug!("setting breakpoint for file: {:?}, line: {:?}", source, line);

        let pc = ctx.get_pc_for_line(Path::new(&source), line);

        if let Some(pc) = pc {
            self.breakpoints.insert(pc);
        }
    }
}
