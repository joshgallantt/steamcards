//! The composition root: every concrete type in the app is named in one of
//! its three phases and nowhere else, so swapping one is a change to this
//! folder alone.
//!
//! The phases are the layers the architecture already has — `DataAssembler`
//! names the file and the Steam session, `DomainAssembler` builds each
//! component over them, `PresentationAssembler` builds the screens from use
//! cases. Each is handed only the phase before it, so the wiring runs one way.
//!
//! Not unit tested: it is wiring, with no behaviour of its own.

use config_file::ConfigLock;

use crate::settings::Settings;

mod data;
mod domain;
mod presentation;

pub(crate) use data::DataAssembler;
pub(crate) use domain::DomainAssembler;
pub(crate) use presentation::PresentationAssembler;

pub(crate) struct CompositionRoot {
    pub presentation: PresentationAssembler,
    /// Held until steamcards exits: see `DataAssembler::lock`.
    _lock: ConfigLock,
}

impl CompositionRoot {
    pub(crate) fn new(settings: &Settings) -> anyhow::Result<Self> {
        let data = DataAssembler::new(settings)?;
        let domain = DomainAssembler::new(&data);
        let presentation = PresentationAssembler::new(domain);
        Ok(Self {
            presentation,
            _lock: data.lock,
        })
    }
}
