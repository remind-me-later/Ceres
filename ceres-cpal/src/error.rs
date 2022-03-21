#[derive(Debug)]
pub enum Error {
    OutputDeviceNotFound,
    SupportedStreamConfig,
    UncapableStreamConfig,
    Initialization,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            OutputDeviceNotFound => write!(f, "couldn't find output device"),
            SupportedStreamConfig => write!(f, "couldn't get supported stream configurations"),
            UncapableStreamConfig => write!(f, "couldn't get any configuration able to stream"),
            Initialization => write!(f, "couldn't initialize audio stream"),
        }
    }
}

impl std::error::Error for Error {}
