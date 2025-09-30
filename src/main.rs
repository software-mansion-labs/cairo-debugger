use dap::errors::ServerError;

use cairo_debugger::CairoDebugger;

// TODO: there will be no bin target in the future.
fn main() -> Result<(), ServerError> {
    CairoDebugger::default().run()
}
