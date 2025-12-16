use std::fs;

use anyhow::{Context as AnyhowContext, Result, anyhow};
use cairo_annotations::annotations::TryFromDebugInfo;
use cairo_annotations::annotations::coverage::{
    CodeLocation, CoverageAnnotationsV1 as SierraCodeLocations,
};
use cairo_lang_sierra::program::{ProgramArtifact, StatementIdx};
use camino::{Utf8Path, Utf8PathBuf};
use scarb_metadata::MetadataCommand;

// Sierra statement index -> start offset
pub type CasmDebugInfo = Vec<usize>;

pub struct Context {
    pub root_path: Utf8PathBuf,
    pub code_locations: SierraCodeLocations,
    pub casm_debug_info: CasmDebugInfo,
}

impl Context {
    pub fn new(sierra_path: &Utf8Path, casm_debug_info: CasmDebugInfo) -> Result<Self> {
        let root_path = get_project_root_path()?;

        let content = fs::read_to_string(sierra_path).expect("Failed to load sierra file");
        let sierra_program: ProgramArtifact = serde_json::from_str(&content)?;
        let debug_info = sierra_program
            .debug_info
            .ok_or_else(|| anyhow!("debug_info must be present in compiled sierra"))?;
        let code_locations = SierraCodeLocations::try_from_debug_info(&debug_info)?;

        Ok(Self { root_path, code_locations, casm_debug_info })
    }

    /// Returns code location for the given PC.
    /// Technically, it should never be `None` if pc and annotations are valid.
    pub fn map_pc_to_code_location(&self, pc: usize) -> Option<CodeLocation> {
        let statement_idx = StatementIdx(
            self.casm_debug_info.partition_point(|&offset| offset <= pc).saturating_sub(1),
        );

        self.code_locations
            .statements_code_locations
            .get(&statement_idx)
            .and_then(|locations| locations.first())
            .cloned()
    }
}

fn get_project_root_path() -> Result<Utf8PathBuf> {
    Ok(MetadataCommand::new()
        .inherit_stderr()
        .exec()
        .context("Failed to get project metadata from Scarb")?
        .workspace
        .root)
}
