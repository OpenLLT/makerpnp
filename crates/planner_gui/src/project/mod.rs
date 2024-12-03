use std::path::PathBuf;
use cushy::value::Dynamic;
use slotmap::new_key_type;
use crate::app_core::CoreService;

new_key_type! {
    /// A key for a project
    pub struct ProjectKey;
}

#[derive(Debug, Clone)]
pub enum ProjectMessage {
    None,
    Load,
}

pub struct Project {
    pub(crate) name: Dynamic<Option<String>>,
    pub(crate) path: PathBuf,
    core_service: CoreService,
}

impl Project {
    pub fn from_path(path: PathBuf) -> (Self, ProjectMessage) {
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::default(),
            path,
            core_service,
        };
        
        (instance, ProjectMessage::Load)
    }
}