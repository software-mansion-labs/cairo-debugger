use dap::types::StackFrame;

use crate::debugger::context::Context;
use crate::debugger::handler::stack_trace::build_stack_frame;

#[derive(Default)]
pub struct CallStack {
    call_frames: Vec<StackFrame>,
    current_frame: Option<StackFrame>,
}

impl CallStack {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn update(&mut self, current_pc: usize, ctx: &Context) {
        if ctx.is_function_call_statement(current_pc) {
            self.call_frames.push(build_stack_frame(ctx, current_pc));
            self.current_frame = None;
        } else if ctx.is_return_statement(current_pc) {
            self.call_frames.pop();
            self.current_frame = None;
        } else {
            self.current_frame = Some(build_stack_frame(ctx, current_pc));
        }
    }

    pub fn get_frames(&self) -> Vec<StackFrame> {
        self.call_frames.iter().cloned().chain(self.current_frame.iter().cloned()).collect()
    }
}
