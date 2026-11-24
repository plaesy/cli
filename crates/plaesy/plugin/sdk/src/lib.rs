//! SDK plugin (ADR 0003: plugin/sdk). Keputusan default, didelegasikan dan bisa dicabut:
//!
//! - Plugin mendeklarasikan kapabilitas yang dibutuhkannya di [`Manifest`].
//! - Host hanya memuat plugin bila setiap kapabilitasnya diberi izin lewat [`Grants`].
//!   Kapabilitas yang tidak diberi menyebabkan galat saat memuat, bukan saat berjalan.
//! - Saat berjalan, plugin hanya bisa memakai kapabilitas yang sudah diberi izin
//!   ([`Context::require`]).
//!
//! SDK ini tidak melakukan I/O. Kapabilitas hanya penanda; penegakannya ada pada host dan
//! pada adapter yang memberi akses nyata.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Kapabilitas yang bisa dideklarasikan plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// Membaca dokumen dari penyimpanan.
    ReadDocuments,
    /// Menulis dokumen ke penyimpanan.
    WriteDocuments,
    /// Mengakses jaringan.
    Network,
    /// Menjalankan proses lokal.
    Process,
}

impl Capability {
    /// Nama stabil yang dipakai di manifest dan pesan galat.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadDocuments => "read_documents",
            Self::WriteDocuments => "write_documents",
            Self::Network => "network",
            Self::Process => "process",
        }
    }
}

/// Identitas dan kapabilitas plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub capabilities: BTreeSet<Capability>,
}

/// Galat plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// Plugin meminta kapabilitas yang tidak diberi izin.
    CapabilityDenied {
        plugin: String,
        capability: Capability,
    },
    /// Nama plugin sudah terdaftar.
    Duplicate(String),
    /// Plugin tidak dikenal.
    Unknown(String),
    /// Plugin gagal saat berjalan.
    Failed { plugin: String, message: String },
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityDenied { plugin, capability } => write!(
                f,
                "plugin {plugin:?} butuh kapabilitas {} yang tidak diberi izin",
                capability.name()
            ),
            Self::Duplicate(name) => write!(f, "plugin {name:?} sudah terdaftar"),
            Self::Unknown(name) => write!(f, "plugin {name:?} tidak dikenal"),
            Self::Failed { plugin, message } => write!(f, "plugin {plugin:?} gagal: {message}"),
        }
    }
}

impl std::error::Error for PluginError {}

/// Konteks yang diberikan host saat plugin berjalan. Hanya kapabilitas yang diizinkan yang lolos.
#[derive(Debug)]
pub struct Context<'a> {
    plugin: &'a str,
    granted: &'a BTreeSet<Capability>,
}

impl Context<'_> {
    /// Memastikan plugin boleh memakai `capability`.
    ///
    /// # Errors
    ///
    /// [`PluginError::CapabilityDenied`] bila kapabilitas tidak diizinkan.
    pub fn require(&self, capability: Capability) -> Result<(), PluginError> {
        if self.granted.contains(&capability) {
            Ok(())
        } else {
            Err(PluginError::CapabilityDenied {
                plugin: self.plugin.to_owned(),
                capability,
            })
        }
    }
}

/// Plugin: menyatakan manifestnya dan menjalankan satu permintaan.
pub trait Plugin: Send + Sync {
    /// Manifest plugin. Harus tetap sama selama plugin hidup.
    fn manifest(&self) -> Manifest;

    /// Menjalankan plugin dengan `input`. Kapabilitas dicek lewat `ctx`.
    ///
    /// # Errors
    ///
    /// Galat yang dilaporkan plugin.
    fn run(&self, ctx: &Context<'_>, input: &Value) -> Result<Value, String>;
}

/// Izin yang diberikan host, per nama plugin. Tidak ada izin bila plugin tidak tercantum.
#[derive(Debug, Clone, Default)]
pub struct Grants {
    by_plugin: BTreeMap<String, BTreeSet<Capability>>,
}

impl Grants {
    /// Memberi `capabilities` kepada plugin `name`.
    pub fn grant(&mut self, name: &str, capabilities: &[Capability]) -> &mut Self {
        self.by_plugin
            .entry(name.to_owned())
            .or_default()
            .extend(capabilities.iter().copied());
        self
    }

    fn for_plugin(&self, name: &str) -> BTreeSet<Capability> {
        self.by_plugin.get(name).cloned().unwrap_or_default()
    }
}

/// Host: memuat plugin dengan izin dan menjalankannya.
#[derive(Default)]
pub struct Host {
    plugins: BTreeMap<String, Box<dyn Plugin>>,
    grants: Grants,
}

impl fmt::Debug for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Host")
            .field("plugins", &self.plugins.keys().collect::<Vec<_>>())
            .field("grants", &self.grants)
            .finish()
    }
}

impl Host {
    /// Host dengan izin yang diberikan.
    #[must_use]
    pub fn new(grants: Grants) -> Self {
        Self {
            plugins: BTreeMap::new(),
            grants,
        }
    }

    /// Memuat plugin. Galat bila nama sudah ada atau ada kapabilitas yang tidak diizinkan.
    ///
    /// # Errors
    ///
    /// [`PluginError::Duplicate`] atau [`PluginError::CapabilityDenied`].
    pub fn load(&mut self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let manifest = plugin.manifest();
        if self.plugins.contains_key(&manifest.name) {
            return Err(PluginError::Duplicate(manifest.name));
        }
        let granted = self.grants.for_plugin(&manifest.name);
        if let Some(missing) = manifest.capabilities.iter().find(|c| !granted.contains(c)) {
            return Err(PluginError::CapabilityDenied {
                plugin: manifest.name,
                capability: *missing,
            });
        }
        self.plugins.insert(manifest.name, plugin);
        Ok(())
    }

    /// Nama plugin yang sudah dimuat, terurut.
    #[must_use]
    pub fn loaded(&self) -> Vec<&str> {
        self.plugins.keys().map(String::as_str).collect()
    }

    /// Menjalankan plugin `name` dengan `input`.
    ///
    /// # Errors
    ///
    /// [`PluginError::Unknown`], atau galat yang dilaporkan plugin.
    pub fn run(&self, name: &str, input: &Value) -> Result<Value, PluginError> {
        let plugin = self
            .plugins
            .get(name)
            .ok_or_else(|| PluginError::Unknown(name.to_owned()))?;
        // Hanya kapabilitas yang dideklarasikan di manifest DAN diizinkan host yang boleh dipakai.
        let declared = plugin.manifest().capabilities;
        let allowed: BTreeSet<Capability> = self
            .grants
            .for_plugin(name)
            .intersection(&declared)
            .copied()
            .collect();
        let ctx = Context {
            plugin: name,
            granted: &allowed,
        };
        plugin
            .run(&ctx, input)
            .map_err(|message| PluginError::Failed {
                plugin: name.to_owned(),
                message,
            })
    }
}

#[cfg(test)]
#[path = "sdk_test.rs"]
mod sdk_test;
