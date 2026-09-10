pub mod subscriptions;
pub mod servers;
pub mod routing;
pub mod processes;
pub mod tun;
pub mod proxy;
pub mod dns;
pub mod general;
pub mod logs;
pub mod diagnostics;

use crate::client::IpcClient;

pub trait Tab {
    fn set_visible(&self, visible: bool);
    fn on_activated(&mut self, client: &IpcClient);
    fn on_command(&mut self, client: &IpcClient, id: usize, code: u16);
}
