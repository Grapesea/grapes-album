use thiserror::Error;

#[derive(Debug, Error)]
pub enum GrapesError {
    #[error("文件系统错误：{0}")]
    Io(#[from] std::io::Error),
    #[error("数据库错误：{0}")]
    Database(#[from] rusqlite::Error),
    #[error("文件监听错误：{0}")]
    Watcher(#[from] notify::Error),
    #[error("图片处理错误：{0}")]
    Image(#[from] image::ImageError),
    #[error("工作区清单错误：{0}")]
    Manifest(#[from] serde_json::Error),
    #[error("无效的工作区：{0}")]
    InvalidWorkspace(String),
    #[error("无效路径：{0}")]
    InvalidPath(String),
    #[error("名称冲突：{0}")]
    Conflict(String),
    #[error("未找到：{0}")]
    NotFound(String),
    #[error("工作区状态不可用：{0}")]
    State(String),
    #[error("网络请求失败：{0}")]
    Network(String),
    #[error("AI Provider 返回错误：{0}")]
    Provider(String),
}

pub type Result<T> = std::result::Result<T, GrapesError>;
