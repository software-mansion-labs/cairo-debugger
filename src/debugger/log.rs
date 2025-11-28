use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::SystemTime;
use std::{env, fs, io};

use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::filter::{EnvFilter, LevelFilter, Targets};
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::fmt::time::Uptime;
use tracing_subscriber::prelude::*;

pub fn init_logging() -> Option<impl Drop> {
    let mut guard = None;

    let fmt_layer = Layer::new()
        .with_writer(io::stderr)
        .with_ansi(io::stderr().is_terminal())
        .with_timer(Uptime::default())
        .with_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .with_env_var("CAIRO_DEBUGGER_LOG")
                .from_env_lossy(),
        );

    let tracing_profile = env::var("CAIRO_DEBUGGER_TRACING_PROFILE").ok().is_some_and(|var| {
        let s = var.as_str();
        s == "true" || s == "1"
    });

    let profile_layer = if tracing_profile {
        let mut path = PathBuf::from(format!(
            "./cairo-debugger-profile-{}.json",
            SystemTime::UNIX_EPOCH.elapsed().unwrap().as_micros()
        ));

        // Create the file now, so that we early panic, and `fs::canonicalize` will work.
        let profile_file = fs::File::create(&path).expect("failed to create profile file");

        // Try to canonicalize the path so that it is easier to find the file from logs.
        if let Ok(canonical) = fs::canonicalize(&path) {
            path = canonical;
        }

        eprintln!("Cairo Debugger run will output tracing profile to: {}", path.display());
        eprintln!(
            "Open that file with https://ui.perfetto.dev (or chrome://tracing) to analyze it"
        );

        let (profile_layer, profile_layer_guard) =
            ChromeLayerBuilder::new().writer(profile_file).include_args(true).build();

        // Filter out less important logs because they're too verbose,
        // and with them the profile file quickly grows to several GBs of data.
        let profile_layer = profile_layer.with_filter(
            Targets::new().with_default(LevelFilter::TRACE).with_target("salsa", LevelFilter::WARN),
        );

        guard = Some(profile_layer_guard);
        Some(profile_layer)
    } else {
        None
    };

    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(fmt_layer).with(profile_layer),
    )
    .expect("could not set up global logger");

    guard
}
