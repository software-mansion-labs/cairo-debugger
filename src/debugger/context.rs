use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, anyhow};
use cairo_annotations::annotations::TryFromDebugInfo;
use cairo_annotations::annotations::coverage::{
    CodeLocation, CoverageAnnotationsV1 as SierraCodeLocations,
};
use cairo_lang_sierra::program::{ProgramArtifact, StatementIdx};
use scarb_metadata::MetadataCommand;

/// Struct that holds all the initial data needed for the debugger during execution.
pub struct Context {
    pub root_path: PathBuf,
    casm_debug_info: CasmDebugInfo,
    code_locations: SierraCodeLocations,
    files_data: HashMap<PathBuf, FileCodeLocationsData>,
}

pub struct CasmDebugInfo {
    /// Sierra statement index -> start CASM bytecode offset
    pub statement_to_pc: Vec<usize>,
}

struct FileCodeLocationsData {
    /// Line number -> start CASM bytecode offset
    lines: BTreeMap<Line, usize>,
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
        let debug_info = sierra_program
            .debug_info
            .ok_or_else(|| anyhow!("debug_info must be present in compiled sierra"))?;
        let code_locations = SierraCodeLocations::try_from_debug_info(&debug_info)?;
        let files_data =
            build_file_locations_map(&casm_debug_info.statement_to_pc, &code_locations);

        Ok(Self { root_path, code_locations, casm_debug_info, files_data })
    }

    pub fn map_pc_to_code_location(&self, pc: usize) -> Option<CodeLocation> {
        let statement_idx = StatementIdx(
            self.casm_debug_info
                .statement_to_pc
                .partition_point(|&offset| offset <= pc)
                .saturating_sub(1),
        );

        self.code_locations
            .statements_code_locations
            .get(&statement_idx)
            .and_then(|locations| locations.first())
            .cloned()
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
}

/// Builds a map to store Sierra statement index and start offset for each file and line.
fn build_file_locations_map(
    statement_to_pc: &[usize],
    code_location_annotations: &SierraCodeLocations,
) -> HashMap<PathBuf, FileCodeLocationsData> {
    // Intermediate storage:
    // Path -> Line -> (min column, sierra statement index and pc)
    let mut file_map: HashMap<PathBuf, BTreeMap<Line, (usize, usize)>> = HashMap::new();

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
                    // The second condition ensures deterministic behavior.
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
                .collect::<BTreeMap<_, _>>();

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
