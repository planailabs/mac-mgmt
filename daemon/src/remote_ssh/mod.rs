pub mod fifo_watcher;
pub mod host_keys;
pub mod pty;
pub mod relay_client;
pub mod ssh_server;
pub mod ws_stream;

#[derive(Debug)]
pub enum RemoteSshCommand {
    Enable,
    Disable,
}
