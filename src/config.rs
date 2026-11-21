use std::{net::SocketAddr, path::PathBuf};

use clap::{ArgAction, Parser};

/// Every option can be given as a CLI flag or an environment variable (`FSW_*`).
#[derive(Parser, Clone)]
#[command(version, about, long_about = None)]
pub struct Config {
    /// Address the web app, API and SIP WebSocket listen on.
    #[arg(long, env = "FSW_LISTEN", default_value = "0.0.0.0:8090")]
    pub listen: SocketAddr,

    /// Shared secret with FreeSWITCH (mod_xml_curl and event socket password). 32+ characters.
    #[arg(long, env = "FSW_FREESWITCH_SECRET", hide_env_values = true)]
    pub freeswitch_secret: String,

    /// FreeSWITCH SIP-over-WebSocket listener that browsers are relayed to via `/sip`.
    #[arg(long, env = "FSW_FREESWITCH_WS", default_value = "ws://127.0.0.1:5066")]
    pub freeswitch_ws: String,

    /// SQLite database for users and contacts.
    #[arg(long, env = "FSW_DATABASE_URL", default_value = "sqlite://fs-webrtc.db")]
    pub database_url: String,

    /// Let anyone create an account. Set to false once everyone has joined.
    #[arg(long, env = "FSW_SIGNUP", default_value_t = true, action = ArgAction::Set)]
    pub signup: bool,

    /// When set, creating an account requires this invite code.
    #[arg(long, env = "FSW_INVITE_CODE", hide_env_values = true)]
    pub invite_code: Option<String>,

    /// Comma-separated STUN/TURN URLs given to browsers.
    #[arg(long, env = "FSW_ICE_SERVERS", value_delimiter = ',', default_value = "stun:stun.l.google.com:19302")]
    pub ice_servers: Vec<String>,

    /// Directory with the built web app.
    #[arg(long, env = "FSW_WEB_DIR", default_value = "web/dist")]
    pub web_dir: PathBuf,

    /// Log as JSON lines instead of human-readable text.
    #[arg(long, env = "FSW_LOG_JSON", default_value_t = false, action = ArgAction::Set)]
    pub log_json: bool,
}

impl Config {
    /// Checks the settings and drops empty values (`FSW_INVITE_CODE=` in an env file means "none").
    pub fn validate(&mut self) -> Result<(), String> {
        self.invite_code = self.invite_code.take().filter(|c| !c.trim().is_empty());
        self.ice_servers.retain(|s| !s.trim().is_empty());
        // It is also written into FreeSWITCH XML and HTTP Basic credentials, so keep it to safe characters.
        let s = &self.freeswitch_secret;
        if s.len() < 32 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)) {
            return Err(
                "FSW_FREESWITCH_SECRET must be 32+ characters of [A-Za-z0-9_-] (try `openssl rand -hex 32`)".into()
            );
        }
        // The relay client is built without TLS: FreeSWITCH is expected on loopback or a private network.
        if !self.freeswitch_ws.starts_with("ws://") {
            return Err("FSW_FREESWITCH_WS must be a ws:// URL".into());
        }
        Ok(())
    }
}
