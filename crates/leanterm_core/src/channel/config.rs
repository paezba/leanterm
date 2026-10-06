use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::AppId;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChannelConfig {
    /// The application ID for this channel.
    pub app_id: AppId,

    /// The name of the file to which logs should be written.
    pub logfile_name: Cow<'static, str>,

    /// Configuration for talking to Leanterm's servers.
    pub server_config: LeantermServerConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LeantermServerConfig {
    /// The root URL for the standard server pool.
    pub server_root_url: Cow<'static, str>,
}

impl LeantermServerConfig {
    pub fn production() -> Self {
        Self {
            server_root_url: "https://app.warp.dev".into(),
        }
    }
}
