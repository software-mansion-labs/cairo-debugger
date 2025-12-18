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
    files_data: HashMap<PathBuf, FileCodeLocationsData>,
    casm_debug_info: CasmDebugInfo,
    code_locations: SierraCodeLocations,
}

pub struct FileCodeLocationsData {
    pub lines: BTreeMap<usize, StatementPc>,
}

#[derive(Copy, Clone)]
pub struct StatementPc {
    pub statement_idx: usize,
    pub pc: usize,
}

pub struct CasmDebugInfo {
    /// Sierra statement index -> start CASM bytecode offset
    statement_to_pc: Vec<usize>,
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

    pub fn get_pc_for_line(&self, source: &Path, line: usize) -> Option<usize> {
        let lines_data = &self.files_data.get(source)?.lines;
        // In annotations lines are 0-indexed, but in the source code they are 1-indexed.
        let line = line.saturating_sub(1);

        if let Some(entry) = lines_data.get(&line) {
            return Some(entry.pc);
        }

        // If we did not find a pc for the exact line,
        // we need to find the next line that is greater than the input line.
        // Then we take the pc of the previous Sierra statement.
        if let Some((_next_line, entry)) = lines_data.range(line..).next() {
            let target_idx = entry.statement_idx.saturating_sub(1);
            return self.casm_debug_info.statement_to_pc.get(target_idx).copied();
        }

        None
    }
}

/// Builds a map to store Sierra statement index and start offset for each file and line.
pub fn build_file_locations_map(
    statement_to_pc: &[usize],
    code_location_annotations: &SierraCodeLocations,
) -> HashMap<PathBuf, FileCodeLocationsData> {
    // Intermediate storage:
    // Path -> Line -> (min column, sierra statement index and pc)
    let mut file_map: HashMap<PathBuf, BTreeMap<usize, (usize, StatementPc)>> = HashMap::new();

    for (statement_idx, locations) in &code_location_annotations.statements_code_locations {
        let idx_val = statement_idx.0;
        // Get the PC for the current statement.
        // If the index is out of bounds, we skip it.
        // It should not happen, just a sanity check.
        let pc = match statement_to_pc.get(idx_val) {
            Some(&pc) => pc,
            None => continue,
        };

        let new_entry = StatementPc { statement_idx: idx_val, pc };

        for loc in locations {
            let path_str = &loc.0.0;
            let path = PathBuf::from(path_str);

            let start_location = &loc.1.start;
            let line = start_location.line.0;
            let col = start_location.col.0;

            // Get or create the map for this specific file.
            let lines_in_file = file_map.entry(path).or_default();

            // Check if we already have data for this line.
            lines_in_file
                .entry(line)
                .and_modify(|(existing_col, existing_entry)| {
                    if col < *existing_col {
                        *existing_col = col;
                        *existing_entry = new_entry;
                    }
                })
                .or_insert((col, new_entry));
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
