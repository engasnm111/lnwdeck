//! Adapter capability descriptors.
//!
//! A descriptor is the single declaration of what an adapter is and what it
//! can actually collect. The runtime uses it to decide whether a channel may
//! be called at all, so an adapter can no longer claim a successful
//! collection while returning nothing: an `Unsupported` channel is reported
//! as `NOT_SUPPORTED`, and a channel declared as supported must return data
//! or an error code.
//!
//! Descriptors also replace the provider id/name tables that used to be
//! duplicated across the application layer and the Tauri commands.

use serde::Serialize;

/// How well an adapter supports one collection channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSupport {
    /// The provider itself reports authoritative values.
    Native,
    /// Derived from local provider artifacts. Values are real measurements
    /// but limits may be unknown, so quota windows are usage-only.
    LocalEstimate,
    /// Not implemented. The runtime never calls the channel.
    Unsupported,
}

impl ChannelSupport {
    /// True when the runtime may invoke this channel.
    pub fn is_supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }

    /// Label shown in diagnostics and in the provider table.
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "supported",
            Self::LocalEstimate => "local estimate",
            Self::Unsupported => "not supported",
        }
    }
}

/// Where an adapter's data comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Local SQLite database written by the provider's tooling.
    LocalSqlite,
    /// Local newline-delimited JSON session logs.
    LocalJsonl,
    /// Local plain-text or structured log directory.
    LocalLog,
    /// Local HTTP API exposed by a locally running engine.
    LocalApi,
    /// Remote HTTP API that requires user-supplied credentials.
    RemoteApi,
    /// No source is wired up yet.
    None,
}

impl SourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::LocalSqlite => "local_sqlite",
            Self::LocalJsonl => "local_jsonl",
            Self::LocalLog => "local_log",
            Self::LocalApi => "local_api",
            Self::RemoteApi => "remote_api",
            Self::None => "none",
        }
    }

    /// True when reaching the source requires network access.
    pub fn is_remote(self) -> bool {
        matches!(self, Self::RemoteApi)
    }
}

/// What the adapter needs in order to read its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// Nothing required.
    None,
    /// Read access to local files written by the provider's tooling.
    LocalFiles,
    /// A user-supplied API key held in the Windows Credential Manager.
    ApiKey,
    /// A user-supplied browser session cookie held in the Windows Credential
    /// Manager. The adapter must never expose the cookie to the UI.
    BrowserCookie,
}

/// Immutable declaration of an adapter's identity and capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdapterDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub vendor: &'static str,
    pub source_kind: SourceKind,
    pub usage_support: ChannelSupport,
    pub quota_support: ChannelSupport,
    pub auth: AuthKind,
    pub adapter_version: &'static str,
}

impl AdapterDescriptor {
    /// True when neither channel is implemented. Such a provider is listed
    /// as not supported and is never reported as healthy.
    pub fn is_inert(&self) -> bool {
        !self.usage_support.is_supported() && !self.quota_support.is_supported()
    }

    /// True when the adapter cannot work until the user supplies credentials.
    pub fn needs_credentials(&self) -> bool {
        matches!(self.auth, AuthKind::ApiKey | AuthKind::BrowserCookie)
    }

    /// Checks the descriptor's internal consistency. Returns the reason when
    /// the declaration itself is contradictory.
    pub fn check(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("descriptor id must not be empty".to_string());
        }
        if self.display_name.trim().is_empty() {
            return Err(format!("descriptor {} has an empty display name", self.id));
        }
        if self.adapter_version.trim().is_empty() {
            return Err(format!("descriptor {} has an empty version", self.id));
        }
        if self.is_inert() && self.source_kind != SourceKind::None {
            return Err(format!(
                "descriptor {} declares source {} but supports no channel",
                self.id,
                self.source_kind.label()
            ));
        }
        if !self.is_inert() && self.source_kind == SourceKind::None {
            return Err(format!(
                "descriptor {} supports a channel but declares no source",
                self.id
            ));
        }
        if self.source_kind.is_remote() && self.auth == AuthKind::None {
            return Err(format!(
                "descriptor {} reads a remote API and must declare its auth requirement",
                self.id
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> AdapterDescriptor {
        AdapterDescriptor {
            id: "sample",
            display_name: "Sample",
            vendor: "Sample Vendor",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Unsupported,
            auth: AuthKind::LocalFiles,
            adapter_version: "0.2.0",
        }
    }

    #[test]
    fn supported_channels_are_callable() {
        assert!(ChannelSupport::Native.is_supported());
        assert!(ChannelSupport::LocalEstimate.is_supported());
        assert!(!ChannelSupport::Unsupported.is_supported());
    }

    #[test]
    fn inert_descriptor_needs_no_source() {
        let inert = AdapterDescriptor {
            source_kind: SourceKind::None,
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Unsupported,
            auth: AuthKind::None,
            ..descriptor()
        };
        assert!(inert.is_inert());
        inert.check().expect("consistent inert descriptor");
    }

    #[test]
    fn descriptor_with_source_but_no_channel_is_rejected() {
        let broken = AdapterDescriptor {
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Unsupported,
            ..descriptor()
        };
        assert!(
            broken.check().is_err(),
            "declaring a source while collecting nothing is contradictory"
        );
    }

    #[test]
    fn descriptor_with_channel_but_no_source_is_rejected() {
        let broken = AdapterDescriptor {
            source_kind: SourceKind::None,
            ..descriptor()
        };
        assert!(broken.check().is_err());
    }

    #[test]
    fn remote_descriptor_must_declare_auth() {
        let broken = AdapterDescriptor {
            source_kind: SourceKind::RemoteApi,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::None,
            ..descriptor()
        };
        assert!(broken.check().is_err());

        let ok = AdapterDescriptor {
            auth: AuthKind::ApiKey,
            ..broken
        };
        ok.check().expect("api key descriptor is consistent");
        assert!(ok.needs_credentials());

        let browser_cookie = AdapterDescriptor {
            auth: AuthKind::BrowserCookie,
            ..ok
        };
        browser_cookie
            .check()
            .expect("browser cookie descriptor is consistent");
        assert!(browser_cookie.needs_credentials());
    }

    #[test]
    fn empty_identity_fields_are_rejected() {
        for broken in [
            AdapterDescriptor {
                id: "  ",
                ..descriptor()
            },
            AdapterDescriptor {
                display_name: "",
                ..descriptor()
            },
            AdapterDescriptor {
                adapter_version: "",
                ..descriptor()
            },
        ] {
            assert!(broken.check().is_err());
        }
    }
}
