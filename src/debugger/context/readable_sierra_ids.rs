use std::collections::{HashMap, HashSet};

use cairo_lang_sierra::program::{
    GenBranchInfo, GenBranchTarget, GenInvocation, GenStatement, Program,
};

// https://github.com/starkware-libs/cairo/blob/64b88f06c6261ac67c6b478434c844d4af81e5a3/crates/cairo-lang-sierra/src/fmt.rs#L29
pub fn extract_labels(program: &Program) -> HashMap<usize, String> {
    let funcs_labels = HashMap::<usize, String>::from_iter(
        program.funcs.iter().enumerate().map(|(i, f)| (f.entry_point.0, format!("F{i}"))),
    );
    // The offsets of branch targets.
    let mut block_offsets = HashSet::<usize>::default();
    for s in &program.statements {
        replace_statement_id(s.clone(), |idx| {
            block_offsets.insert(idx.0);
        });
    }
    // All labels including inner function labels.
    let mut labels = funcs_labels.clone();
    // Starting as `NONE` for support of invalid Sierra code.
    let mut function_label = "NONE".to_string();
    // Assuming function code is contiguous - this is the index for same function labels.
    let mut inner_idx = 0;
    for i in 0..program.statements.len() {
        if let Some(label) = funcs_labels.get(&i) {
            function_label = label.clone();
            inner_idx = 0;
        } else if block_offsets.contains(&i) {
            labels.insert(i, format!("{function_label}_B{inner_idx}"));
            inner_idx += 1;
        }
    }

    labels
}

// https://github.com/starkware-libs/cairo/blob/c539d077479654eee6323d9c0c6eafad82d4851a/crates/cairo-lang-sierra/src/labeled_statement.rs#L25
pub fn replace_statement_id<StatementIdIn, StatementIdOut>(
    statement: GenStatement<StatementIdIn>,
    mut map_stmt_id: impl FnMut(StatementIdIn) -> StatementIdOut,
) -> GenStatement<StatementIdOut> {
    match statement {
        GenStatement::Invocation(GenInvocation { libfunc_id, args, branches }) => {
            GenStatement::Invocation(GenInvocation {
                libfunc_id,
                args,
                branches: branches
                    .into_iter()
                    .map(|GenBranchInfo { results, target }| GenBranchInfo {
                        results,
                        target: match target {
                            GenBranchTarget::Fallthrough => GenBranchTarget::Fallthrough,
                            GenBranchTarget::Statement(statement_id) => {
                                GenBranchTarget::Statement(map_stmt_id(statement_id))
                            }
                        },
                    })
                    .collect(),
            })
        }
        GenStatement::Return(vars) => GenStatement::Return(vars),
    }
}
