use std::sync::Arc;

use anyhow::bail;
use clap::Parser;
use crossbeam_channel::unbounded;
use planner_app::capabilities::navigator::NavigationOperation;
use planner_app::{Effect, Event};
use tracing::{debug, trace};

use crate::core::Core;
use crate::opts::{build_project_file_path, EventError, Opts};

mod core;
mod opts;

fn main() -> anyhow::Result<()> {
    let args = argfile::expand_args(argfile::parse_fromfile, argfile::PREFIX).unwrap();

    let opts = Opts::parse_from(args);

    cli::tracing::configure_tracing(opts.trace.clone(), opts.verbose.clone())?;

    let project_name = opts.project.clone().unwrap();
    let directory = opts.path.clone();

    let path = build_project_file_path(&project_name, &directory);

    let event: Result<Event, _> = Event::try_from(opts);

    match event {
        Ok(event) => {
            let core = core::new();

            let should_load_first = !matches!(event, Event::CreateProject { .. });
            if should_load_first {
                run_loop(&core, Event::Load {
                    path,
                })?;
            }

            run_loop(&core, event)?;
        }
        // clap configuration prevents this
        Err(EventError::MissingProjectName) => unreachable!(),
    }

    Ok(())
}

fn run_loop(core: &Core, event: Event) -> Result<(), anyhow::Error> {
    let (tx, rx) = unbounded::<Effect>();

    core::update(&core, event, &Arc::new(tx))?;

    while let Ok(effect) = rx.recv() {
        trace!("run_loop. effect: {:?}", effect);
        match effect {
            _render @ Effect::Render(_) => {
                let view = core.view();

                if let Some(error) = view.error {
                    bail!(error)
                }

                // Saving after any operation is implicit for the CLI.
                // FUTURE: Maybe it would be useful to have a 'dry-run' flag that doesn't trigger a save.
                if view.modified {
                    run_loop(core, Event::Save)?
                }
            }
            Effect::Navigator(request) => {
                let operation = request.operation;
                match operation {
                    NavigationOperation::Navigate {
                        path,
                    } => {
                        // Currently, the CLI app cannot navigate anywhere and does not request views.
                        debug!("navigate from run_loop. path: {}", path)
                    }
                }
            }
            Effect::ViewRenderer(_) => {
                // Currently, the CLI app should not cause these effects.
                unreachable!()
            }
        }
    }
    Ok(())
}
