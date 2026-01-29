use std::iter::once;
use std::path::Path;

use cairo_annotations::annotations::coverage::CodeLocation;
use cairo_lang_sierra::program::StatementIdx;
use dap::types::StackFrame;
use dap::types::{Source, StackFramePresentationhint};

use crate::debugger::context::Context;

#[derive(Default)]
pub struct CallStack {
    /// Stack of call frames. Does ***not*** contain a current function frame.
    call_frames: Vec<StackFrame>,

    /// Modification that should be applied to the stack when a new sierra statement is reached.
    ///
    /// This field is there to ensure that a correct stack trace is returned when a current
    /// statement maps to a function call or a return statement.
    /// The stack should be modified ***after*** such a statement is executed.
    action_on_new_statement: Option<Action>,
}

enum Action {
    Push(StackFrame),
    Pop,
}

impl CallStack {
    pub fn update(&mut self, statement_idx: StatementIdx, ctx: &Context) {
        // We can be sure that the `statement_idx` is different from the one which was the arg when
        // `action_on_new_statement` was set.
        // The reason is that both function call and return in sierra compile to one CASM instruction each.
        // https://github.com/starkware-libs/cairo/blob/20eca60c88a35f7da13f573b2fc68818506703a9/crates/cairo-lang-sierra-to-casm/src/invocations/function_call.rs#L46
        // https://github.com/starkware-libs/cairo/blob/d52acf845fc234f1746f814de7c64b535563d479/crates/cairo-lang-sierra-to-casm/src/compiler.rs#L533
        match self.action_on_new_statement.take() {
            Some(Action::Push(frame)) => {
                self.call_frames.push(frame);
            }
            Some(Action::Pop) => {
                self.call_frames.pop();
            }
            None => {}
        }

        if ctx.is_function_call_statement(statement_idx) {
            self.action_on_new_statement =
                Some(Action::Push(build_stack_frame(ctx, statement_idx)));
        } else if ctx.is_return_statement(statement_idx) {
            self.action_on_new_statement = Some(Action::Pop);
        }
    }

    pub fn get_frames(&self, statement_idx: StatementIdx, ctx: &Context) -> Vec<StackFrame> {
        let current_frame = build_stack_frame(ctx, statement_idx);
        // DAP expects frames to start from the most nested element.
        self.call_frames.iter().cloned().chain(once(current_frame)).rev().collect()
    }
}

fn build_stack_frame(ctx: &Context, statement_idx: StatementIdx) -> StackFrame {
    match ctx.code_location_for_statement_idx(statement_idx) {
        Some(CodeLocation(source_file, code_span, _)) => {
            let file_path = Path::new(&source_file.0);
            let name = ctx
                .function_name_for_statement_idx(statement_idx)
                .map(|name| name.0)
                .unwrap_or("test".to_string());

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
