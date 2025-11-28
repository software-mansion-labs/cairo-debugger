use anyhow::{Result, bail};
use dap::events::{Event, StoppedEventBody};
use dap::prelude::{Command, Request, ResponseBody};
use dap::responses::{
    EvaluateResponse, ScopesResponse, SetBreakpointsResponse, StackTraceResponse, ThreadsResponse,
    VariablesResponse,
};
use dap::types::{Breakpoint, Capabilities, Source, StackFrame, StoppedEventReason, Thread};
use tracing::trace;

use crate::CairoDebugger;

pub enum HandleResult {
    Handled,
}

impl CairoDebugger {
    pub(crate) fn handle_request(&self, request: Request) -> Result<HandleResult> {
        match &request.command {
            // We have not yet decided if we want to support these.
            Command::BreakpointLocations(_)
            | Command::Cancel(_)
            | Command::Completions(_)
            | Command::DataBreakpointInfo(_)
            | Command::Disassemble(_)
            | Command::Disconnect(_)
            | Command::Goto(_)
            | Command::ExceptionInfo(_)
            | Command::GotoTargets(_)
            | Command::LoadedSources
            | Command::Modules(_)
            | Command::ReadMemory(_)
            | Command::RestartFrame(_)
            | Command::SetDataBreakpoints(_)
            | Command::Restart(_)
            | Command::SetExceptionBreakpoints(_)
            | Command::TerminateThreads(_)
            | Command::Terminate(_)
            | Command::StepInTargets(_)
            | Command::SetVariable(_)
            | Command::SetInstructionBreakpoints(_)
            | Command::SetExpression(_)
            | Command::WriteMemory(_) => {
                // If we receive these with current capabilities, it is the client's fault.
                let msg = format!("Received an unsupported request: {request:?}");
                self.connection.send_error(request, &msg)?;
                bail!("Unsupported request");
            }

            // These may be supported after the MVP.
            // Nonetheless, if we receive these with current capabilities,
            // it is the client's fault.
            Command::ReverseContinue(_) => {
                self.connection.send_error(request, "Reverse continue is not yet supported")?;
                bail!("Reverse continue is not yet supported");
            }
            Command::StepBack(_) => {
                self.connection.send_error(request, "Step back is not yet supported")?;
                bail!("Step back is not yet supported");
            }
            Command::SetFunctionBreakpoints(_) => {
                self.connection
                    .send_error(request, "Set function breakpoints is not yet supported")?;
                bail!("Set function breakpoints is not yet supported");
            }

            // It makes no sense to send `attach` in the current architecture.
            Command::Attach(_) => {
                self.connection.send_error(request, "Attach is not supported")?;
                bail!("Unsupported request");
            }

            // Supported requests.
            Command::Initialize(args) => {
                trace!("Initialized a client: {:?}", args.client_name);
                self.connection.send_success(
                    request,
                    ResponseBody::Initialize(Capabilities {
                        supports_configuration_done_request: Some(true),
                        ..Default::default()
                    }),
                )?;
                self.connection.send_event(Event::Initialized)?;
                Ok(HandleResult::Handled)
            }
            Command::ConfigurationDone => {
                trace!("Configuration done");
                self.connection.send_success(request, ResponseBody::ConfigurationDone)?;
                Ok(HandleResult::Handled)
            }
            Command::Continue(_) => {
                todo!()
            }
            Command::Launch(_) => {
                // Start running the Cairo program here.
                self.connection.send_success(request, ResponseBody::Launch)?;
                Ok(HandleResult::Handled)
            }
            Command::Next(_) => {
                todo!()
            }
            Command::Pause(_) => {
                self.connection.send_event(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Pause,
                    thread_id: Some(0),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: Some(true),
                    hit_breakpoint_ids: None,
                }))?;
                self.connection.send_success(request, ResponseBody::Pause)?;
                Ok(HandleResult::Handled)
            }
            Command::SetBreakpoints(args) => {
                let mut response_bps = Vec::new();
                if let Some(requested_bps) = &args.breakpoints {
                    for bp in requested_bps {
                        // For now accept every breakpoint as valid
                        response_bps.push(Breakpoint {
                            verified: true,
                            source: Some(args.source.clone()),
                            line: Some(bp.line),
                            ..Default::default()
                        });
                    }
                }
                self.connection.send_success(
                    request,
                    ResponseBody::SetBreakpoints(SetBreakpointsResponse {
                        breakpoints: response_bps,
                    }),
                )?;
                Ok(HandleResult::Handled)
            }
            Command::Source(_) => {
                todo!()
            }
            Command::StackTrace(_) => {
                self.connection.send_success(
                    request,
                    ResponseBody::StackTrace(StackTraceResponse {
                        stack_frames: vec![StackFrame {
                            id: 1,
                            name: "test".to_string(),
                            // Replace it with the actual source path.
                            // Otherwise, the debugger will crush after returning this response.
                            source: Some(Source { name: None, path: None, ..Default::default() }),
                            line: 1,
                            column: 1,
                            ..Default::default()
                        }],
                        total_frames: Some(1),
                    }),
                )?;
                Ok(HandleResult::Handled)
            }
            Command::StepIn(_) => {
                todo!()
            }
            Command::StepOut(_) => {
                todo!()
            }

            Command::Evaluate(_) => {
                self.connection.send_success(
                    request,
                    ResponseBody::Evaluate(EvaluateResponse {
                        // Return whatever since we cannot opt out of supporting this request.
                        result: "".to_string(),
                        type_field: None,
                        presentation_hint: None,
                        variables_reference: 0,
                        named_variables: None,
                        indexed_variables: None,
                        memory_reference: None,
                    }),
                )?;
                Ok(HandleResult::Handled)
            }
            Command::Threads => {
                self.connection.send_success(
                    request,
                    ResponseBody::Threads(ThreadsResponse {
                        // Return a single thread.
                        threads: vec![Thread { id: 0, name: "".to_string() }],
                    }),
                )?;
                Ok(HandleResult::Handled)
            }
            Command::Variables(_) => {
                self.connection.send_success(
                    request,
                    ResponseBody::Variables(VariablesResponse {
                        // Return no variables.
                        variables: vec![],
                    }),
                )?;
                Ok(HandleResult::Handled)
            }
            Command::Scopes(_) => {
                // Return no scopes.
                // Return no scopes.
                self.connection.send_success(
                    request,
                    ResponseBody::Scopes(ScopesResponse { scopes: vec![] }),
                )?;
                Ok(HandleResult::Handled)
            }
        }
    }
}
