use {
    crate::{
        backend::{Connector, ConnectorEvent, ConnectorId, MonitorInfo},
        ifs::wl_output::WlOutputGlobal,
        rect::Rect,
        state::{ConnectorData, OutputData, State},
        tree::{OutputNode, OutputRenderData},
        utils::{asyncevent::AsyncEvent, clonecell::CloneCell},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub fn handle(state: &Rc<State>, connector: &Rc<dyn Connector>) {
    let id = connector.id();
    let data = Rc::new(ConnectorData {
        connector: connector.clone(),
        handler: Default::default(),
        connected: Cell::new(false),
    });
    let oh = ConnectorHandler {
        id,
        state: state.clone(),
        data: data.clone(),
    };
    let future = state.eng.spawn(oh.handle());
    data.handler.set(Some(future));
    state.connectors.set(id, data);
}

struct ConnectorHandler {
    id: ConnectorId,
    state: Rc<State>,
    data: Rc<ConnectorData>,
}

impl ConnectorHandler {
    async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.data.connector.on_change(Rc::new(move || ae.trigger()));
        }
        if let Some(config) = self.state.config.get() {
            config.new_connector(self.id);
        }
        'outer: loop {
            while let Some(event) = self.data.connector.event() {
                match event {
                    ConnectorEvent::Removed => break 'outer,
                    ConnectorEvent::Connected(mi) => self.handle_connected(&ae, mi).await,
                    _ => unreachable!(),
                }
            }
            ae.triggered().await;
        }
        if let Some(config) = self.state.config.get() {
            config.del_connector(self.id);
        }
        self.data.handler.set(None);
        self.state.connectors.remove(&self.id);
    }

    async fn handle_connected(&self, ae: &Rc<AsyncEvent>, info: MonitorInfo) {
        log::info!("Connector {} connected", self.data.connector.kernel_id());
        self.data.connected.set(true);
        let name = self.state.globals.name();
        let x1 = self
            .state
            .root
            .outputs
            .lock()
            .values()
            .map(|o| o.global.pos.get().x2())
            .max()
            .unwrap_or(0);
        let global = Rc::new(WlOutputGlobal::new(
            name,
            &self.data,
            x1,
            &info.initial_mode,
            &info.manufacturer,
            &info.product,
            info.width_mm,
            info.height_mm,
        ));
        let on = Rc::new(OutputNode {
            id: self.state.node_ids.next(),
            workspaces: Default::default(),
            workspace: CloneCell::new(None),
            seat_state: Default::default(),
            global: global.clone(),
            layers: Default::default(),
            render_data: RefCell::new(OutputRenderData {
                active_workspace: Rect::new_empty(0, 0),
                inactive_workspaces: Default::default(),
                titles: Default::default(),
            }),
            state: self.state.clone(),
            is_dummy: false,
        });
        let mode = info.initial_mode;
        let output_data = Rc::new(OutputData {
            connector: self.data.clone(),
            monitor_info: info,
            node: on.clone(),
        });
        self.state.outputs.set(self.id, output_data);
        if self.state.outputs.len() == 1 {
            let seats = self.state.globals.seats.lock();
            for seat in seats.values() {
                seat.set_position(x1 + mode.width / 2, mode.height / 2);
            }
        }
        global.node.set(Some(on.clone()));
        if let Some(config) = self.state.config.get() {
            config.connector_connected(self.id);
        }
        self.state.root.outputs.set(self.id, on.clone());
        self.state.add_global(&global);
        'outer: loop {
            while let Some(event) = self.data.connector.event() {
                match event {
                    ConnectorEvent::Disconnected => break 'outer,
                    ConnectorEvent::ModeChanged(mode) => {
                        on.update_mode(mode);
                    }
                    _ => unreachable!(),
                }
            }
            ae.triggered().await;
        }
        log::info!("Connector {} disconnected", self.data.connector.kernel_id());
        if let Some(config) = self.state.config.get() {
            config.connector_disconnected(self.id);
        }
        global.node.set(None);
        let _ = self.state.remove_global(&*global);
        self.state.root.outputs.remove(&self.id);
        self.data.connected.set(false);
        self.state.outputs.remove(&self.id);
    }
}