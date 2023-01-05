use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ModuleError {
    #[error("Module not found for environment: {environment:?}, region: {region:?}")]
    NotADirectory { environment: String, region: String },
    #[error("Config value {0:?} must be set")]
    IncompleteConfig(String)
}
