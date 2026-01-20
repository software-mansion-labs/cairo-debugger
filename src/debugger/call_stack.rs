use std::iter::once;
use std::path::Path;

use cairo_annotations::annotations::coverage::CodeLocation;
use dap::types::StackFrame;
use dap::types::{Source, StackFramePresentationhint};

use crate::debugger::context::Context;

#[derive(Default)]
pub struct CallStack {
    call_frames: Vec<StackFrame>,
}

impl CallStack {
    pub fn update(&mut self, current_pc: usize, ctx: &Context) {
        if ctx.is_function_call_statement(current_pc) {
            self.call_frames.push(build_stack_frame(ctx, current_pc));
        } else if ctx.is_return_statement(current_pc) {
            self.call_frames.pop();
        }
    }

    pub fn get_frames(&self, current_pc: usize, ctx: &Context) -> Vec<StackFrame> {
        let current_frame = build_stack_frame(ctx, current_pc);
        // DAP expects frames to start from the most nested element.
        self.call_frames.iter().cloned().chain(once(current_frame)).rev().collect()
    }
}

fn build_stack_frame(ctx: &Context, pc: usize) -> StackFrame {
    match ctx.map_pc_to_code_location(pc) {
        Some(CodeLocation(source_file, code_span, _)) => {
            let file_path = Path::new(&source_file.0);
            let name =
                ctx.map_pc_to_function_name(pc).map(|name| name.0).unwrap_or("test".to_string());

            let is_user_code = file_path.starts_with(&ctx.root_path);
            let presentation_hint = Some(if is_user_code {
                StackFramePresentationhint::Normal
            } else {
                StackFramePresentationhint::Subtle
            });

            // Annotations from debug info are 0-indexed.
            // UI expects 1-indexed, hence +1 below.
            let line = (code_span.start.line.0 + 1) as i64;
            let column = (code_span.start.col.0 + 1) as i64;

            StackFrame {
                id: 1,
                name,
                source: Some(Source {
                    name: None,
                    path: Some(source_file.0),
                    ..Default::default()
                }),
                line,
                column,
                presentation_hint,
                ..Default::default()
            }
        }
        None => StackFrame {
            id: 1,
            name: "Unknown".to_string(),
            line: 1,
            column: 1,
            presentation_hint: Some(StackFramePresentationhint::Subtle),
            ..Default::default()
        },
    }
}
