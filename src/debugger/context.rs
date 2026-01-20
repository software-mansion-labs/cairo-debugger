use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, anyhow};
use cairo_annotations::annotations::TryFromDebugInfo;
use cairo_annotations::annotations::coverage::{
    CodeLocation, CoverageAnnotationsV1 as SierraCodeLocations, SourceCodeSpan,
};
use cairo_annotations::annotations::debugger::{
    DebuggerAnnotationsV1 as FunctionsDebugInfo, SierraFunctionId, SierraVarId,
};
use cairo_annotations::annotations::profiler::{
    FunctionName, ProfilerAnnotationsV1 as SierraFunctionNames,
};
use cairo_lang_casm::cell_expression::CellExpression;
use cairo_lang_casm::operand::Register;
use cairo_lang_sierra::extensions::core::{CoreConcreteLibfunc, CoreLibfunc, CoreType};
use cairo_lang_sierra::program::{
    GenBranchInfo, GenBranchTarget, GenInvocation, GenStatement, Program, ProgramArtifact,
    Statement, StatementIdx,
};
use cairo_lang_sierra::program_registry::ProgramRegistry;
use cairo_lang_sierra_to_casm::compiler::{
    CairoProgramDebugInfo, SierraToCasmConfig, StatementKindDebugInfo,
};
use cairo_lang_sierra_to_casm::metadata::calc_metadata;
use cairo_lang_sierra_to_casm::references::ReferenceExpression;
use cairo_vm::types::relocatable::MaybeRelocatable;
use cairo_vm::vm::vm_core::VirtualMachine;
use scarb_metadata::MetadataCommand;
use starknet_types_core::felt::Felt;
use tracing::trace;

/// Struct that holds all the initial data needed for the debugger during execution.
pub struct Context {
    pub root_path: PathBuf,
    casm_debug_info: CairoProgramDebugInfo,
    code_locations: SierraCodeLocations,
    function_names: SierraFunctionNames,
    files_data: HashMap<PathBuf, FileCodeLocationsData>,
    program: Program,
    sierra_program_registry: ProgramRegistry<CoreType, CoreLibfunc>,
    cairo_var_to_casm:
        HashMap<StatementIdx, HashMap<(String, SourceCodeSpan), ReferenceExpression>>,
    labels: HashMap<usize, String>,
}

pub struct CasmDebugInfo {
    /// Sierra statement index -> start CASM bytecode offset
    pub statement_to_pc: Vec<usize>,
}

struct FileCodeLocationsData {
    /// Line number -> start CASM bytecode offset
    lines: HashMap<Line, usize>,
}

/// Line number in a file, 0-indexed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Default)]
pub struct Line(usize);

impl Line {
    pub fn new(line: usize) -> Self {
        Self(line)
    }
}

impl Context {
    pub fn new(sierra_path: &Path, casm_debug_info: CasmDebugInfo) -> Result<Self> {
        let root_path = get_project_root_path(sierra_path)?;

        let content = fs::read_to_string(sierra_path).expect("Failed to load sierra file");
        let sierra_program: ProgramArtifact = serde_json::from_str(&content)?;
        let program = sierra_program.program;

        let sierra_program_registry =
            ProgramRegistry::new(&program).expect("creating program registry failed");

        let debug_info = sierra_program
            .debug_info
            .ok_or_else(|| anyhow!("debug_info must be present in compiled sierra"))?;

        let code_locations = SierraCodeLocations::try_from_debug_info(&debug_info)?;
        let files_data =
            build_file_locations_map(&casm_debug_info.statement_to_pc, &code_locations);

        let functions_debug_info = FunctionsDebugInfo::try_from_debug_info(&debug_info)?;

        // Temporary to get casm debug info until it is returned by USC.
        let casm_debug_info = compile_sierra_to_get_casm_debug_info(&program)?;
        let cairo_var_to_casm =
            build_cairo_var_to_casm_map(&program, &casm_debug_info, functions_debug_info);
        trace!("{:#?}", cairo_var_to_casm);

        let function_names = SierraFunctionNames::try_from_debug_info(&debug_info)?;

        let labels = extract_labels(&program);

        eprintln!("{}", program);

        Ok(Self {
            root_path,
            code_locations,
            function_names,
            casm_debug_info,
            files_data,
            program,
            sierra_program_registry,
            cairo_var_to_casm,
            labels,
        })
    }

    pub fn map_pc_to_code_location(&self, pc: usize) -> Option<CodeLocation> {
        let statement_idx = self.map_pc_to_statement_idx(pc);
        self.code_locations
            .statements_code_locations
            .get(&statement_idx)
            .and_then(|locations| locations.first().cloned())
    }

    pub fn map_pc_to_function_name(&self, pc: usize) -> Option<FunctionName> {
        let statement_idx = self.map_pc_to_statement_idx(pc);
        self.function_names
            .statements_functions
            .get(&statement_idx)
            .and_then(|locations| locations.first().cloned())
    }

    pub fn get_pc_for_line(&self, source: &Path, line: Line) -> Option<usize> {
        let lines_data = &self.files_data.get(source)?.lines;

        if let Some(pc) = lines_data.get(&line) {
            return Some(*pc);
        }

        // Some mappings may be missing, but for now we accept this.
        // If a breakpoint is set on an unmapped line, it will be treated as invalid.
        None
    }

    pub fn is_return_statement(&self, pc: usize) -> bool {
        matches!(self.map_pc_to_statement(pc), Statement::Return(_))
    }

    pub fn is_function_call_statement(&self, pc: usize) -> bool {
        match self.map_pc_to_statement(pc) {
            Statement::Invocation(invocation) => {
                matches!(
                    self.sierra_program_registry.get_libfunc(&invocation.libfunc_id),
                    Ok(CoreConcreteLibfunc::FunctionCall(_))
                )
            }
            Statement::Return(_) => false,
        }
    }

    pub fn print_values_of_variables(&self, pc: usize, vm: &VirtualMachine) {
        let statement_idx = self.map_pc_to_statement_idx(pc);
        let statement = self.map_pc_to_statement(pc);
        let with_labels =
            replace_statement_id(statement.clone(), |idx| self.labels[&idx.0].clone());
        eprintln!("pc={pc} {statement_idx:?}: {with_labels}");

        let Some(variables) = self.cairo_var_to_casm.get(&statement_idx) else {
            return;
        };

        for ((var_name, _), ref_expr) in variables {
            let cells = &ref_expr.cells;
            let mut cells_vals = vec![];
            for cell in cells {
                match cell {
                    CellExpression::Deref(cell_ref) => {
                        let mut relocatable = match cell_ref.register {
                            Register::AP => vm.get_ap(),
                            Register::FP => vm.get_fp(),
                        };
                        let offset_from_register = cell_ref.offset as isize;
                        let register_offset = relocatable.offset as isize;
                        relocatable.offset =
                            (register_offset + offset_from_register).try_into().unwrap();

                        match vm.segments.memory.get_maybe_relocatable(relocatable) {
                            Ok(MaybeRelocatable::Int(value)) => cells_vals.push(value),
                            Ok(MaybeRelocatable::RelocatableValue(relocatable)) => {
                                trace!("UNEXPECTED RELOCATABLE (MAYBE ARRAY): {relocatable:?}")
                            }
                            Err(_) => (),
                        }
                    }
                    CellExpression::DoubleDeref(..) => {
                        trace!("DOUBLE Ds")
                    }
                    CellExpression::Immediate(value) => cells_vals.push(Felt::from(value)),
                    CellExpression::BinOp { .. } => {
                        trace!("BINOP")
                    }
                };
            }
            trace!("{var_name}: {cells_vals:#?}")
        }
    }

    fn map_pc_to_statement(&self, pc: usize) -> &Statement {
        let statement_idx = self.map_pc_to_statement_idx(pc);

        self.program.statements.get(statement_idx.0).expect("statement not found in program")
    }

    fn map_pc_to_statement_idx(&self, pc: usize) -> StatementIdx {
        StatementIdx(
            self.casm_debug_info
                .sierra_statement_info
                .partition_point(|statement_info| statement_info.start_offset <= pc)
                .saturating_sub(1),
        )
    }
}

/// Builds a map to store Sierra statement index and start offset for each file and line.
fn build_file_locations_map(
    statement_to_pc: &[usize],
    code_location_annotations: &SierraCodeLocations,
) -> HashMap<PathBuf, FileCodeLocationsData> {
    // Intermediate storage:
    // Path -> Line -> (min column, pc)
    let mut file_map: HashMap<PathBuf, HashMap<Line, (usize, usize)>> = HashMap::new();

    for (statement_idx, locations) in &code_location_annotations.statements_code_locations {
        let pc = *statement_to_pc.get(statement_idx.0).expect("Invalid Sierra statement index");

        for loc in locations {
            let path_str = &loc.0.0;
            let path = PathBuf::from(path_str);

            let start_location = &loc.1.start;
            let line = Line::new(start_location.line.0);
            let col = start_location.col.0;

            // Get or create the map for this specific file.
            let lines_in_file = file_map.entry(path).or_default();

            // Check if we already have data for this line.
            lines_in_file
                .entry(line)
                .and_modify(|(existing_col, existing_entry)| {
                    // Update the entry if it is at a lower column, or at the same column with a lower PC.
                    // The second condition ensures deterministic behavior - selection of the lowest appropriate pc.
                    if col < *existing_col || (col == *existing_col && pc < *existing_entry) {
                        *existing_col = col;
                        *existing_entry = pc;
                    }
                })
                .or_insert((col, pc));
        }
    }

    // Transform the intermediate map into the final output format,
    // removing the column information as it is no longer necessary.
    file_map
        .into_iter()
        .map(|(path, lines_map)| {
            let clean_lines = lines_map
                .into_iter()
                .map(|(line, (_col, stmt))| (line, stmt))
                .collect::<HashMap<_, _>>();

            (path, FileCodeLocationsData { lines: clean_lines })
        })
        .collect()
}

fn build_cairo_var_to_casm_map(
    program: &Program,
    cairo_program_debug_info: &CairoProgramDebugInfo,
    functions_debug_info: FunctionsDebugInfo,
) -> HashMap<StatementIdx, HashMap<(String, SourceCodeSpan), ReferenceExpression>> {
    let mut cairo_var_to_ref_expr = HashMap::new();
    for (idx, statement_debug_info) in
        cairo_program_debug_info.sierra_statement_info.iter().enumerate()
    {
        let (casm_ref_expressions_for_vars, vars) =
            match (&program.statements[idx], &statement_debug_info.additional_kind_info) {
                (
                    Statement::Invocation(invocation),
                    StatementKindDebugInfo::Invoke(invocation_debug),
                ) => {
                    let casm_ref_expressions_for_vars: Vec<_> =
                        invocation_debug.ref_values.iter().cloned().map(|x| x.expression).collect();
                    (casm_ref_expressions_for_vars, invocation.args.clone())
                }
                (Statement::Return(vars), StatementKindDebugInfo::Return(return_debug)) => {
                    let casm_ref_expressions_for_vars: Vec<_> =
                        return_debug.ref_values.iter().cloned().map(|x| x.expression).collect();
                    (casm_ref_expressions_for_vars, vars.clone())
                }
                _ => unreachable!(),
            };

        assert_eq!(casm_ref_expressions_for_vars.len(), vars.len());
        let function_id =
            &program.funcs[program.funcs.partition_point(|x| x.entry_point.0 <= idx) - 1].id;
        let func_debug_info =
            &functions_debug_info.functions_info[&SierraFunctionId(function_id.id)];

        for (casm_expressions, var_id) in casm_ref_expressions_for_vars.iter().zip(vars.clone()) {
            let Some((name, span)) =
                func_debug_info.sierra_to_cairo_variable.get(&SierraVarId(var_id.id))
            else {
                continue;
            };
            cairo_var_to_ref_expr
                .entry(StatementIdx(idx))
                .or_insert_with(HashMap::new)
                .insert((name.clone(), span.clone()), casm_expressions.clone());
        }
    }

    cairo_var_to_ref_expr
}

fn compile_sierra_to_get_casm_debug_info(program: &Program) -> Result<CairoProgramDebugInfo> {
    let metadata = calc_metadata(program, Default::default())
        .with_context(|| "Failed calculating metadata.")?;
    let cairo_program = cairo_lang_sierra_to_casm::compiler::compile(
        program,
        &metadata,
        SierraToCasmConfig { gas_usage_check: true, max_bytecode_size: usize::MAX },
    )
    .with_context(|| "Compilation failed.")?;

    Ok(cairo_program.debug_info)
}

// TODO(#50)
fn get_project_root_path(sierra_path: &Path) -> Result<PathBuf> {
    Ok(MetadataCommand::new()
        .current_dir(sierra_path.parent().expect("Compiled Sierra must be in target directory"))
        .inherit_stderr()
        .exec()
        .context("Failed to get project metadata from Scarb")?
        .workspace
        .root
        .into())
}

fn extract_labels(program: &Program) -> HashMap<usize, String> {
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

fn replace_statement_id<StatementIdIn, StatementIdOut>(
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
