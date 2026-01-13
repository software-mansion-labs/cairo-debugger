use std::collections::{HashMap, HashSet};
use std::path::Path;

use tracing::{debug, trace};

use crate::debugger::context::{Context, Line};

type SourcePath = String;

pub struct State {
    configuration_done: bool,
    execution_stopped: bool,
    pub breakpoints: HashMap<SourcePath, HashSet<usize>>,
    pub current_pc: usize,
}

impl State {
    pub fn new() -> Self {
        Self {
            configuration_done: false,
            execution_stopped: false,
            breakpoints: HashMap::default(),
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

    pub fn verify_and_set_breakpoint(
        &mut self,
        source: SourcePath,
        line: Line,
        ctx: &Context,
    ) -> bool {
        let pc = ctx.get_pc_for_line(Path::new(&source), line);

        if let Some(pc) = pc {
            debug!("Setting breakpoint for file: {:?}, line: {:?}", source, line);
            self.breakpoints.entry(source).or_default().insert(pc);

            return true;
        }

        false
    }

    pub fn clear_breakpoints(&mut self, source: &SourcePath) {
        self.breakpoints.remove(source);
    }
}
