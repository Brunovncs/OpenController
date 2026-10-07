//! The virtual controller bus on Windows, whichever driver makes the controllers: ViGEmBus, the
//! default, or VIIPER, experimental. Both take the same requests, so the engine never asks which
//! one it has.

use crate::mapping::XusbReport;
use crate::platform::VirtualDriver;
use crate::win::Overlapped;
use crate::{devnode, vigem, viiper};
use crossbeam_channel::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;

pub use crate::vigem::Feedback;

pub enum Bus {
    ViGEm(Arc<vigem::Bus>),
    Viiper(Box<viiper::Bus>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusError {
    ViGEm(vigem::BusError),
    Viiper(viiper::BusError),
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BusError::ViGEm(e) => e.fmt(f),
            BusError::Viiper(e) => e.fmt(f),
        }
    }
}

impl BusError {
    /// How the settings show it.
    pub fn driver(&self) -> crate::engine::Driver {
        match self {
            BusError::ViGEm(e) => e.driver(),
            BusError::Viiper(viiper::BusError::ServerMissing | viiper::BusError::UsbipMissing) => crate::engine::Driver::Missing,
            BusError::Viiper(e) => crate::engine::Driver::Failed(e.to_string()),
        }
    }
}

impl Bus {
    pub fn connect(driver: VirtualDriver) -> Result<Bus, BusError> {
        match driver {
            VirtualDriver::ViGEmBus => vigem::Bus::connect().map(|b| Bus::ViGEm(Arc::new(b))).map_err(BusError::ViGEm),
            VirtualDriver::Viiper => viiper::Bus::start(&viiper::server_exe()).map(|b| Bus::Viiper(Box::new(b))).map_err(BusError::Viiper),
        }
    }

    /// ViGEmBus's driver version, or the version VIIPER's server gives.
    pub fn version(&self) -> Option<String> {
        match self {
            Bus::ViGEm(b) => devnode::driver_version(&b.path),
            Bus::Viiper(b) => Some(b.version().to_string()),
        }
    }

    pub fn lost(&self) -> Option<String> {
        match self {
            Bus::ViGEm(_) => None,
            Bus::Viiper(b) => b.lost(),
        }
    }

    pub fn plug_x360(&self, io: &mut Overlapped) -> Result<u32, BusError> {
        match self {
            Bus::ViGEm(b) => b.plug_x360(io).map_err(BusError::ViGEm),
            Bus::Viiper(b) => b.plug_x360().map_err(BusError::Viiper),
        }
    }

    pub fn unplug(&self, io: &mut Overlapped, serial: u32) -> Result<(), u32> {
        match self {
            Bus::ViGEm(b) => b.unplug(io, serial),
            Bus::Viiper(b) => b.unplug(serial),
        }
    }

    pub fn submit(&self, io: &mut Overlapped, serial: u32, report: &XusbReport) -> Result<(), u32> {
        match self {
            Bus::ViGEm(b) => b.submit(io, serial, report),
            Bus::Viiper(b) => b.submit(serial, report),
        }
    }

    pub fn listen(self: &Arc<Bus>, serial: u32, tx: Sender<Feedback>) -> JoinHandle<()> {
        match &**self {
            Bus::ViGEm(b) => b.listen(serial, tx),
            Bus::Viiper(b) => b.listen(serial, tx),
        }
    }

    pub fn cancel_all(&self) {
        match self {
            Bus::ViGEm(b) => b.cancel_all(),
            Bus::Viiper(b) => b.cancel_all(),
        }
    }
}
