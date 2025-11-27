use anyhow::Result;
use connection::Connection;
use dap::events::{Event, StoppedEventBody};
use dap::prelude::{Command, Request, ResponseBody};
use dap::responses::{
    EvaluateResponse, ScopesResponse, SetBreakpointsResponse, StackTraceResponse, ThreadsResponse,
    VariablesResponse,
};
use dap::types::{Breakpoint, Capabilities, Source, StackFrame, StoppedEventReason, Thread};
use tracing::trace;

mod connection;

// TODO: add vm, add handlers for requests.
pub struct CairoDebugger {
    connection: Connection,
}

enum ServerResponse {
    Success(ResponseBody),
    Error(String),
    Event(Event),
    SuccessThenEvent(ResponseBody, Event),
}

impl CairoDebugger {
    pub fn connect() -> Result<Self> {
        let connection = Connection::new()?;
        Ok(Self { connection })
    }

    pub fn run(&mut self) -> Result<()> {
        while let Some(req) = self.connection.next_request() {
            match handle_request(&req) {
                ServerResponse::Success(body) => self.connection.send_success(req, body)?,
                ServerResponse::Error(msg) => self.connection.send_error(req, &msg)?,
                ServerResponse::Event(event) => self.connection.send_event(event)?,
                ServerResponse::SuccessThenEvent(body, event) => {
                    self.connection.send_success(req, body)?;
                    self.connection.send_event(event)?;
                }
            }
        }

        Ok(())
    }
}

fn handle_request(request: &Request) -> ServerResponse {
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
            ServerResponse::Error(msg)
        }

        // These may be supported after the MVP.
        // Nonetheless, if we receive these with current capabilities,
        // it is the client's fault.
        Command::ReverseContinue(_) => {
            ServerResponse::Error("Step back is not yet supported".into())
        }
        Command::StepBack(_) => ServerResponse::Error("Step back is not yet supported".into()),
        Command::SetFunctionBreakpoints(_) => {
            ServerResponse::Error("Set function breakpoints is not yet supported".into())
        }

        // It makes no sense to send `attach` in the current architecture.
        Command::Attach(_) => ServerResponse::Error("Attach is not supported".into()),

        // Supported requests.
        Command::Initialize(args) => {
            trace!("Initialized a client: {:?}", args.client_name);

            ServerResponse::Success(ResponseBody::Initialize(Capabilities {
                supports_configuration_done_request: Some(true),
                ..Default::default()
            }))
        }
        Command::ConfigurationDone => {
            trace!("Configuration done");
            ServerResponse::Success(ResponseBody::ConfigurationDone)
        }
        Command::Continue(_) => {
            todo!()
        }
        Command::Launch(_) => {
            // Start running the Cairo program here.
            ServerResponse::SuccessThenEvent(ResponseBody::Launch, Event::Initialized)
        }
        Command::Next(_) => {
            todo!()
        }
        Command::Pause(_) => ServerResponse::Event(Event::Stopped(StoppedEventBody {
            reason: StoppedEventReason::Pause,
            thread_id: Some(0),
            description: None,
            preserve_focus_hint: None,
            text: None,
            all_threads_stopped: Some(true),
            hit_breakpoint_ids: None,
        })),
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
            ServerResponse::Success(ResponseBody::SetBreakpoints(SetBreakpointsResponse {
                breakpoints: response_bps,
            }))
        }
        Command::Source(_) => {
            todo!()
        }
        Command::StackTrace(_) => {
            ServerResponse::Success(ResponseBody::StackTrace(StackTraceResponse {
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
            }))
        }
        Command::StepIn(_) => {
            todo!()
        }
        Command::StepOut(_) => {
            todo!()
        }

        Command::Evaluate(_) => {
            ServerResponse::Success(ResponseBody::Evaluate(EvaluateResponse {
                // Return whatever since we cannot opt out of supporting this request.
                result: "".to_string(),
                type_field: None,
                presentation_hint: None,
                variables_reference: 0,
                named_variables: None,
                indexed_variables: None,
                memory_reference: None,
            }))
        }
        Command::Threads => {
            ServerResponse::Success(ResponseBody::Threads(ThreadsResponse {
                // Return a single thread.
                threads: vec![Thread { id: 0, name: "".to_string() }],
            }))
        }
        Command::Variables(_) => {
            ServerResponse::Success(ResponseBody::Variables(VariablesResponse {
                // Return no variables.
                variables: vec![],
            }))
        }
        Command::Scopes(_) => {
            // Return no scopes.
            ServerResponse::Success(ResponseBody::Scopes(ScopesResponse { scopes: vec![] }))
        }
    }
}
