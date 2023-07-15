use std::{fs::File, os::fd::FromRawFd, io::Write};

use uapi::{memfd_create, c::{MFD_CLOEXEC, MFD_ALLOW_SEALING, F_SEAL_GROW, F_SEAL_SHRINK, F_SEAL_WRITE}, IntoUstr, fcntl_add_seals};

use crate::{utils::buffd::BufFdError, io_uring::IoUringError, wire::{zwp_linux_dmabuf_feedback_v1::{FormatTable, MainDevice, TrancheTargetDevice, TrancheFormats, TrancheFlags, TrancheDone, Done}, ZwpLinuxDmabufFeedbackV1Id}};

use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_linux_dmabuf_v1::*, ZwpLinuxDmabufV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpLinuxDmabufV1Global {
    name: GlobalName,
}

impl ZwpLinuxDmabufV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpLinuxDmabufV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let obj = Rc::new(ZwpLinuxDmabufV1 {
            id,
            client: client.clone(),
            _version: version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;

        if version >= FEEDBACK_SINCE_VERSION {
            log::info!("version >= FEEDBACK_SINCE_VERSION, using v4 feedback");
            return Ok(())
        }

        if let Some(ctx) = client.state.render_ctx.get() {
            let formats = ctx.formats();
            for format in formats.values() {
                if format.implicit_external_only && !ctx.supports_external_texture() {
                    continue;
                }
                obj.send_format(format.format.drm);
                if version >= MODIFIERS_SINCE_VERSION {
                    for modifier in format.modifiers.values() {
                        if modifier.external_only && !ctx.supports_external_texture() {
                            continue;
                        }
                        obj.send_modifier(format.format.drm, modifier.modifier);
                    }
                }
            }
        }
        Ok(())
    }
}

const MODIFIERS_SINCE_VERSION: u32 = 3;
const FEEDBACK_SINCE_VERSION: u32 = 4;

global_base!(
    ZwpLinuxDmabufV1Global,
    ZwpLinuxDmabufV1,
    ZwpLinuxDmabufV1Error
);

impl Global for ZwpLinuxDmabufV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        4
    }
}

simple_add_global!(ZwpLinuxDmabufV1Global);

pub struct ZwpLinuxDmabufV1 {
    id: ZwpLinuxDmabufV1Id,
    pub client: Rc<Client>,
    _version: u32,
    pub tracker: Tracker<Self>,
}

pub struct ZwpLinuxDmabufFeedbackV1 {
    id: ZwpLinuxDmabufFeedbackV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl ZwpLinuxDmabufV1 {
    fn send_format(&self, format: u32) {
        self.client.event(Format {
            self_id: self.id,
            format,
        })
    }

    fn send_modifier(&self, format: u32, modifier: u64) {
        self.client.event(Modifier {
            self_id: self.id,
            format,
            modifier_hi: (modifier >> 32) as _,
            modifier_lo: modifier as _,
        })
    }

    fn destroy(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), ZwpLinuxDmabufV1Error> {
        let _req: Destroy = self.client.parse(&**self, parser)?;
        self.client.remove_obj(&**self)?;
        Ok(())
    }

    fn create_params(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let req: CreateParams = self.client.parse(&**self, parser)?;
        let params = Rc::new(ZwpLinuxBufferParamsV1::new(req.params_id, self));
        track!(self.client, params);
        self.client.add_client_obj(&params)?;
        Ok(())
    }

    fn get_default_feedback(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let req: GetDefaultFeedback = self.client.parse(&**self, parser)?;
        self.send_feedback(req.id)
    }

    fn get_surface_feedback(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let req: GetSurfaceFeedback = self.client.parse(&**self, parser)?;
        self.send_feedback(req.id)
    }

    fn send_feedback(self: &Rc<Self>, feedback_id: ZwpLinuxDmabufFeedbackV1Id) -> Result<(), ZwpLinuxDmabufV1Error> {
        if let Some(ctx) = self.client.state.render_ctx.get() {
            let fd = memfd_create(b"dmabuf_feedback\0".into_ustr(), MFD_CLOEXEC | MFD_ALLOW_SEALING)
            .map_err(|err| {
                log::info!("err = {err:?}");
                ClientError::Io(BufFdError::Io(IoUringError::OsError(err.into())))
            })?;

            let mut file = unsafe {
                File::from_raw_fd(*fd)
            };

            let mut size: u16 = 0;

            let formats = ctx.formats();
            for format in formats.values() {
                if format.implicit_external_only && !ctx.supports_external_texture() {
                    continue;
                }

                for modifier in format.modifiers.values() {
                    if modifier.external_only && !ctx.supports_external_texture() {
                        continue;
                    }

                    let format = format.format.drm;
                    let modifier = modifier.modifier;

                    file.write(&format.to_le_bytes())
                        .map_err(|_| ClientError::InvalidMethod)?;

                    file.write(&[0, 0, 0, 0])
                        .map_err(|_| ClientError::InvalidMethod)?;

                    file.write(&modifier.to_le_bytes())
                        .map_err(|_| ClientError::InvalidMethod)?;

                    size += 1;
                }
            }

            
            fcntl_add_seals(*fd, F_SEAL_GROW | F_SEAL_SHRINK | F_SEAL_WRITE)
                .map_err(|_| ClientError::InvalidMethod)?;

            std::mem::forget(file);

            let feedback = Rc::new(ZwpLinuxDmabufFeedbackV1 {
                id: feedback_id,
                client: self.client.clone(),
                tracker: Default::default(),
            });
            track!(self.client, feedback);
            self.client.add_client_obj(&feedback)?;

            self.client.event(MainDevice {
                self_id: feedback_id,
                device: ctx.dev,
            });

            self.client.event(FormatTable {
                self_id: feedback_id,
                fd: Rc::new(fd),
                size: size as u32 * 16 as u32,
            });

            self.client.event(TrancheTargetDevice {
                self_id: feedback_id,
                device: ctx.dev,
            });

            let indices = (0..size).collect::<Vec<_>>();

            self.client.event(TrancheFormats {
                self_id: feedback_id,
                indices: indices.as_slice(),
            });

            self.client.event(TrancheFlags {
                self_id: feedback_id,
                flags: 0,
            });

            self.client.event(TrancheDone {
                self_id: feedback_id,
            });

            self.client.event(Done {
                self_id: feedback_id,
            });

            Ok(())
        } else {
            Err(ClientError::InvalidMethod.into())
        }
    }
}

impl ZwpLinuxDmabufFeedbackV1 {
    fn destroy(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), ZwpLinuxDmabufV1Error> {
        let _req: Destroy = self.client.parse(&**self, parser)?;
        self.client.remove_obj(&**self)?;
        Ok(())
    }
}

object_base! {
    ZwpLinuxDmabufV1;

    DESTROY => destroy,
    CREATE_PARAMS => create_params,
    GET_DEFAULT_FEEDBACK => get_default_feedback,
    GET_SURFACE_FEEDBACK => get_surface_feedback,
}

object_base! {
    ZwpLinuxDmabufFeedbackV1;

    DESTROY => destroy,
}

impl Object for ZwpLinuxDmabufV1 {
    fn num_requests(&self) -> u32 {
        GET_SURFACE_FEEDBACK + 1
    }
}

impl Object for ZwpLinuxDmabufFeedbackV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }
}

simple_add_obj!(ZwpLinuxDmabufV1);
simple_add_obj!(ZwpLinuxDmabufFeedbackV1);

#[derive(Debug, Error)]
pub enum ZwpLinuxDmabufV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(ZwpLinuxDmabufV1Error, ClientError);
efrom!(ZwpLinuxDmabufV1Error, MsgParserError);
