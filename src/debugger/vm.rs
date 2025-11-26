use crate::CairoDebugger;
use cairo_vm::vm::runners::hook::RunnerPreStepHook;
use cairo_vm::vm::vm_core::VirtualMachine;

impl RunnerPreStepHook for CairoDebugger {
    fn execute(&self, _vm: &VirtualMachine) {
        // TODO: Improve error handling
        self.sync().expect("Debugger failed");
    }
}
