//! Disclaimer: unless stated otherwise, any `pc`-like value in this codebase refers to an offset
//! in the program segment (the first VM segment), not a value of pc in a relocated trace.
//!
//! It is done this way since the debugger is a VM hook -
//! and during VM execution pc is a relocatable, not a value from a relocated trace entry.

mod connection;
mod debugger;

pub use debugger::CairoDebugger;
pub use debugger::context::CasmDebugInfo;
