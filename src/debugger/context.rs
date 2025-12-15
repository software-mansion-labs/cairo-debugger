use std::fs;

use cairo_annotations::annotations::TryFromDebugInfo;
use cairo_annotations::annotations::coverage::CoverageAnnotationsV1 as SierraCodeLocations;
use cairo_lang_sierra::program::ProgramArtifact;
use camino::Utf8Path;

pub struct Context {
    pub code_locations: SierraCodeLocations,
}

impl Context {
    pub fn new(sierra_path: &Utf8Path) -> Self {
        let content = fs::read_to_string(sierra_path).expect("Failed to load sierra file");
        let sierra_program: ProgramArtifact = serde_json::from_str(&content).unwrap();
        let code_locations = SierraCodeLocations::try_from_debug_info(
            &sierra_program.debug_info.expect("debug_info must be present"),
        )
        .expect("Failed to parse coverage annotations");

        Self { code_locations }
    }
}
