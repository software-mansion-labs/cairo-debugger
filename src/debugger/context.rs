use std::fs;

use anyhow::{Result, anyhow};
use cairo_annotations::annotations::TryFromDebugInfo;
use cairo_annotations::annotations::coverage::CoverageAnnotationsV1 as SierraCodeLocations;
use cairo_lang_sierra::program::ProgramArtifact;
use camino::Utf8Path;

pub struct Context {
    pub code_locations: SierraCodeLocations,
}

impl Context {
    pub fn new(sierra_path: &Utf8Path) -> Result<Self> {
        let content = fs::read_to_string(sierra_path).expect("Failed to load sierra file");
        let sierra_program: ProgramArtifact = serde_json::from_str(&content)?;
        let debug_info = sierra_program
            .debug_info
            .ok_or_else(|| anyhow!("debug_info must be present in compiled sierra"))?;
        let code_locations = SierraCodeLocations::try_from_debug_info(&debug_info)?;

        Ok(Self { code_locations })
    }
}
