#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    FileSystem,
    Network,
    Credential,
    Hook,
}

pub struct Permissions {
    granted: Vec<Permission>,
}

impl Permissions {
    pub fn new(granted: &[Permission]) -> Self {
        Self {
            granted: granted.to_vec(),
        }
    }

    pub fn has(&self, permission: &Permission) -> bool {
        self.granted.contains(permission)
    }
}
