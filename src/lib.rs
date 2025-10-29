use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};

use dap::errors::ServerError;
use dap::prelude::{Command, ResponseBody, Server};
use dap::responses::{EvaluateResponse, ScopesResponse, ThreadsResponse, VariablesResponse};
use dap::types::{Capabilities, Thread};
use tracing::trace;

// TODO: add vm, add handlers for requests.
pub struct CairoDebugger {
    server: Server<TcpStream, TcpStream>,
}

impl CairoDebugger {
    pub fn connect() -> Result<Self, ServerError> {
        let tcp_listener = TcpListener::bind("127.0.0.1:0").map_err(ServerError::IoError)?;
        let os_assigned_port = tcp_listener.local_addr().unwrap().port();
        // Print it so that the client can read it.
        println!("\nDEBUGGER PORT: {os_assigned_port}");

        let (stream, _client_addr) = tcp_listener.accept().map_err(ServerError::IoError)?;
        let input = BufReader::new(stream.try_clone().unwrap());
        let output = BufWriter::new(stream);
        Ok(Self { server: Server::new(input, output) })
    }

    pub fn run(&mut self) -> Result<(), ServerError> {
        while let Some(req) = self.server.poll_request()? {
            let response = match &req.command {
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
                    let msg = format!("Received an unsupported request: {req:?}");
                    req.error(&msg)
                }

                // These may be supported after the MVP.
                // Nonetheless, if we receive these with current capabilities,
                // it is the client's fault.
                Command::ReverseContinue(_) => req.error("Step back is not yet supported"),
                Command::StepBack(_) => req.error("Step back is not yet supported"),
                Command::SetFunctionBreakpoints(_) => {
                    req.error("Set function breakpoints is not yet supported")
                }

                // It makes no sense to send `attach` in the current architecture.
                Command::Attach(_) => req.error("Attach is not supported"),

                // Supported requests.
                Command::Initialize(args) => {
                    trace!("Initialized a client: {:?}", args.client_name);

                    req.success(ResponseBody::Initialize(Capabilities {
                        supports_configuration_done_request: Some(true),
                        ..Default::default()
                    }))
                }
                Command::ConfigurationDone => {
                    trace!("Configuration done");
                    req.success(ResponseBody::ConfigurationDone)
                }
                Command::Continue(_) => {
                    todo!()
                }
                Command::Launch(_) => {
                    // Start running the Cairo program here.
                    req.success(ResponseBody::Launch)
                }
                Command::Next(_) => {
                    todo!()
                }
                Command::Pause(_) => {
                    todo!()
                }
                Command::SetBreakpoints(_) => {
                    todo!()
                }
                Command::Source(_) => {
                    todo!()
                }
                Command::StackTrace(_) => {
                    todo!()
                }
                Command::StepIn(_) => {
                    todo!()
                }
                Command::StepOut(_) => {
                    todo!()
                }

                Command::Evaluate(_) => req.success(ResponseBody::Evaluate(EvaluateResponse {
                    // Return whatever since we cannot opt out of supporting this request.
                    result: "".to_string(),
                    type_field: None,
                    presentation_hint: None,
                    variables_reference: 0,
                    named_variables: None,
                    indexed_variables: None,
                    memory_reference: None,
                })),
                Command::Threads => req.success(ResponseBody::Threads(ThreadsResponse {
                    // Return a single thread.
                    threads: vec![Thread { id: 0, name: "".to_string() }],
                })),
                Command::Variables(_) => req.success(ResponseBody::Variables(VariablesResponse {
                    // Return no variables.
                    variables: vec![],
                })),
                Command::Scopes(_) => {
                    // Return no scopes.
                    req.success(ResponseBody::Scopes(ScopesResponse { scopes: vec![] }))
                }
            };

            self.server.respond(response)?;
        }

        Ok(())
    }
}
