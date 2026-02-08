use std::iter::once;
use std::path::Path;

use cairo_annotations::annotations::coverage::{CodeLocation, SourceFileFullPath};
use cairo_annotations::annotations::profiler::FunctionName;
use cairo_lang_sierra::program::StatementIdx;
use dap::types::{Scope, ScopePresentationhint, StackFrame, Variable};
use dap::types::{Source, StackFramePresentationhint};

use crate::debugger::MIN_OBJECT_REFERENCE;
use crate::debugger::context::Context;

#[derive(Default)]
pub struct CallStack {
    /// Stack of indexes of sierra statements that are function calls and values of variables in frames corresponding to these functions.
    /// Does ***not*** contain a current function frame.
    ///
    /// [Object references](https://microsoft.github.io/debug-adapter-protocol/overview#lifetime-of-objects-references):
    /// object reference for each stack frame is equal to its `1 + 2 * index` where `index` is its
    /// position in this vector. For variables, it is `2 + 2 * index`.
    call_ids: Vec<(StatementIdx, FunctionVariables)>,

    /// Modification that should be applied to the stack when a new sierra statement is reached.
    ///
    /// This field is there to ensure that a correct stack trace is returned when a current
    /// statement maps to a function call or a return statement.
    /// The stack should be modified ***after*** such a statement is executed.
    action_on_new_statement: Option<Action>,
}

enum Action {
    Push(StatementIdx),
    Pop,
}

impl CallStack {
    pub fn depth(&self) -> usize {
        self.call_ids.len()
    }
    pub fn update(&mut self, statement_idx: StatementIdx, ctx: &Context) {
        // We can be sure that the `statement_idx` is different from the one which was the arg when
        // `action_on_new_statement` was set.
        // The reason is that both function call and return in sierra compile to one CASM instruction each.
        // https://github.com/starkware-libs/cairo/blob/20eca60c88a35f7da13f573b2fc68818506703a9/crates/cairo-lang-sierra-to-casm/src/invocations/function_call.rs#L46
        // https://github.com/starkware-libs/cairo/blob/d52acf845fc234f1746f814de7c64b535563d479/crates/cairo-lang-sierra-to-casm/src/compiler.rs#L533
        match self.action_on_new_statement.take() {
            Some(Action::Push(statement)) => {
                // TODO(#16)
                self.call_ids.push((statement, FunctionVariables {}));
            }
            Some(Action::Pop) => {
                self.call_ids.pop();
            }
            None => {}
        }

        if ctx.is_function_call_statement(statement_idx) {
            self.action_on_new_statement = Some(Action::Push(statement_idx));
        } else if ctx.is_return_statement(statement_idx) {
            self.action_on_new_statement = Some(Action::Pop);
        }
    }

    pub fn get_frames(&self, statement_idx: StatementIdx, ctx: &Context) -> Vec<StackFrame> {
        // DAP expects frames to start from the most nested element.
        self.call_ids
            .iter()
            .map(|(call_statement_idx, _)| call_statement_idx)
            .cloned()
            .chain(once(statement_idx))
            .rev()
            .flat_map(|statement_idx| self.build_stack_frames(ctx, statement_idx))
            .collect()
    }

    pub fn get_scopes_for_frame(&self, frame_id: i64) -> Vec<Scope> {
        let scope = Scope {
            name: "Locals".to_string(),
            variables_reference: frame_id + 1,
            presentation_hint: Some(ScopePresentationhint::Locals),
            ..Default::default()
        };
        vec![scope]
    }

    pub fn get_variables(&self, variables_reference: i64) -> Vec<Variable> {
        let index = variables_reference / 2 - 1;
        let &FunctionVariables {} = if index == self.call_ids.len() as i64 {
            // TODO(#16)
            //  Build them on demand.
            &FunctionVariables {}
        } else {
            &self.call_ids[index as usize].1
        };

        vec![]
    }

    /// Builds a vector of stack frames, ordered from the most nested (innermost) to the least nested (outermost) element.
    fn build_stack_frames(&self, ctx: &Context, statement_idx: StatementIdx) -> Vec<StackFrame> {
        let Some(code_locations) = ctx.code_locations_for_statement_idx(statement_idx) else {
            return vec![unknown_frame()];
        };

        let default_function_names = vec![FunctionName("test".to_string())];
        let function_names =
            ctx.function_names_for_statement_idx(statement_idx).unwrap_or(&default_function_names);

        code_locations
            .iter()
            .zip(function_names)
            .map(|(code_location, function_name)| {
                self.build_stack_frame(code_location, function_name, ctx)
            })
            .collect()
    }

    fn build_stack_frame(
        &self,
        CodeLocation(SourceFileFullPath(source_file), code_span, _): &CodeLocation,
        FunctionName(function_name): &FunctionName,
        ctx: &Context,
    ) -> StackFrame {
        let id = MIN_OBJECT_REFERENCE + 2 * self.call_ids.len() as i64;
        let file_path = Path::new(&source_file);
        let name = function_name.clone();

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
            id,
            name,
            source: Some(Source {
                name: None,
                path: Some(source_file.clone()),
                ..Default::default()
            }),
            line,
            column,
            presentation_hint,
            ..Default::default()
        }
    }
}

fn unknown_frame() -> StackFrame {
    StackFrame {
        id: 1,
        name: "Unknown".to_string(),
        line: 1,
        column: 1,
        presentation_hint: Some(StackFramePresentationhint::Subtle),
        ..Default::default()
    }
}

// TODO(#16)
struct FunctionVariables {}
