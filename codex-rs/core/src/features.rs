//! Centralized feature flags and metadata.
//!
//! This module defines a small set of toggles that gate experimental and
//! optional behavior across the codebase. Instead of wiring individual
//! booleans through multiple types, call sites consult a single `Features`
//! container attached to `Config`.

use crate::config::ConfigToml;
use crate::config::profile::ConfigProfile;
#[cfg(feature = "config-schema")]
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

mod legacy;
pub(crate) use legacy::LegacyFeatureToggles;

/// High-level lifecycle stage for a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Experimental,
    Beta {
        name: &'static str,
        menu_description: &'static str,
        announcement: &'static str,
    },
    Stable,
    Deprecated,
    Removed,
}

impl Stage {
    pub fn beta_menu_name(self) -> Option<&'static str> {
        match self {
            Stage::Beta { name, .. } => Some(name),
            _ => None,
        }
    }

    pub fn beta_menu_description(self) -> Option<&'static str> {
        match self {
            Stage::Beta {
                menu_description, ..
            } => Some(menu_description),
            _ => None,
        }
    }

    pub fn beta_announcement(self) -> Option<&'static str> {
        match self {
            Stage::Beta { announcement, .. } => Some(announcement),
            _ => None,
        }
    }

    /// Returns a short label for use in schema descriptions.
    pub fn schema_label(self) -> &'static str {
        match self {
            Stage::Stable => "Stable",
            Stage::Beta { .. } => "Beta",
            Stage::Experimental => "Experimental",
            Stage::Deprecated => "Deprecated",
            Stage::Removed => "Removed",
        }
    }
}

/// Metadata for a single feature definition.
#[derive(Debug, Clone, Copy)]
pub struct FeatureSpec {
    pub id: Feature,
    pub key: &'static str,
    pub stage: Stage,
    pub default_enabled: bool,
}

/// Generates the `Feature` enum, its methods, and the `FEATURES` array from a
/// single source of truth. Each entry produces:
/// - An enum variant with `#[doc = $desc]` for IDE hover/rustdoc
/// - `description()` returning the same literal at runtime
/// - `key()`, `stage()`, `default_enabled()` via direct match arms (no lookup)
/// - An entry in the `FEATURES` array for iteration
macro_rules! define_features {
    ($(
        $variant:ident {
            desc: $desc:literal,
            key: $key:literal,
            stage: $stage:expr,
            default: $default:expr $(,)?
        }
    ),+ $(,)?) => {
        /// Unique features toggled via configuration.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Feature {
            $(
                #[doc = $desc]
                $variant,
            )+
        }

        impl Feature {
            pub fn key(self) -> &'static str {
                match self { $(Self::$variant => $key,)+ }
            }

            /// Returns the description for this feature (same as the doc comment).
            pub fn description(self) -> &'static str {
                match self { $(Self::$variant => $desc,)+ }
            }

            pub fn stage(self) -> Stage {
                match self { $(Self::$variant => $stage,)+ }
            }

            pub fn default_enabled(self) -> bool {
                match self { $(Self::$variant => $default,)+ }
            }
        }

        /// Single registry of all feature definitions.
        pub const FEATURES: &[FeatureSpec] = &[
            $(
                FeatureSpec {
                    id: Feature::$variant,
                    key: $key,
                    stage: $stage,
                    default_enabled: $default,
                },
            )+
        ];
    };
}

define_features! {
    // Stable
    GhostCommit {
        desc: "Create a ghost commit at each turn.",
        key: "undo",
        stage: Stage::Stable,
        default: false,
    },
    ParallelToolCalls {
        desc: "Allow model to call multiple tools in parallel (only for models supporting it).",
        key: "parallel",
        stage: Stage::Stable,
        default: true,
    },
    ViewImageTool {
        desc: "Include the view_image tool.",
        key: "view_image_tool",
        stage: Stage::Stable,
        default: true,
    },
    ShellTool {
        desc: "Enable the default shell tool.",
        key: "shell_tool",
        stage: Stage::Stable,
        default: true,
    },
    ModelWarnings {
        desc: "Send warnings to the model to correct it on the tool usage.",
        key: "warnings",
        stage: Stage::Stable,
        default: true,
    },
    WebSearchRequest {
        desc: "Allow the model to request web searches.",
        key: "web_search_request",
        stage: Stage::Stable,
        default: false,
    },

    // Beta
    UnifiedExec {
        desc: "Use the single unified PTY-backed exec tool.",
        key: "unified_exec",
        stage: Stage::Beta {
            name: "Background terminal",
            menu_description: "Run long-running terminal commands in the background.",
            announcement: "NEW! Try Background terminals for long running processes. Enable in /experimental!",
        },
        default: false,
    },
    ShellSnapshot {
        desc: "Experimental shell snapshotting.",
        key: "shell_snapshot",
        stage: Stage::Beta {
            name: "Shell snapshot",
            menu_description: "Snapshot your shell environment to avoid re-running login scripts for every command.",
            announcement: "NEW! Try shell snapshotting to make your Codex faster. Enable in /experimental!",
        },
        default: false,
    },

    // Experimental
    ApplyPatchFreeform {
        desc: "Include the freeform apply_patch tool.",
        key: "apply_patch_freeform",
        stage: Stage::Experimental,
        default: false,
    },
    ExecPolicy {
        desc: "Gate the execpolicy enforcement for shell/unified exec.",
        key: "exec_policy",
        stage: Stage::Experimental,
        default: true,
    },
    WindowsSandbox {
        desc: "Enable Windows sandbox (restricted token) on Windows.",
        key: "experimental_windows_sandbox",
        stage: Stage::Experimental,
        default: false,
    },
    WindowsSandboxElevated {
        desc: "Use the elevated Windows sandbox pipeline (setup + runner).",
        key: "elevated_windows_sandbox",
        stage: Stage::Experimental,
        default: false,
    },
    RemoteCompaction {
        desc: "Remote compaction enabled (only for ChatGPT auth).",
        key: "remote_compaction",
        stage: Stage::Experimental,
        default: true,
    },
    RemoteModels {
        desc: "Refresh remote models and emit AppReady once the list is available.",
        key: "remote_models",
        stage: Stage::Experimental,
        default: false,
    },
    Skills {
        desc: "Enable discovery and injection of skills.",
        key: "skills",
        stage: Stage::Experimental,
        default: true,
    },
    PowershellUtf8 {
        desc: "Enforce UTF8 output in Powershell.",
        key: "powershell_utf8",
        stage: Stage::Experimental,
        default: false,
    },
    Tui2 {
        desc: "Experimental TUI v2 (viewport) implementation.",
        key: "tui2",
        stage: Stage::Experimental,
        default: false,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyFeatureUsage {
    pub alias: String,
    pub feature: Feature,
}

/// Holds the effective set of enabled features.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Features {
    enabled: BTreeSet<Feature>,
    legacy_usages: BTreeSet<LegacyFeatureUsage>,
}

#[derive(Debug, Clone, Default)]
pub struct FeatureOverrides {
    pub include_apply_patch_tool: Option<bool>,
    pub web_search_request: Option<bool>,
}

impl FeatureOverrides {
    fn apply(self, features: &mut Features) {
        LegacyFeatureToggles {
            include_apply_patch_tool: self.include_apply_patch_tool,
            tools_web_search: self.web_search_request,
            ..Default::default()
        }
        .apply(features);
    }
}

impl Features {
    /// Starts with built-in defaults.
    pub fn with_defaults() -> Self {
        let mut set = BTreeSet::new();
        for spec in FEATURES {
            if spec.default_enabled {
                set.insert(spec.id);
            }
        }
        Self {
            enabled: set,
            legacy_usages: BTreeSet::new(),
        }
    }

    pub fn enabled(&self, f: Feature) -> bool {
        self.enabled.contains(&f)
    }

    pub fn enable(&mut self, f: Feature) -> &mut Self {
        self.enabled.insert(f);
        self
    }

    pub fn disable(&mut self, f: Feature) -> &mut Self {
        self.enabled.remove(&f);
        self
    }

    pub fn record_legacy_usage_force(&mut self, alias: &str, feature: Feature) {
        self.legacy_usages.insert(LegacyFeatureUsage {
            alias: alias.to_string(),
            feature,
        });
    }

    pub fn record_legacy_usage(&mut self, alias: &str, feature: Feature) {
        if alias == feature.key() {
            return;
        }
        self.record_legacy_usage_force(alias, feature);
    }

    pub fn legacy_feature_usages(&self) -> impl Iterator<Item = (&str, Feature)> + '_ {
        self.legacy_usages
            .iter()
            .map(|usage| (usage.alias.as_str(), usage.feature))
    }

    /// Apply a table of key -> bool toggles (e.g. from TOML).
    pub fn apply_map(&mut self, m: &BTreeMap<String, bool>) {
        for (k, v) in m {
            match feature_for_key(k) {
                Some(feat) => {
                    if k != feat.key() {
                        self.record_legacy_usage(k.as_str(), feat);
                    }
                    if *v {
                        self.enable(feat);
                    } else {
                        self.disable(feat);
                    }
                }
                None => {
                    tracing::warn!("unknown feature key in config: {k}");
                }
            }
        }
    }

    #[allow(deprecated)]
    pub fn from_config(
        cfg: &ConfigToml,
        config_profile: &ConfigProfile,
        overrides: FeatureOverrides,
    ) -> Self {
        let mut features = Features::with_defaults();

        #[allow(deprecated)]
        let base_legacy = LegacyFeatureToggles {
            experimental_use_freeform_apply_patch: cfg.experimental_use_freeform_apply_patch,
            experimental_use_unified_exec_tool: cfg.experimental_use_unified_exec_tool,
            tools_web_search: cfg.tools.as_ref().and_then(|t| t.web_search),
            tools_view_image: cfg.tools.as_ref().and_then(|t| t.view_image),
            ..Default::default()
        };
        base_legacy.apply(&mut features);

        if let Some(base_features) = cfg.features.as_ref() {
            features.apply_map(&base_features.entries);
        }

        let profile_legacy = LegacyFeatureToggles {
            include_apply_patch_tool: config_profile.include_apply_patch_tool,
            experimental_use_freeform_apply_patch: config_profile
                .experimental_use_freeform_apply_patch,

            experimental_use_unified_exec_tool: config_profile.experimental_use_unified_exec_tool,
            tools_web_search: config_profile.tools_web_search,
            tools_view_image: config_profile.tools_view_image,
        };
        profile_legacy.apply(&mut features);
        if let Some(profile_features) = config_profile.features.as_ref() {
            features.apply_map(&profile_features.entries);
        }

        overrides.apply(&mut features);

        features
    }

    pub fn enabled_features(&self) -> Vec<Feature> {
        self.enabled.iter().copied().collect()
    }
}

/// Keys accepted in `[features]` tables.
fn feature_for_key(key: &str) -> Option<Feature> {
    for spec in FEATURES {
        if spec.key == key {
            return Some(spec.id);
        }
    }
    legacy::feature_for_key(key)
}

/// Returns `true` if the provided string matches a known feature toggle key.
pub fn is_known_feature_key(key: &str) -> bool {
    feature_for_key(key).is_some()
}

/// Deserializable features table for TOML.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "config-schema", derive(JsonSchema))]
#[cfg_attr(feature = "config-schema", schemars(extend("additionalProperties" = {"type": "boolean"})))]
pub struct FeaturesToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, bool>,
}

/// Returns all feature specs for schema generation.
#[cfg(feature = "config-schema")]
pub fn all_feature_specs() -> &'static [FeatureSpec] {
    FEATURES
}
