use cairo_debugger::CairoDebugger;
use dap::errors::ServerError;

// TODO: there will be no bin target in the future.
fn main() -> Result<(), ServerError> {
    CairoDebugger::connect()?.run()
}
