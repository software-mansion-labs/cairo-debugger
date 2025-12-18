use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, anyhow};
use cairo_annotations::annotations::TryFromDebugInfo;
use cairo_annotations::annotations::coverage::{
    CodeLocation, CoverageAnnotationsV1 as SierraCodeLocations,
};
use cairo_lang_sierra::program::{ProgramArtifact, StatementIdx};
use scarb_metadata::MetadataCommand;

pub struct Context {
    pub root_path: PathBuf,
    pub code_locations: SierraCodeLocations,
    pub casm_debug_info: CasmDebugInfo,
}

pub struct CasmDebugInfo {
    // Sierra statement index -> start offset
    statement_to_pc: Vec<usize>,
}

impl Context {
    pub fn new(sierra_path: &Path, casm_debug_info: CasmDebugInfo) -> Result<Self> {
        let root_path = get_project_root_path()?;

        let content = fs::read_to_string(sierra_path).expect("Failed to load sierra file");
        let sierra_program: ProgramArtifact = serde_json::from_str(&content)?;
        let debug_info = sierra_program
            .debug_info
            .ok_or_else(|| anyhow!("debug_info must be present in compiled sierra"))?;
        let code_locations = SierraCodeLocations::try_from_debug_info(&debug_info)?;

        Ok(Self { root_path, code_locations, casm_debug_info })
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
}

fn get_project_root_path() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    Ok(MetadataCommand::new()
        .current_dir(current_dir)
        .inherit_stderr()
        .exec()
        .context("Failed to get project metadata from Scarb")?
        .workspace
        .root
        .into())
}
