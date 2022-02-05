use crate::client::{Client, DynEventFormatter};
use crate::globals::{Global, GlobalName};
use crate::ifs::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration;
use crate::object::{Interface, Object};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

mod types;

const CREATE: u32 = 0;

const DEFAULT_MODE: u32 = 0;

#[allow(dead_code)]
const NONE: u32 = 0;
#[allow(dead_code)]
const CLIENT: u32 = 1;
const SERVER: u32 = 2;

id!(OrgKdeKwinServerDecorationManagerGlobalId);

pub struct OrgKdeKwinServerDecorationManagerGlobal {
    name: GlobalName,
}
impl OrgKdeKwinServerDecorationManagerGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: OrgKdeKwinServerDecorationManagerGlobalId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), OrgKdeKwinServerDecorationManagerError> {
        let obj = Rc::new(OrgKdeKwinServerDecorationManager {
            id,
            client: client.clone(),
            _version: version,
        });
        client.add_client_obj(&obj)?;
        client.event(obj.default_mode(SERVER));
        Ok(())
    }
}

bind!(OrgKdeKwinServerDecorationManagerGlobal);

impl Global for OrgKdeKwinServerDecorationManagerGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn singleton(&self) -> bool {
        true
    }

    fn interface(&self) -> Interface {
        Interface::OrgKdeKwinServerDecorationManager
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(OrgKdeKwinServerDecorationManagerGlobal);

pub struct OrgKdeKwinServerDecorationManager {
    id: OrgKdeKwinServerDecorationManagerGlobalId,
    client: Rc<Client>,
    _version: u32,
}

impl OrgKdeKwinServerDecorationManager {
    fn default_mode(self: &Rc<Self>, mode: u32) -> DynEventFormatter {
        Box::new(DefaultMode {
            obj: self.clone(),
            mode,
        })
    }

    fn create(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateError> {
        let req: Create = self.client.parse(self, parser)?;
        let _ = self.client.lookup(req.surface)?;
        let obj = Rc::new(OrgKdeKwinServerDecoration::new(req.id, &self.client));
        self.client.add_client_obj(&obj)?;
        self.client.event(obj.mode(SERVER));
        Ok(())
    }
}

object_base! {
    OrgKdeKwinServerDecorationManager, OrgKdeKwinServerDecorationManagerError;

    CREATE => create,
}

impl Object for OrgKdeKwinServerDecorationManager {
    fn num_requests(&self) -> u32 {
        CREATE + 1
    }
}

simple_add_obj!(OrgKdeKwinServerDecorationManager);