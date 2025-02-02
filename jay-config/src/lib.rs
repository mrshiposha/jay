//! This crate allows you to configure the Jay compositor.
//!
//! A minimal example configuration looks as follows:
//!
//! ```rust
//! use jay_config::config;
//!
//! fn configure() {
//!
//! }
//!
//! config!(configure);
//! ```
//!
//! This configuration will not allow you to interact with the compositor at all nor exit it.
//! To add at least that much functionality, add the following code to `configure`:
//!
//! ```rust
//! use jay_config::{config, quit};
//! use jay_config::input::{get_seat, input_devices, on_new_input_device};
//! use jay_config::keyboard::mods::ALT;
//! use jay_config::keyboard::syms::SYM_q;
//!
//! fn configure() {
//!     // Create a seat.
//!     let seat = get_seat("default");
//!     // Create a key binding to exit the compositor.
//!     seat.bind(ALT | SYM_q, || quit());
//!     // Assign all current and future input devices to this seat.
//!     input_devices().into_iter().for_each(move |d| d.set_seat(seat));
//!     on_new_input_device(move |d| d.set_seat(seat));
//! }
//!
//! config!(configure);
//! ```

#![allow(
    clippy::zero_prefixed_literal,
    clippy::manual_range_contains,
    clippy::uninlined_format_args
)]

use {
    crate::keyboard::ModifiedKeySym,
    bincode::{Decode, Encode},
    std::fmt::{Debug, Display, Formatter},
};

#[macro_use]
mod macros;
#[doc(hidden)]
pub mod _private;
pub mod embedded;
pub mod exec;
pub mod input;
pub mod keyboard;
pub mod logging;
pub mod status;
pub mod theme;
pub mod timer;
pub mod video;

/// A planar direction.
#[derive(Encode, Decode, Copy, Clone, Debug, Eq, PartialEq)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

/// A planar axis.
#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// Returns the axis orthogonal to `self`.
    pub fn other(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

/// Exits the compositor.
pub fn quit() {
    get!().quit()
}

/// Switches to a different VT.
pub fn switch_to_vt(n: u32) {
    get!().switch_to_vt(n)
}

/// Reloads the configuration.
///
/// If the configuration cannot be reloaded, this function has no effect.
pub fn reload() {
    get!().reload()
}

/// Returns whether this execution of the configuration function is due to a reload.
///
/// This can be used to decide whether the configuration should auto-start programs.
pub fn is_reload() -> bool {
    get!(false).is_reload()
}

/// Sets whether new workspaces are captured by default.
///
/// The default is `true`.
pub fn set_default_workspace_capture(capture: bool) {
    get!().set_default_workspace_capture(capture)
}

/// Returns whether new workspaces are captured by default.
pub fn get_default_workspace_capture() -> bool {
    get!(true).get_default_workspace_capture()
}

/// Toggles whether new workspaces are captured by default.
pub fn toggle_default_workspace_capture() {
    let get = get!();
    get.set_default_workspace_capture(!get.get_default_workspace_capture());
}

/// A workspace.
#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Workspace(pub u64);

impl Workspace {
    /// Returns whether this workspace existed at the time `Seat::get_workspace` was called.
    pub fn exists(self) -> bool {
        self.0 != 0
    }

    /// Sets whether the workspaces is captured.
    ///
    /// The default is determined by `set_default_workspace_capture`.
    pub fn set_capture(self, capture: bool) {
        get!().set_workspace_capture(self, capture)
    }

    /// Returns whether the workspaces is captured.
    pub fn get_capture(self) -> bool {
        get!(true).get_workspace_capture(self)
    }

    /// Toggles whether the workspaces is captured.
    pub fn toggle_capture(self) {
        let get = get!();
        get.set_workspace_capture(self, !get.get_workspace_capture(self));
    }
}

/// Returns the workspace with the given name.
///
/// Workspaces are identified by their name. Calling this function alone does not create the
/// workspace if it doesn't already exist.
pub fn get_workspace(name: &str) -> Workspace {
    get!(Workspace(0)).get_workspace(name)
}

/// A PCI ID.
///
/// PCI IDs can be used to identify a hardware component. See the Debian [documentation][pci].
///
/// [pci]: https://wiki.debian.org/HowToIdentifyADevice/PCI
#[derive(Encode, Decode, Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct PciId {
    pub vendor: u32,
    pub model: u32,
}

impl Display for PciId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor, self.model)
    }
}

/// Sets the callback to be called when the display goes idle.
pub fn on_idle<F: Fn() + 'static>(f: F) {
    get!().on_idle(f)
}

/// Sets the callback to be called when all devices have been enumerated.
///
/// This callback is only invoked once during the lifetime of the compositor. This is a
/// good place to select the DRM device used for rendering.
pub fn on_devices_enumerated<F: FnOnce() + 'static>(f: F) {
    get!().on_devices_enumerated(f)
}
