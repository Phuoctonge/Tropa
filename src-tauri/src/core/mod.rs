pub mod auto_backup;
pub mod autostart;
pub mod backup;
pub mod hotkey;
pub mod lightweight;
pub mod listener;
pub mod logger;
pub mod manager;
#[cfg(target_os = "macos")]
pub mod network_watch;
pub mod notify;
pub(crate) mod owner_identity;
pub mod proxy_control;
pub mod proxy_view;
pub mod runstate;
pub(crate) mod runtime_bundle;
pub mod service;
pub mod sysopt;
pub mod timer;
pub mod tray;
pub mod updater;
pub mod validate;
#[cfg(all(windows, any(feature = "portable", feature = "tropa-probe", test)))]
pub(crate) mod warp_children;
#[cfg(all(windows, feature = "portable"))]
pub(crate) mod warp_connection;
#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_discovery;
#[cfg(all(windows, feature = "portable"))]
pub(crate) mod warp_elevation;
#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_probe;
pub(crate) mod warp_failure;
#[cfg(all(windows, feature = "portable"))]
pub(crate) mod warp_diagnostics;
pub(crate) mod warp_metadata;
pub(crate) mod warp_registration;
#[cfg(all(windows, feature = "portable", feature = "tropa-probe"))]
pub(crate) mod warp_smoke;
pub mod win_uwp;

pub use self::{manager::CoreManager, timer::Timer, updater::SilentUpdater};
pub use notify::{handle, notification};

#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_search;

#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_scout;

#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_timing;

pub(crate) mod warp_masque_registration;

#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_engine;

#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_targets;

#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) mod warp_runtime;

#[cfg(all(windows, feature = "portable"))]
pub(crate) mod warp_health;
