use std::fmt;

#[derive(Debug)]
pub enum Error {
    Http(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    Zip(String),
    Auth(String),
    Launch(String),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Http(e) => write!(f, "网络请求失败: {e}"),
            Error::Io(e) => write!(f, "文件读写失败: {e}"),
            Error::Json(e) => write!(f, "数据解析失败: {e}"),
            Error::Zip(e) => write!(f, "压缩包处理失败: {e}"),
            Error::Auth(e) => write!(f, "登录失败: {e}"),
            Error::Launch(e) => write!(f, "启动失败: {e}"),
            Error::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::Json(value)
    }
}

impl From<ureq::Error> for Error {
    fn from(value: ureq::Error) -> Self {
        Error::Http(value.to_string())
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(value: zip::result::ZipError) -> Self {
        Error::Zip(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
