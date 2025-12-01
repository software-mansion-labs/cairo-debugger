use cairo_vm::vm::runners::hook::RunnerPreStepHook;
use cairo_vm::vm::vm_core::VirtualMachine;

use crate::CairoDebugger;

impl RunnerPreStepHook for CairoDebugger {
    fn execute(&self, vm: &VirtualMachine) {
        // TODO: Improve error handling
        self.sync_with_vm(vm).expect("Debugger failed");
    }
}
