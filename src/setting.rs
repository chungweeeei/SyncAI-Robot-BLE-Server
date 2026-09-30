use bluer::gatt::local::ReqError;
use serde::Deserialize;

pub const MAX_COMMAND_LENGTH: usize = 512;
const SSID_MAX: usize = 32;
const PASSWORD_MAX: usize = 63;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag="cmd", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    SetWifi {
        id: u8,
        ssid: String,
        password: String
    },
    Disconnect {
        id: u8
    }
}

#[derive(Debug)]
pub enum CommandError {
    TooLong(usize),
    Malformed(serde_json::Error),
    InvalidSsid,
    InvalidPassword,
}

impl Command {
    pub fn parse(data: &[u8]) -> Result<Self, CommandError> {
        if data.len() > MAX_COMMAND_LENGTH {
            return Err(CommandError::TooLong(data.len()));
        }
        let cmd: Self = serde_json::from_slice(data).map_err(CommandError::Malformed)?;
        cmd.validate()?;
        Ok(cmd)
    }

    pub fn id(&self) -> u8 {
        match self {
            Self::SetWifi { id, .. } | Self::Disconnect { id, .. } => *id,
        }
    }

    fn validate(&self) -> Result<(), CommandError> {
        if let Self::SetWifi { ssid, password, .. } = self {
            if ssid.is_empty() || ssid.len() > SSID_MAX {
                return Err(CommandError::InvalidSsid);
            }
            if password.len() > PASSWORD_MAX {
                return Err(CommandError::InvalidPassword);
            }
        }
        Ok(())
    }
}

impl From<CommandError> for ReqError {
    fn from(e: CommandError) -> Self {
        match e {
            CommandError::TooLong(_) => Self::InvalidValueLength,
            CommandError::Malformed(_) => Self::NotSupported,
            CommandError::InvalidSsid | CommandError::InvalidPassword => Self::Failed,
        }
    }
}
