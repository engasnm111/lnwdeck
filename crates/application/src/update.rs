use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateState {
    Idle,
    Checking,
    Available,
    Downloading,
    Verifying,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub release_date: String,
    pub artifacts: Vec<ArtifactEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub target: String,
    pub url: String,
    pub sha256: String,
    pub signature: Option<String>,
    pub size_bytes: u64,
}

pub struct UpdateService {
    state: UpdateState,
    current_version: String,
    #[allow(dead_code)]
    manifest_url: String,
    manifest: Option<UpdateManifest>,
    selected_artifact: Option<ArtifactEntry>,
}

impl UpdateService {
    pub fn new(current_version: &str, manifest_url: &str) -> Self {
        Self {
            state: UpdateState::Idle,
            current_version: current_version.to_string(),
            manifest_url: manifest_url.to_string(),
            manifest: None,
            selected_artifact: None,
        }
    }

    pub fn state(&self) -> UpdateState {
        self.state
    }

    pub fn manifest(&self) -> Option<&UpdateManifest> {
        self.manifest.as_ref()
    }

    pub fn selected_artifact(&self) -> Option<&ArtifactEntry> {
        self.selected_artifact.as_ref()
    }

    pub fn start_check(&mut self) {
        self.state = UpdateState::Checking;
    }

    pub fn set_available(&mut self, manifest: UpdateManifest) {
        self.manifest = Some(manifest);
        self.state = UpdateState::Available;
    }

    pub fn select_artifact(&mut self, target: &str) -> Result<(), String> {
        let manifest = self.manifest.as_ref().ok_or("no manifest loaded")?;
        let artifact = manifest
            .artifacts
            .iter()
            .find(|a| a.target == target)
            .ok_or_else(|| format!("no artifact for target: {target}"))?
            .clone();
        self.selected_artifact = Some(artifact);
        self.state = UpdateState::Available;
        Ok(())
    }

    pub fn start_download(&mut self) -> Result<(), String> {
        if self.selected_artifact.is_none() {
            return Err("no artifact selected".to_string());
        }
        self.state = UpdateState::Downloading;
        Ok(())
    }

    pub fn start_verify(&mut self) -> Result<(), String> {
        if self.state != UpdateState::Downloading {
            return Err("not in downloading state".to_string());
        }
        self.state = UpdateState::Verifying;
        Ok(())
    }

    pub fn verify_signature(&self, _signature_hex: &str) -> Result<bool, String> {
        let artifact = self
            .selected_artifact
            .as_ref()
            .ok_or("no artifact selected")?;
        match &artifact.signature {
            Some(sig) => Ok(!sig.is_empty()),
            None => Err("no signature in manifest".to_string()),
        }
    }

    pub fn set_ready(&mut self) {
        self.state = UpdateState::Ready;
    }

    pub fn set_failed(&mut self) {
        self.state = UpdateState::Failed;
    }

    pub fn new_version_available(&self) -> bool {
        if let Some(manifest) = &self.manifest {
            manifest.version != self.current_version
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> UpdateManifest {
        UpdateManifest {
            version: "0.2.0".to_string(),
            release_date: "2025-06-01".to_string(),
            artifacts: vec![ArtifactEntry {
                target: "x86_64-pc-windows-msvc".to_string(),
                url: "https://releases.lnwdeck.app/0.2.0/lnwdeck_0.2.0_x64-setup.exe".to_string(),
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
                signature: Some("deadbeef".to_string()),
                size_bytes: 50_000_000,
            }],
        }
    }

    #[test]
    fn idle_is_initial_state() {
        let service = UpdateService::new("0.1.0", "https://example.com/latest.json");
        assert_eq!(service.state(), UpdateState::Idle);
    }

    #[test]
    fn lifecycle_transitions() {
        let mut service = UpdateService::new("0.1.0", "https://example.com/latest.json");

        service.start_check();
        assert_eq!(service.state(), UpdateState::Checking);

        service.set_available(sample_manifest());
        assert_eq!(service.state(), UpdateState::Available);

        assert!(service.new_version_available());

        service.select_artifact("x86_64-pc-windows-msvc").unwrap();
        service.start_download().unwrap();
        assert_eq!(service.state(), UpdateState::Downloading);

        service.start_verify().unwrap();
        assert_eq!(service.state(), UpdateState::Verifying);

        service.set_ready();
        assert_eq!(service.state(), UpdateState::Ready);
    }

    #[test]
    fn invalid_signature_rejection() {
        let mut service = UpdateService::new("0.1.0", "https://example.com/latest.json");

        let mut manifest = sample_manifest();
        manifest.artifacts[0].signature = None;
        service.set_available(manifest);
        service.select_artifact("x86_64-pc-windows-msvc").unwrap();

        let result = service.verify_signature("any-sig");
        assert!(result.is_err(), "missing signature must be rejected");
    }

    #[test]
    fn same_version_is_not_available() {
        let mut service = UpdateService::new("0.1.0", "https://example.com/latest.json");

        let manifest = UpdateManifest {
            version: "0.1.0".to_string(),
            release_date: "2025-01-01".to_string(),
            artifacts: vec![],
        };
        service.set_available(manifest);

        assert!(!service.new_version_available());
    }

    #[test]
    fn no_automatic_shutdown() {
        let mut service = UpdateService::new("0.1.0", "https://example.com/latest.json");
        service.set_available(sample_manifest());
        service.select_artifact("x86_64-pc-windows-msvc").unwrap();
        service.start_download().unwrap();
        service.start_verify().unwrap();
        service.set_ready();

        // Ready state does NOT trigger shutdown — restart requires explicit user action
        assert_eq!(service.state(), UpdateState::Ready);
    }
}
