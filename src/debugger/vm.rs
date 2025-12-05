use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::vm::runners::hook::RunnerPreStepHook;
use cairo_vm::vm::vm_core::VirtualMachine;

use crate::CairoDebugger;

impl RunnerPreStepHook for CairoDebugger {
    #[tracing::instrument(skip(self, vm), err)]
    fn execute(&mut self, vm: &VirtualMachine) -> Result<(), VirtualMachineError> {
        self.sync_with_vm(vm).map_err(VirtualMachineError::Other)
    }
}
