use std::path::Path;

use cairo_annotations::annotations::coverage::CodeLocation;
use dap::types::{Source, StackFrame, StackFramePresentationhint};

use crate::debugger::context::Context;

pub fn build_stack_frame(ctx: &Context, pc: usize) -> StackFrame {
    match ctx.map_pc_to_code_location(pc) {
        Some(CodeLocation(source_file, code_span, _)) => {
            let file_path = Path::new(&source_file.0);
            let is_user_code = file_path.starts_with(&ctx.root_path);

            StackFrame {
                id: 1,
                name: "test".to_string(),
                source: Some(Source {
                    name: None,
                    path: Some(source_file.0),
                    ..Default::default()
                }),
                // Annotations from debug info are 0-indexed.
                // UI expects 1-indexed, hence +1 below.
                line: (code_span.start.line.0 + 1) as i64,
                column: (code_span.start.col.0 + 1) as i64,
                presentation_hint: Some(if is_user_code {
                    StackFramePresentationhint::Normal
                } else {
                    StackFramePresentationhint::Subtle
                }),
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
