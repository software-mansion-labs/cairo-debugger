use anyhow::{Result, bail};
use camino::Utf8PathBuf;
use dap::events::{Event, StoppedEventBody};
use dap::prelude::{Command, Request, ResponseBody};
use dap::responses::{
    ContinueResponse, EvaluateResponse, ScopesResponse, SetBreakpointsResponse, StackTraceResponse,
    ThreadsResponse, VariablesResponse,
};
use dap::types::{
    Breakpoint, Capabilities, Source, StackFrame, StackFramePresentationhint, StoppedEventReason,
    Thread,
};
use tracing::{error, trace};

use crate::debugger::context::Context;
use crate::debugger::state::State;

pub struct HandlerResponse {
    pub response_body: ResponseBody,
    pub event: Option<Event>,
}

impl From<ResponseBody> for HandlerResponse {
    fn from(response_body: ResponseBody) -> Self {
        Self { response_body, event: None }
    }
}

impl HandlerResponse {
    #[must_use]
    pub fn with_event(mut self, event: Event) -> Self {
        self.event = Some(event);
        self
    }
}

pub fn handle_request(
    request: &Request,
    state: &mut State,
    ctx: &Context,
) -> Result<HandlerResponse> {
    match &request.command {
        // We have not yet decided if we want to support these.
        Command::Attach(_)
        | Command::ReverseContinue(_)
        | Command::StepBack(_)
        | Command::SetFunctionBreakpoints(_)
        | Command::BreakpointLocations(_)
        | Command::Cancel(_)
        | Command::Completions(_)
        | Command::DataBreakpointInfo(_)
        | Command::Disassemble(_)
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
            error!("Received unsupported request: {request:?}");
            bail!("Unsupported request");
        }

        // Initialize flow requests.
        Command::Initialize(args) => {
            trace!("Initialized a client: {:?}", args.client_name);
            Ok(HandlerResponse::from(ResponseBody::Initialize(Capabilities {
                supports_configuration_done_request: Some(true),
                ..Default::default()
            }))
            .with_event(Event::Initialized))
        }
        Command::Launch(_) => Ok(ResponseBody::Launch.into()),
        Command::ConfigurationDone => {
            // Start running the Cairo program here.
            state.set_configuration_done();
            Ok(ResponseBody::ConfigurationDone.into())
        }

        Command::Pause(_) => {
            state.stop_execution();
            Ok(HandlerResponse::from(ResponseBody::Pause).with_event(Event::Stopped(
                StoppedEventBody {
                    reason: StoppedEventReason::Pause,
                    thread_id: Some(0),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: Some(true),
                    hit_breakpoint_ids: None,
                },
            )))
        }
        Command::Continue(_) => {
            state.resume_execution();
            Ok(ResponseBody::Continue(ContinueResponse { all_threads_continued: Some(true) })
                .into())
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
            Ok(ResponseBody::SetBreakpoints(SetBreakpointsResponse { breakpoints: response_bps })
                .into())
        }

        Command::Threads => {
            Ok(ResponseBody::Threads(ThreadsResponse {
                // Return a single thread.
                threads: vec![Thread { id: 0, name: "".to_string() }],
            })
            .into())
        }
        Command::StackTrace(_) => {
            let code_location = ctx.map_pc_to_code_location(state.current_pc);
            let source_path = code_location.as_ref().map(|val| val.0.0.clone()).unwrap();

            let presentation_hint = if Utf8PathBuf::from(source_path).starts_with(&ctx.root_path) {
                StackFramePresentationhint::Normal
            } else {
                StackFramePresentationhint::Subtle
            };
            Ok(ResponseBody::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "test".to_string(),
                    source: Some(Source {
                        name: None,
                        path: code_location.as_ref().map(|val| val.0.0.clone()),
                        ..Default::default()
                    }),
                    line: code_location.as_ref().map(|val| val.1.start.line.0 + 1).unwrap_or(1)
                        as i64,
                    column: code_location.as_ref().map(|val| val.1.start.col.0 + 1).unwrap_or(1)
                        as i64,
                    presentation_hint: Some(presentation_hint),
                    ..Default::default()
                }],
                total_frames: Some(1),
            })
            .into())
        }
        Command::Scopes(_) => {
            // Return no scopes.
            Ok(ResponseBody::Scopes(ScopesResponse { scopes: vec![] }).into())
        }
        Command::Variables(_) => {
            Ok(ResponseBody::Variables(VariablesResponse {
                // Return no variables.
                variables: vec![],
            })
            .into())
        }

        Command::Next(_) => {
            todo!()
        }
        Command::StepIn(_) => {
            todo!()
        }
        Command::StepOut(_) => {
            todo!()
        }
        Command::Source(_) => {
            todo!()
        }

        Command::Evaluate(_) => {
            Ok(ResponseBody::Evaluate(EvaluateResponse {
                // Return whatever since we cannot opt out of supporting this request.
                result: "".to_string(),
                type_field: None,
                presentation_hint: None,
                variables_reference: 0,
                named_variables: None,
                indexed_variables: None,
                memory_reference: None,
            })
            .into())
        }

        Command::Disconnect(_) => {
            todo!()
        }
    }
}
