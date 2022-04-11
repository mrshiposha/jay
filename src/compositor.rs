use uapi::c;
use {
    crate::{
        acceptor::{Acceptor, AcceptorError},
        async_engine::{AsyncEngine, AsyncError, Phase},
        backend,
        backends::dummy::DummyOutput,
        cli::{GlobalArgs, RunArgs},
        client::Clients,
        clientmem::{self, ClientMemError},
        config::ConfigProxy,
        dbus::Dbus,
        event_loop::{EventLoop, EventLoopError},
        forker,
        globals::Globals,
        ifs::{wl_output::WlOutputGlobal, wl_surface::NoneSurfaceExt},
        leaks,
        logger::Logger,
        render::{self, RenderError},
        sighand::{self, SighandError},
        state::{ConnectorData, State},
        tasks,
        tree::{
            container_layout, container_render_data, float_layout, float_titles, DisplayNode,
            NodeIds, OutputNode, WorkspaceNode,
        },
        utils::{
            clonecell::CloneCell, errorfmt::ErrorFmt, fdcloser::FdCloser, queue::AsyncQueue,
            run_toplevel::RunToplevel,
        },
        wheel::{Wheel, WheelError},
        xkbcommon::XkbContext,
        xwayland,
    },
    forker::ForkerProxy,
    std::{cell::Cell, ops::Deref, rc::Rc, sync::Arc},
    thiserror::Error,
};
use crate::utils::oserror::OsError;
use crate::utils::tri::Try;

pub const MAX_EXTENTS: i32 = (1 << 24) - 1;

pub fn start_compositor(global: GlobalArgs, args: RunArgs) {
    let forker = match ForkerProxy::create() {
        Ok(f) => Rc::new(f),
        Err(e) => fatal!("Could not create a forker process: {}", ErrorFmt(e)),
    };
    let logger = Logger::install_compositor(global.log_level.into());
    if let Err(e) = main_(forker, logger.clone(), &args) {
        let e = ErrorFmt(e);
        log::error!("A fatal error occurred: {}", e);
        eprintln!("A fatal error occurred: {}", e);
        eprintln!("See {} for more details.", logger.path());
        std::process::exit(1);
    }
    log::info!("Exit");
}

#[derive(Debug, Error)]
enum MainError {
    #[error("The client acceptor caused an error")]
    AcceptorError(#[from] AcceptorError),
    #[error("The event loop caused an error")]
    EventLoopError(#[from] EventLoopError),
    #[error("The signal handler caused an error")]
    SighandError(#[from] SighandError),
    #[error("The clientmem subsystem caused an error")]
    ClientmemError(#[from] ClientMemError),
    #[error("The timer subsystem caused an error")]
    WheelError(#[from] WheelError),
    #[error("The async subsystem caused an error")]
    AsyncError(#[from] AsyncError),
    #[error("The render backend caused an error")]
    RenderError(#[from] RenderError),
}

fn main_(forker: Rc<ForkerProxy>, logger: Arc<Logger>, _args: &RunArgs) -> Result<(), MainError> {
    init_fd_limit();
    leaks::init();
    render::init()?;
    clientmem::init()?;
    let el = EventLoop::new()?;
    sighand::install(&el)?;
    let xkb_ctx = XkbContext::new().unwrap();
    let xkb_keymap = xkb_ctx.keymap_from_str(include_str!("keymap.xkb")).unwrap();
    let wheel = Wheel::install(&el)?;
    let engine = AsyncEngine::install(&el, &wheel)?;
    let (_run_toplevel_future, run_toplevel) = RunToplevel::install(&engine);
    let node_ids = NodeIds::default();
    let state = Rc::new(State {
        xkb_ctx,
        backend: Default::default(),
        forker: Default::default(),
        default_keymap: xkb_keymap,
        eng: engine.clone(),
        el: el.clone(),
        render_ctx: Default::default(),
        cursors: Default::default(),
        wheel,
        clients: Clients::new(),
        globals: Globals::new(),
        connector_ids: Default::default(),
        root: Rc::new(DisplayNode::new(node_ids.next())),
        workspaces: Default::default(),
        dummy_output: Default::default(),
        node_ids,
        backend_events: AsyncQueue::new(),
        seat_ids: Default::default(),
        seat_queue: Default::default(),
        slow_clients: AsyncQueue::new(),
        none_surface_ext: Rc::new(NoneSurfaceExt),
        tree_changed_sent: Cell::new(false),
        config: Default::default(),
        input_device_ids: Default::default(),
        input_device_handlers: Default::default(),
        theme: Default::default(),
        pending_container_layout: Default::default(),
        pending_container_render_data: Default::default(),
        pending_float_layout: Default::default(),
        pending_float_titles: Default::default(),
        dbus: Dbus::new(&engine, &run_toplevel),
        fdcloser: FdCloser::new(),
        logger,
        connectors: Default::default(),
        outputs: Default::default(),
        status: Default::default(),
    });
    {
        let dummy_output = Rc::new(OutputNode {
            id: state.node_ids.next(),
            global: Rc::new(WlOutputGlobal::new(
                state.globals.name(),
                &Rc::new(ConnectorData {
                    connector: Rc::new(DummyOutput {
                        id: state.connector_ids.next(),
                    }),
                    handler: Cell::new(None),
                    connected: Cell::new(true),
                    name: "Dummy".to_string(),
                }),
                0,
                &backend::Mode {
                    width: 0,
                    height: 0,
                    refresh_rate_millihz: 0,
                },
                "none",
                "none",
                0,
                0,
            )),
            workspaces: Default::default(),
            workspace: Default::default(),
            seat_state: Default::default(),
            layers: Default::default(),
            render_data: Default::default(),
            state: state.clone(),
            is_dummy: true,
            status: Default::default(),
        });
        let dummy_workspace = Rc::new(WorkspaceNode {
            id: state.node_ids.next(),
            output: CloneCell::new(dummy_output.clone()),
            position: Default::default(),
            container: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
            name: "dummy".to_string(),
            output_link: Default::default(),
            visible: Cell::new(false),
        });
        dummy_workspace.output_link.set(Some(
            dummy_output.workspaces.add_last(dummy_workspace.clone()),
        ));
        dummy_output.show_workspace(&dummy_workspace);
        state.dummy_output.set(Some(dummy_output));
    }
    forker.install(&state);
    let config = ConfigProxy::default(&state);
    state.config.set(Some(Rc::new(config)));
    let _global_event_handler = engine.spawn(tasks::handle_backend_events(state.clone()));
    let _slow_client_handler = engine.spawn(tasks::handle_slow_clients(state.clone()));
    let _container_do_layout = engine.spawn2(Phase::Layout, container_layout(state.clone()));
    let _container_render_titles =
        engine.spawn2(Phase::PostLayout, container_render_data(state.clone()));
    let _float_do_layout = engine.spawn2(Phase::Layout, float_layout(state.clone()));
    let _float_render_titles = engine.spawn2(Phase::PostLayout, float_titles(state.clone()));
    let socket_path = Acceptor::install(&state)?;
    forker.setenv(b"WAYLAND_DISPLAY", socket_path.as_bytes());
    forker.setenv(b"_JAVA_AWT_WM_NONREPARENTING", b"1");
    let _xwayland = engine.spawn(xwayland::manage(state.clone()));
    let _backend = engine.spawn(tasks::start_backend(state.clone()));
    el.run()?;
    drop(_xwayland);
    state.clients.clear();
    for (_, seat) in state.globals.seats.lock().deref() {
        seat.clear();
    }
    leaks::log_leaked();
    Ok(())
}

fn init_fd_limit() {
    let res = OsError::tri(|| {
        let mut cur = uapi::getrlimit(c::RLIMIT_NOFILE as _)?;
        if cur.rlim_cur < cur.rlim_max {
            log::info!("Increasing file descriptor limit from {} to {}", cur.rlim_cur, cur.rlim_max);
            cur.rlim_cur = cur.rlim_max;
            uapi::setrlimit(c::RLIMIT_NOFILE as _, &cur)?;
        }
        Ok(())
    });
    if let Err(e) = res {
        log::warn!("Could not increase file descriptor limit: {}", ErrorFmt(e));
    }
}
