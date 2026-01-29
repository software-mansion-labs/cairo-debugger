use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, anyhow};
use cairo_annotations::annotations::TryFromDebugInfo;
use cairo_annotations::annotations::coverage::{
    CodeLocation, CoverageAnnotationsV1 as SierraCodeLocations,
};
use cairo_annotations::annotations::profiler::{
    FunctionName, ProfilerAnnotationsV1 as SierraFunctionNames,
};
use cairo_lang_sierra::extensions::core::{CoreConcreteLibfunc, CoreLibfunc, CoreType};
use cairo_lang_sierra::program::{Program, ProgramArtifact, Statement, StatementIdx};
use cairo_lang_sierra::program_registry::ProgramRegistry;
use scarb_metadata::MetadataCommand;

/// Struct that holds all the initial data needed for the debugger during execution.
pub struct Context {
    pub root_path: PathBuf,
    casm_debug_info: CasmDebugInfo,
    code_locations: SierraCodeLocations,
    function_names: SierraFunctionNames,
    files_data: HashMap<PathBuf, FileCodeLocationsData>,
    program: Program,
    sierra_program_registry: ProgramRegistry<CoreType, CoreLibfunc>,
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
        let function_names = SierraFunctionNames::try_from_debug_info(&debug_info)?;
        let files_data =
            build_file_locations_map(&casm_debug_info.statement_to_pc, &code_locations);

        Ok(Self {
            root_path,
            code_locations,
            function_names,
            casm_debug_info,
            files_data,
            program,
            sierra_program_registry,
        })
    }

    pub fn statement_idx_for_pc(&self, pc: usize) -> StatementIdx {
        StatementIdx(
            self.casm_debug_info
                .statement_to_pc
                .partition_point(|&offset| offset <= pc)
                .saturating_sub(1),
        )
    }

    pub fn code_location_for_statement_idx(
        &self,
        statement_idx: StatementIdx,
    ) -> Option<CodeLocation> {
        self.code_locations
            .statements_code_locations
            .get(&statement_idx)
            .and_then(|locations| locations.first().cloned())
    }

    pub fn function_name_for_statement_idx(
        &self,
        statement_idx: StatementIdx,
    ) -> Option<FunctionName> {
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

        // If a breakpoint is set on an unmapped line, it will be treated as invalid.
        None
    }

    pub fn is_return_statement(&self, statement_idx: StatementIdx) -> bool {
        matches!(self.statement_idx_to_statement(statement_idx), Statement::Return(_))
    }

    pub fn is_function_call_statement(&self, statement_idx: StatementIdx) -> bool {
        match self.statement_idx_to_statement(statement_idx) {
            Statement::Invocation(invocation) => {
                matches!(
                    self.sierra_program_registry.get_libfunc(&invocation.libfunc_id),
                    Ok(CoreConcreteLibfunc::FunctionCall(_))
                )
            }
            Statement::Return(_) => false,
        }
    }

    fn statement_idx_to_statement(&self, statement_idx: StatementIdx) -> &Statement {
        &self.program.statements[statement_idx.0]
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

    for (StatementIdx(idx), locations) in &code_location_annotations.statements_code_locations {
        let pc = *statement_to_pc.get(*idx).expect("Invalid Sierra statement index");
        // If the next sierra statement maps to the same pc, it means the compilation of the current
        // statement did not produce any CASM instructions.
        //
        // We should not take such statements into account when creating a line -> pc map, since
        // there is no actual pc that corresponds to a line which corresponds to such a statement.
        //
        // An example:
        // ```
        // fn main() -> felt252 {
        //   let x = 5;
        //   let y = @x; // <- The Line
        //   x + 5
        // }
        // The Line compiles to (with optimizations turned off during Cairo->Sierra compilation)
        // to a statement `snapshot_take<felt252>([0]) -> ([1], [2]);. This libfunc takes
        // a sierra variable of id 0 and returns its original value and its duplicate, which are
        // now "in" sierra vars of id 1 and 2.
        // Even though the statement maps to some Cairo code in coverage mappings,
        // it does not compile to any CASM instructions directly - check the link below.
        // https://github.com/starkware-libs/cairo/blob/27f9d1a3fcd00993ff43016ce9579e36064e5266/crates/cairo-lang-sierra-to-casm/src/invocations/mod.rs#L718
        // TODO(#61): compare `start_offset` and `end_offset` of current statement instead once USC
        //  (and thus snforge) starts providing full `CairoProgramDebugInfo`.
        if statement_to_pc
            .get(idx + 1)
            .is_some_and(|pc_of_next_statement| *pc_of_next_statement == pc)
        {
            continue;
        };

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
