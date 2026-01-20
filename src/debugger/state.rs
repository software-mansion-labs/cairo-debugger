use std::collections::{HashMap, HashSet};
use std::path::Path;

use cairo_vm::vm::vm_core::VirtualMachine;
use tracing::{debug, trace};

use crate::debugger::call_stack::CallStack;
use crate::debugger::context::{Context, Line};

type SourcePath = String;

pub struct State {
    configuration_done: bool,
    execution_stopped: bool,
    pub breakpoints: HashMap<SourcePath, HashSet<usize>>,
    pub current_pc: usize,
    pub call_stack: CallStack,
}

impl State {
    pub fn new() -> Self {
        Self {
            configuration_done: false,
            execution_stopped: false,
            breakpoints: HashMap::default(),
            current_pc: 0,
            call_stack: CallStack::default(),
        }
    }

    pub fn update_state(&mut self, vm: &VirtualMachine, ctx: &Context) {
        self.current_pc = vm.get_pc().offset;
        self.call_stack.update(self.current_pc, ctx)
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
