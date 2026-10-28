mod clash;
#[allow(clippy::module_inception)]
mod config;
pub(crate) mod dns;
mod encrypt;
mod mixed_port;
mod port;
mod prfitem;
pub mod profiles;
pub mod runtime;
pub(crate) mod snapshot;
pub(crate) mod tropa;
mod verge;
pub(crate) mod warp_account;
pub(crate) mod warp_masque_account;
pub(crate) mod warp_endpoint;
pub(crate) mod warp_profiles;
pub(crate) mod warp_context;
pub(crate) mod warp_selection;

pub(crate) use self::config::Config;
pub use self::{clash::*, encrypt::*, mixed_port::*, prfitem::*, profiles::*, verge::*};

pub const DEFAULT_PAC: &str = r#"function FindProxyForURL(url, host) {
  return "PROXY 127.0.0.1:%mixed-port%; SOCKS5 127.0.0.1:%mixed-port%; DIRECT;";
}
"#;
