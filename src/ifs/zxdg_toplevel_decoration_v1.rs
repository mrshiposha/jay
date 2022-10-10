use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::xdg_surface::xdg_toplevel::{Decoration, XdgToplevel},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zxdg_toplevel_decoration_v1::*, ZxdgToplevelDecorationV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

const CLIENT_SIDE: u32 = 1;
const SERVER_SIDE: u32 = 2;

pub struct ZxdgToplevelDecorationV1 {
    pub id: ZxdgToplevelDecorationV1Id,
    pub client: Rc<Client>,
    pub toplevel: Rc<XdgToplevel>,
    pub tracker: Tracker<Self>,
}

impl ZxdgToplevelDecorationV1 {
    pub fn new(
        id: ZxdgToplevelDecorationV1Id,
        client: &Rc<Client>,
        toplevel: &Rc<XdgToplevel>,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            toplevel: toplevel.clone(),
            tracker: Default::default(),
        }
    }

    fn send_configure(&self, mode: u32) {
        self.client.event(Configure {
            self_id: self.id,
            mode,
        })
    }

    pub fn do_send_configure(&self) {
        let mode = match self.toplevel.decoration.get() {
            Decoration::Client => CLIENT_SIDE,
            Decoration::Server => SERVER_SIDE,
        };
        self.send_configure(mode);
        self.toplevel.send_current_configure();
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZxdgToplevelDecorationV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_mode(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZxdgToplevelDecorationV1Error> {
        let _req: SetMode = self.client.parse(&**self, parser)?;
        self.do_send_configure();
        Ok(())
    }

    fn unset_mode(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZxdgToplevelDecorationV1Error> {
        let _req: UnsetMode = self.client.parse(&**self, parser)?;
        self.do_send_configure();
        Ok(())
    }
}

object_base! {
    ZxdgToplevelDecorationV1;

    DESTROY => destroy,
    SET_MODE => set_mode,
    UNSET_MODE => unset_mode,
}

impl Object for ZxdgToplevelDecorationV1 {
    fn num_requests(&self) -> u32 {
        UNSET_MODE + 1
    }
}

simple_add_obj!(ZxdgToplevelDecorationV1);

#[derive(Debug, Error)]
pub enum ZxdgToplevelDecorationV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZxdgToplevelDecorationV1Error, ClientError);
efrom!(ZxdgToplevelDecorationV1Error, MsgParserError);
