//! XDG directory resolution, upward `.anix/` discovery, `config.toml` schema,
//! 6-layer merge precedence, and private binary lookup (Sections 2, 3.4, 8.1, 8.2, 10.2).

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

use anix_proto::ir::{ApprovalMode, OutputMode, StderrMode};
use anix_proto::provider::ProviderMode;
use serde::Deserialize;
use thiserror::Error;

use crate::cli::{ApprovalArg, Cli, OutputArg, StderrArg};
use crate::frontmatter::{
    AgentScript, CompactThreshold, parse_approval_mode, parse_output_mode, parse_stderr_mode,
    validate_agent_name,
};
use crate::platform::{SandboxPolicy, posix};

const DEFAULT_PROVIDER: &str = "openai-responses";
const DEFAULT_SHELL: &str = "msh";
const DEFAULT_MAX_TURNS: u32 = 20;
const DEFAULT_MAX_TOKENS: u32 = 128_000;
const DEFAULT_COMPACT_THRESHOLD: CompactThreshold = CompactThreshold::Ratio(0.8);
const DEFAULT_PROVIDER_TIMEOUT_SEC: u64 = 120;
const DEFAULT_MSH_TIMEOUT_SEC: u64 = 120;
const DEFAULT_MAX_MSH_TIMEOUT_SEC: u64 = 600;
const DEFAULT_IDLE_TIMEOUT_SEC: u64 = 300;
const DEFAULT_APPROVAL_DECIDER_TIMEOUT_SEC: u64 = 30;
const DEFAULT_VISION_BRIDGE_TIMEOUT_SEC: u64 = 60;

pub const DEFAULT_SAFE_COMMANDS: &[&str] = &[
    "ls",
    "cat",
    "head",
    "tail",
    "rg",
    "grep",
    "find",
    "pwd",
    "git status",
    "git diff",
    "git log",
];

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid TOML in `{path}`: {source}")]
    Toml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("configuration error: {0}")]
    Invalid(String),
}

/// Resolved XDG & project directories.
#[derive(Debug, Clone)]
pub struct AgentPaths {
    pub home_dir: PathBuf,
    pub global_config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub libexec_dir: PathBuf,
    pub tools_dir: PathBuf,
    pub project_agent_dir: Option<PathBuf>,
    pub cwd: PathBuf,
}

impl AgentPaths {
    pub fn discover(cwd: &Path) -> Result<Self, ConfigError> {
        let home_dir = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| ConfigError::Invalid("HOME environment variable is not set".into()))?;

        let global_config_dir = env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".config"))
            .join("anix");

        let state_dir = env::var_os("XDG_STATE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".local/state"))
            .join("anix");

        let runtime_dir = match env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
            Some(dir) => PathBuf::from(dir).join("anix"),
            None => PathBuf::from(format!("/tmp/anix-{}", posix::current_uid())),
        };

        let libexec_dir = home_dir.join(".local/libexec/anix");
        let tools_dir = libexec_dir.join("tools");
        let project_agent_dir = discover_project_agent_dir(cwd, &home_dir);

        Ok(Self {
            home_dir,
            global_config_dir,
            state_dir,
            runtime_dir,
            libexec_dir,
            tools_dir,
            project_agent_dir,
            cwd: cwd.to_path_buf(),
        })
    }

    pub fn global_config_file(&self) -> PathBuf {
        self.global_config_dir.join("config.toml")
    }

    pub fn project_config_file(&self) -> Option<PathBuf> {
        self.project_agent_dir
            .as_ref()
            .map(|d| d.join("config.toml"))
    }

    pub fn trusted_registry_file(&self) -> PathBuf {
        self.state_dir.join("trusted.json")
    }

    pub fn journal_socket_path(&self) -> PathBuf {
        self.runtime_dir.join("journal.sock")
    }

    pub fn jobs_dir(&self) -> PathBuf {
        self.runtime_dir.join("jobs")
    }

    /// Resolve a component binary (`shell` or provider `bin`) per Section 2:
    /// - If `spec` contains `/`, expand `~` if leading and return directly;
    /// - Else search `$PATH` first, then fall back to `~/.local/libexec/anix/<spec>`.
    pub fn resolve_component_bin(&self, spec: &str, base_dir: Option<&Path>) -> PathBuf {
        if spec.contains('/') {
            let expanded = expand_tilde(spec, &self.home_dir);
            if expanded.is_absolute() {
                return expanded;
            }
            if let Some(base) = base_dir {
                return base.join(expanded);
            }
            return self.cwd.join(expanded);
        }

        if let Some(path_var) = env::var_os("PATH") {
            for dir in env::split_paths(&path_var) {
                let candidate = dir.join(spec);
                if posix::is_executable_file(&candidate) {
                    return candidate;
                }
            }
        }

        let libexec_candidate = self.libexec_dir.join(spec);
        if libexec_candidate.exists() {
            return libexec_candidate;
        }

        // Also check sibling binary directory (helpful when running directly from `target/debug`).
        if let Ok(exe) = env::current_exe()
            && let Some(exe_dir) = exe.parent()
        {
            let sibling = exe_dir.join(spec);
            if posix::is_executable_file(&sibling) {
                return sibling;
            }
        }

        libexec_candidate
    }

    /// Resolve tools directory for `msh` PATH prepend (`~/.local/libexec/anix/tools`,
    /// falling back to sibling dir when running uninstalled in dev if tools dir lacks binaries).
    pub fn effective_tools_dir(&self) -> PathBuf {
        if self.tools_dir.join("aclr").is_file() || self.tools_dir.join("aview").is_file() {
            return self.tools_dir.clone();
        }
        if let Ok(exe) = env::current_exe()
            && let Some(exe_dir) = exe.parent()
            && (exe_dir.join("aclr").is_file() || exe_dir.join("aview").is_file())
        {
            return exe_dir.to_path_buf();
        }
        self.tools_dir.clone()
    }
}

/// Upward project `.anix/` search stopping at first `.anix/`, `.git`, or `$HOME` (Section 8.1).
pub fn discover_project_agent_dir(cwd: &Path, home_dir: &Path) -> Option<PathBuf> {
    let canonical_home = fs::canonicalize(home_dir).unwrap_or_else(|_| home_dir.to_path_buf());
    let mut cur = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());

    loop {
        let candidate = cur.join(".anix");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if cur.join(".git").exists() || cur == canonical_home {
            return None;
        }
        let parent = cur.parent()?;
        if parent == cur {
            return None;
        }
        cur = parent.to_path_buf();
    }
}

/// Raw `config.toml` file structure (strict: rejects unknown fields per Section 6.5).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfigFile {
    #[serde(default)]
    pub defaults: RawDefaults,
    #[serde(default)]
    pub approval: RawApproval,
    #[serde(default)]
    pub vision: RawVisionConfig,
    #[serde(default)]
    pub providers: BTreeMap<String, RawProviderConfig>,
    #[serde(default)]
    pub compact: RawCompactConfig,
    #[serde(default)]
    pub sandbox: RawSandboxConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDefaults {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub vision: Option<bool>,
    pub shell: Option<String>,
    pub max_turns: Option<u32>,
    pub max_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub compact_threshold: Option<toml::Value>,
    pub approval: Option<String>,
    pub output: Option<String>,
    pub stderr: Option<String>,
    pub msh_timeout_sec: Option<u64>,
    pub max_msh_timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawApproval {
    pub safe_commands: Option<Vec<String>>,
    pub bin: Option<String>,
    pub timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVisionConfig {
    pub bin: Option<String>,
    pub timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProviderConfig {
    pub bin: Option<String>,
    pub mode: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub vision: Option<bool>,
    pub timeout_sec: Option<u64>,
    pub params: Option<toml::Table>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCompactConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub prompt_file: Option<String>,
    pub bin: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSandboxConfig {
    pub enabled: Option<bool>,
    pub strict: Option<bool>,
}

/// Load a `config.toml` file if it exists (returns default if absent).
pub fn load_config_file(path: &Path) -> Result<Option<RawConfigFile>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let parsed: RawConfigFile = toml::from_str(&content).map_err(|source| ConfigError::Toml {
        path: path.display().to_string(),
        source,
    })?;
    Ok(Some(parsed))
}

/// Effective API key resolution source for a Provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeySource {
    /// Resolved from the non-empty environment variable named by `api_key_env` (highest priority).
    Env(String),
    /// Resolved from the direct `api_key` configured in `config.toml` (fallback).
    Config,
    /// Neither `api_key_env` nor `api_key` is required (e.g. local unauthenticated endpoint).
    NoneRequired,
    /// `api_key_env` is configured and unset/empty, and no fallback `api_key` is configured.
    Missing(String),
}

/// Merged Provider configuration entry.
#[derive(Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub bin: String,
    pub mode: ProviderMode,
    pub base_url: String,
    pub api_key: Option<String>,
    pub api_key_env: String,
    pub vision: Option<bool>,
    pub timeout_sec: u64,
    pub params: BTreeMap<String, serde_json::Value>,
    /// True if `bin` was defined or overridden in project `.anix/config.toml` (requires `anix trust`).
    pub bin_from_project: bool,
}

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("name", &self.name)
            .field("bin", &self.bin)
            .field("mode", &self.mode)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("api_key_env", &self.api_key_env)
            .field("vision", &self.vision)
            .field("timeout_sec", &self.timeout_sec)
            .field("params", &self.params)
            .field("bin_from_project", &self.bin_from_project)
            .finish()
    }
}

impl ProviderConfig {
    /// Resolve the effective API key with ENV taking precedence over direct `config.toml` `api_key`:
    /// 1. Non-empty environment variable named by `api_key_env` (highest priority).
    /// 2. Non-empty `api_key` directly configured in `config.toml` (fallback).
    /// 3. Empty string if neither is present.
    pub fn resolve_api_key(&self) -> (String, ApiKeySource) {
        if !self.api_key_env.is_empty()
            && let Ok(val) = env::var(&self.api_key_env)
            && !val.trim().is_empty()
        {
            return (val, ApiKeySource::Env(self.api_key_env.clone()));
        }
        if let Some(ref key) = self.api_key
            && !key.trim().is_empty()
        {
            return (key.clone(), ApiKeySource::Config);
        }
        if !self.api_key_env.is_empty() {
            (
                String::new(),
                ApiKeySource::Missing(self.api_key_env.clone()),
            )
        } else {
            (String::new(), ApiKeySource::NoneRequired)
        }
    }
}

/// Merged `[approval]` external decider configuration (v1.1 §5.2).
#[derive(Debug, Clone, Default)]
pub struct ApprovalDeciderConfig {
    pub bin: Option<String>,
    pub timeout_sec: u64,
    /// True if `bin` came from project `.anix/config.toml` (requires `anix trust`).
    pub bin_from_project: bool,
}

/// Merged `[vision]` external image-to-text bridge configuration (v1.1 §5.5).
#[derive(Debug, Clone, Default)]
pub struct VisionBridgeConfig {
    pub bin: Option<String>,
    pub timeout_sec: u64,
    /// True if `bin` came from project `.anix/config.toml` (requires `anix trust`).
    pub bin_from_project: bool,
}

/// Merged `[compact]` configuration.
#[derive(Debug, Clone, Default)]
pub struct CompactConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub prompt_file: Option<PathBuf>,
    pub bin: Option<String>,
    /// True if `bin` came from project `.anix/config.toml` (requires `anix trust`).
    pub bin_from_project: bool,
}

/// Fully merged, validated runtime configuration for an `anix` invocation.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub paths: AgentPaths,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub vision: bool,
    pub shell: String,
    pub max_turns: u32,
    pub max_tokens: u32,
    pub max_output_tokens: Option<u32>,
    pub compact_threshold: CompactThreshold,
    pub no_compact: bool,
    pub approval: ApprovalMode,
    pub approval_decider: ApprovalDeciderConfig,
    pub vision_bridge: VisionBridgeConfig,
    pub fail_on_deny: bool,
    pub output: OutputMode,
    pub stderr: StderrMode,
    pub trace_fd: Option<RawFd>,
    pub context: Option<PathBuf>,
    pub inbox: Option<PathBuf>,
    pub idle_timeout_sec: u64,
    pub params: BTreeMap<String, serde_json::Value>,
    pub msh_timeout_sec: u64,
    pub max_msh_timeout_sec: u64,
    pub sandbox: SandboxPolicy,
    pub safe_commands: Vec<String>,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub compact: CompactConfig,
}

impl ResolvedConfig {
    /// Merge all 6 layers strictly in Section 3.4 order:
    /// `CLI > Env > Frontmatter > Project .anix/config.toml > Global config.toml > Built-in defaults`.
    pub fn resolve(
        cli: &Cli,
        script: &AgentScript,
        paths: AgentPaths,
    ) -> Result<Self, ConfigError> {
        let global = load_config_file(&paths.global_config_file())?.unwrap_or_default();
        let project = match paths.project_config_file() {
            Some(p) => load_config_file(&p)?.unwrap_or_default(),
            None => RawConfigFile::default(),
        };

        // 1. Merge providers map (built-in default -> global -> project)
        let mut providers = default_providers();
        merge_providers(&mut providers, &global.providers, false)?;
        merge_providers(&mut providers, &project.providers, true)?;

        // 2. Merge compact, approval decider, and vision bridge configs (global -> project)
        let compact = merge_compact(&global.compact, &project.compact, &paths.home_dir)?;
        let approval_decider = merge_approval_decider(&global.approval, &project.approval);
        let vision_bridge = merge_vision_bridge(&global.vision, &project.vision);

        // 3. Merge safe_commands (built-in -> global -> project)
        let safe_commands = project
            .approval
            .safe_commands
            .or(global.approval.safe_commands)
            .unwrap_or_else(|| {
                DEFAULT_SAFE_COMMANDS
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect()
            });

        // 4. Scalar fields following exact 6-layer precedence
        let fm = &script.frontmatter;
        let name = fm
            .name
            .clone()
            .unwrap_or_else(|| script.default_name.clone());
        validate_agent_name(&name).map_err(|e| ConfigError::Invalid(e.to_string()))?;

        let provider = cli
            .provider
            .clone()
            .or_else(|| env_non_empty("ANIX_PROVIDER"))
            .or_else(|| fm.provider.clone())
            .or_else(|| project.defaults.provider.clone())
            .or_else(|| global.defaults.provider.clone())
            .unwrap_or_else(|| DEFAULT_PROVIDER.to_owned());

        if !providers.contains_key(&provider) {
            return Err(ConfigError::Invalid(format!(
                "provider `{provider}` is not defined in [providers.{provider}]"
            )));
        }

        let model = cli
            .model
            .clone()
            .or_else(|| env_non_empty("ANIX_MODEL"))
            .or_else(|| fm.model.clone())
            .or_else(|| project.defaults.model.clone())
            .or_else(|| global.defaults.model.clone())
            .unwrap_or_default();
        if model.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "model must be configured via CLI, ANIX_MODEL, frontmatter, or config.toml".into(),
            ));
        }

        let shell = cli
            .shell
            .clone()
            .or_else(|| env_non_empty("ANIX_SHELL"))
            .or_else(|| fm.shell.clone())
            .or_else(|| project.defaults.shell.clone())
            .or_else(|| global.defaults.shell.clone())
            .unwrap_or_else(|| DEFAULT_SHELL.to_owned());

        let max_turns = match cli.max_turns {
            Some(v) => v,
            None => match env_non_empty("ANIX_MAX_TURNS") {
                Some(s) => s
                    .parse::<u32>()
                    .map_err(|_| ConfigError::Invalid(format!("invalid ANIX_MAX_TURNS `{s}`")))?,
                None => fm
                    .max_turns
                    .or(project.defaults.max_turns)
                    .or(global.defaults.max_turns)
                    .unwrap_or(DEFAULT_MAX_TURNS),
            },
        };
        if max_turns < 1 {
            return Err(ConfigError::Invalid("max_turns must be >= 1".into()));
        }

        let max_tokens = match cli.max_tokens {
            Some(v) => v,
            None => match env_non_empty("ANIX_MAX_TOKENS") {
                Some(s) => s
                    .parse::<u32>()
                    .map_err(|_| ConfigError::Invalid(format!("invalid ANIX_MAX_TOKENS `{s}`")))?,
                None => fm
                    .max_tokens
                    .or(project.defaults.max_tokens)
                    .or(global.defaults.max_tokens)
                    .unwrap_or(DEFAULT_MAX_TOKENS),
            },
        };
        if max_tokens < 1024 {
            return Err(ConfigError::Invalid("max_tokens must be >= 1024".into()));
        }

        let max_output_tokens_raw = match cli.max_output_tokens {
            Some(v) => v,
            None => match env_non_empty("ANIX_MAX_OUTPUT_TOKENS") {
                Some(s) => s.parse::<u32>().map_err(|_| {
                    ConfigError::Invalid(format!("invalid ANIX_MAX_OUTPUT_TOKENS `{s}`"))
                })?,
                None => fm
                    .max_output_tokens
                    .or(project.defaults.max_output_tokens)
                    .or(global.defaults.max_output_tokens)
                    .unwrap_or(0),
            },
        };
        let max_output_tokens = if max_output_tokens_raw == 0 {
            None
        } else {
            if max_output_tokens_raw >= max_tokens {
                return Err(ConfigError::Invalid(format!(
                    "max_output_tokens ({max_output_tokens_raw}) must be < max_tokens ({max_tokens})"
                )));
            }
            Some(max_output_tokens_raw)
        };

        // Shallow merge `params`: CLI `--param` > frontmatter `params` > project/global `[providers.<name>.params]` (Section 3.4).
        let mut params = providers
            .get(&provider)
            .map(|p| p.params.clone())
            .unwrap_or_default();
        for (k, v) in &fm.params {
            params.insert(k.clone(), v.clone());
        }
        for raw_param in &cli.params {
            let (k, v) = parse_cli_param(raw_param)?;
            params.insert(k, v);
        }

        let inbox = cli
            .inbox
            .clone()
            .or_else(|| env_non_empty("ANIX_INBOX").map(PathBuf::from))
            .map(|p| {
                let expanded = expand_tilde(&p.display().to_string(), &paths.home_dir);
                if expanded.is_absolute() {
                    expanded
                } else {
                    paths.cwd.join(expanded)
                }
            });

        let idle_timeout_sec = match cli.idle_timeout {
            Some(v) => v,
            None => match env_non_empty("ANIX_IDLE_TIMEOUT") {
                Some(s) => s.parse::<u64>().map_err(|_| {
                    ConfigError::Invalid(format!("invalid ANIX_IDLE_TIMEOUT `{s}`"))
                })?,
                None => DEFAULT_IDLE_TIMEOUT_SEC,
            },
        };

        let compact_threshold = if cli.no_compact {
            CompactThreshold::Disabled
        } else if let Some(raw) = &cli.compact_threshold {
            CompactThreshold::parse_str(raw).map_err(ConfigError::Invalid)?
        } else if let Some(raw) = env_non_empty("ANIX_COMPACT_THRESHOLD") {
            CompactThreshold::parse_str(&raw).map_err(ConfigError::Invalid)?
        } else if let Some(t) = fm.compact_threshold {
            t
        } else if let Some(ref v) = project.defaults.compact_threshold {
            parse_toml_compact_threshold(v)?
        } else if let Some(ref v) = global.defaults.compact_threshold {
            parse_toml_compact_threshold(v)?
        } else {
            DEFAULT_COMPACT_THRESHOLD
        };

        let approval = resolve_approval(cli, fm.approval, &project.defaults, &global.defaults)?;
        let output = resolve_output(cli, fm.output, &project.defaults, &global.defaults)?;
        let stderr = resolve_stderr(cli, fm.stderr, &project.defaults, &global.defaults)?;
        let active_provider_vision = providers.get(&provider).and_then(|p| p.vision);
        let vision = resolve_vision(
            cli,
            fm.vision,
            active_provider_vision,
            project.defaults.vision,
            global.defaults.vision,
        )?;
        let trace_fd = resolve_trace_fd(cli.trace_fd)?;

        // Empty-string override rule (Section 3.4):
        // CLI `-c ""` or `ANIX_CONTEXT=""` (set and empty) overrides frontmatter `context`!
        let context_raw: Option<String> = if let Some(ref c) = cli.context {
            if c.is_empty() { None } else { Some(c.clone()) }
        } else if let Some(env_c) = env::var_os("ANIX_CONTEXT") {
            let s = env_c.to_string_lossy().into_owned();
            if s.is_empty() { None } else { Some(s) }
        } else {
            fm.context.as_ref().filter(|s| !s.is_empty()).cloned()
        };

        let context = context_raw.map(|raw| {
            let expanded = expand_tilde(&raw, &paths.home_dir);
            if expanded.is_absolute() {
                expanded
            } else {
                paths.cwd.join(expanded)
            }
        });

        let msh_timeout_sec = env_non_empty("MSH_TIMEOUT_SEC")
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&v| v > 0)
            .or(project.defaults.msh_timeout_sec)
            .or(global.defaults.msh_timeout_sec)
            .unwrap_or(DEFAULT_MSH_TIMEOUT_SEC);

        let max_msh_timeout_sec = env_non_empty("MSH_MAX_TIMEOUT_SEC")
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&v| v > 0)
            .or(project.defaults.max_msh_timeout_sec)
            .or(global.defaults.max_msh_timeout_sec)
            .unwrap_or(DEFAULT_MAX_MSH_TIMEOUT_SEC)
            .max(msh_timeout_sec);

        let sandbox = resolve_sandbox(cli.sandbox.as_deref(), &project.sandbox, &global.sandbox)?;

        // v1.2.0 §3.4: Startup Fail-Fast validation across all merged configuration layers
        // BEFORE WAL `session_init` is ever written (invalid combinations exit 2).
        let shell_enabled = !shell.eq_ignore_ascii_case("none");
        if output == OutputMode::Explicit {
            if !shell_enabled {
                return Err(ConfigError::Invalid(
                    "output=explicit requires an executable shell (cannot combine with shell=none)"
                        .into(),
                ));
            }
            if approval == ApprovalMode::Deny {
                return Err(ConfigError::Invalid(
                    "output=explicit cannot be combined with approval=deny (shell execution is required to write $ANIX_OUT)".into(),
                ));
            }
        }
        if shell_enabled {
            let base_dir = paths.project_agent_dir.as_deref().and_then(|d| d.parent());
            let resolved_shell = paths.resolve_component_bin(&shell, base_dir);
            if !posix::is_executable_file(&resolved_shell) {
                return Err(ConfigError::Invalid(format!(
                    "shell executable `{shell}` not found or not executable (resolved `{}`)",
                    resolved_shell.display()
                )));
            }
        }

        Ok(Self {
            paths,
            name,
            provider,
            model,
            vision,
            shell,
            max_turns,
            max_tokens,
            max_output_tokens,
            compact_threshold,
            no_compact: cli.no_compact,
            approval,
            approval_decider,
            vision_bridge,
            fail_on_deny: cli.fail_on_deny,
            output,
            stderr,
            trace_fd,
            context,
            inbox,
            idle_timeout_sec,
            params,
            msh_timeout_sec,
            max_msh_timeout_sec,
            sandbox,
            safe_commands,
            providers,
            compact,
        })
    }

    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        self.providers.get(&self.provider)
    }

    pub fn shell_enabled(&self) -> bool {
        !self.shell.eq_ignore_ascii_case("none")
    }

    /// Non-blocking startup diagnostics for suspicious/migrating v1.2.0 combinations.
    pub fn startup_warnings(&self, prompt: &str) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.output == OutputMode::Reply
            && self.shell_enabled()
            && self.approval == ApprovalMode::Deny
        {
            warnings.push(
                "output=reply with shell enabled and approval=deny blocks shell commands; consider `shell: none` for pure-text agents".into(),
            );
        }
        if self.output == OutputMode::None && !self.shell_enabled() {
            warnings.push(
                "output=none with shell=none produces neither stdout output nor shell side effects"
                    .into(),
            );
        }
        if self.output != OutputMode::Explicit && prompt.contains("ANIX_OUT") {
            warnings.push(format!(
                "script prompt references ANIX_OUT while output={}; $ANIX_OUT is only mounted when output=explicit (pass `-o explicit` or remove ANIX_OUT from prompt)",
                self.output.as_str()
            ));
        }
        warnings
    }

    pub fn effective_compact_threshold(&self) -> Option<u64> {
        if self.no_compact {
            return None;
        }
        self.compact_threshold.resolve(u64::from(self.max_tokens))
    }
}

pub fn default_providers() -> BTreeMap<String, ProviderConfig> {
    let mut map = BTreeMap::new();
    map.insert(
        DEFAULT_PROVIDER.into(),
        ProviderConfig {
            name: DEFAULT_PROVIDER.into(),
            bin: "anix-provider-openai-responses".into(),
            mode: ProviderMode::Coprocess,
            base_url: "https://api.openai.com/v1".into(),
            api_key: None,
            api_key_env: "OPENAI_API_KEY".into(),
            vision: None,
            timeout_sec: DEFAULT_PROVIDER_TIMEOUT_SEC,
            params: BTreeMap::new(),
            bin_from_project: false,
        },
    );
    map
}

pub(crate) fn merge_providers(
    target: &mut BTreeMap<String, ProviderConfig>,
    layer: &BTreeMap<String, RawProviderConfig>,
    is_project: bool,
) -> Result<(), ConfigError> {
    for (name, raw) in layer {
        let entry = target
            .entry(name.clone())
            .or_insert_with(|| ProviderConfig {
                name: name.clone(),
                bin: format!("anix-provider-{name}"),
                mode: ProviderMode::Coprocess,
                base_url: String::new(),
                api_key: None,
                api_key_env: String::new(),
                vision: None,
                timeout_sec: DEFAULT_PROVIDER_TIMEOUT_SEC,
                params: BTreeMap::new(),
                bin_from_project: false,
            });
        if let Some(ref bin) = raw.bin {
            entry.bin = bin.clone();
            if is_project {
                entry.bin_from_project = true;
            }
        }
        if let Some(ref mode_raw) = raw.mode {
            entry.mode = ProviderMode::parse_str(mode_raw).map_err(|reason| {
                ConfigError::Invalid(format!("invalid [providers.{name}].mode: {reason}"))
            })?;
        }
        if let Some(ref base_url) = raw.base_url {
            entry.base_url = base_url.clone();
        }
        if let Some(ref api_key) = raw.api_key {
            entry.api_key = if api_key.trim().is_empty() {
                None
            } else {
                Some(api_key.clone())
            };
        }
        if let Some(ref api_key_env) = raw.api_key_env {
            entry.api_key_env = api_key_env.clone();
        }
        if let Some(vision) = raw.vision {
            entry.vision = Some(vision);
        }
        if let Some(timeout) = raw.timeout_sec {
            entry.timeout_sec = timeout;
        }
        if let Some(ref table) = raw.params
            && let Ok(serde_json::Value::Object(map)) = serde_json::to_value(table)
        {
            for (k, v) in map {
                entry.params.insert(k, v);
            }
        }
    }
    Ok(())
}

/// Parse `--param KEY=VALUE` (value parsed as JSON literal first, else raw string, Section 3.1).
fn parse_cli_param(raw: &str) -> Result<(String, serde_json::Value), ConfigError> {
    let Some((key, val_str)) = raw.split_once('=') else {
        return Err(ConfigError::Invalid(format!(
            "invalid --param `{raw}` (expected KEY=VALUE)"
        )));
    };
    let key = key.trim();
    if key.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "invalid --param `{raw}` (key must not be empty)"
        )));
    }
    let value = serde_json::from_str::<serde_json::Value>(val_str)
        .unwrap_or_else(|_| serde_json::Value::String(val_str.to_owned()));
    Ok((key.to_owned(), value))
}

fn merge_compact(
    global: &RawCompactConfig,
    project: &RawCompactConfig,
    home_dir: &Path,
) -> Result<CompactConfig, ConfigError> {
    let provider = project.provider.clone().or_else(|| global.provider.clone());
    let model = project.model.clone().or_else(|| global.model.clone());

    // prompt and prompt_file are mutually exclusive within the winning layer.
    let (prompt, prompt_file) = if project.prompt.is_some() && project.prompt_file.is_some() {
        return Err(ConfigError::Invalid(
            "[compact].prompt and [compact].prompt_file are mutually exclusive".into(),
        ));
    } else if project.prompt.is_some() || project.prompt_file.is_some() {
        (
            project.prompt.clone(),
            project
                .prompt_file
                .as_deref()
                .map(|p| expand_tilde(p, home_dir)),
        )
    } else if global.prompt.is_some() && global.prompt_file.is_some() {
        return Err(ConfigError::Invalid(
            "[compact].prompt and [compact].prompt_file are mutually exclusive".into(),
        ));
    } else {
        (
            global.prompt.clone(),
            global
                .prompt_file
                .as_deref()
                .map(|p| expand_tilde(p, home_dir)),
        )
    };

    let (bin, bin_from_project) = match project.bin.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(b) => (Some(b.to_owned()), true),
        None => (
            global
                .bin
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(ToOwned::to_owned),
            false,
        ),
    };

    Ok(CompactConfig {
        provider,
        model,
        prompt,
        prompt_file,
        bin,
        bin_from_project,
    })
}

fn merge_approval_decider(global: &RawApproval, project: &RawApproval) -> ApprovalDeciderConfig {
    let (bin, bin_from_project) = match project.bin.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(b) => (Some(b.to_owned()), true),
        None => (
            global
                .bin
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(ToOwned::to_owned),
            false,
        ),
    };
    let timeout_sec = project
        .timeout_sec
        .or(global.timeout_sec)
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_APPROVAL_DECIDER_TIMEOUT_SEC);
    ApprovalDeciderConfig {
        bin,
        timeout_sec,
        bin_from_project,
    }
}

fn merge_vision_bridge(global: &RawVisionConfig, project: &RawVisionConfig) -> VisionBridgeConfig {
    let (bin, bin_from_project) = match project.bin.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(b) => (Some(b.to_owned()), true),
        None => (
            global
                .bin
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(ToOwned::to_owned),
            false,
        ),
    };
    let timeout_sec = project
        .timeout_sec
        .or(global.timeout_sec)
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_VISION_BRIDGE_TIMEOUT_SEC);
    VisionBridgeConfig {
        bin,
        timeout_sec,
        bin_from_project,
    }
}

fn resolve_vision(
    cli: &Cli,
    fm_vision: Option<bool>,
    provider_vision: Option<bool>,
    project_default: Option<bool>,
    global_default: Option<bool>,
) -> Result<bool, ConfigError> {
    if cli.vision {
        return Ok(true);
    }
    if cli.no_vision {
        return Ok(false);
    }
    if let Some(raw) = env_non_empty("ANIX_VISION") {
        return match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" => Ok(true),
            "0" | "false" => Ok(false),
            other => Err(ConfigError::Invalid(format!(
                "invalid ANIX_VISION `{other}` (expected `1`, `true`, `0`, or `false`)"
            ))),
        };
    }
    if let Some(v) = fm_vision {
        return Ok(v);
    }
    if let Some(v) = provider_vision {
        return Ok(v);
    }
    if let Some(v) = project_default {
        return Ok(v);
    }
    if let Some(v) = global_default {
        return Ok(v);
    }
    Ok(true)
}

fn resolve_trace_fd(cli_fd: Option<i32>) -> Result<Option<RawFd>, ConfigError> {
    let fd = match cli_fd {
        Some(n) => Some(n),
        None => match env_non_empty("ANIX_TRACE_FD") {
            Some(raw) => Some(raw.trim().parse::<i32>().map_err(|_| {
                ConfigError::Invalid(format!(
                    "invalid ANIX_TRACE_FD `{raw}` (expected integer >= 3)"
                ))
            })?),
            None => None,
        },
    };
    if let Some(n) = fd
        && n < 3
    {
        return Err(ConfigError::Invalid(format!(
            "invalid trace fd `{n}` (must be >= 3)"
        )));
    }
    Ok(fd)
}

fn resolve_approval(
    cli: &Cli,
    fm: Option<ApprovalMode>,
    project: &RawDefaults,
    global: &RawDefaults,
) -> Result<ApprovalMode, ConfigError> {
    if cli.yes {
        return Ok(ApprovalMode::Auto);
    }
    if let Some(arg) = cli.approval {
        return Ok(match arg {
            ApprovalArg::Ask => ApprovalMode::Ask,
            ApprovalArg::Auto => ApprovalMode::Auto,
            ApprovalArg::Deny => ApprovalMode::Deny,
        });
    }
    if env_truthy("ANIX_YES") {
        return Ok(ApprovalMode::Auto);
    }
    if let Some(raw) = env_non_empty("ANIX_APPROVAL") {
        return parse_approval_mode(&raw).map_err(ConfigError::Invalid);
    }
    if let Some(m) = fm {
        return Ok(m);
    }
    if let Some(ref raw) = project.approval {
        return parse_approval_mode(raw).map_err(ConfigError::Invalid);
    }
    if let Some(ref raw) = global.approval {
        return parse_approval_mode(raw).map_err(ConfigError::Invalid);
    }
    Ok(ApprovalMode::Ask)
}

fn resolve_output(
    cli: &Cli,
    fm: Option<OutputMode>,
    project: &RawDefaults,
    global: &RawDefaults,
) -> Result<OutputMode, ConfigError> {
    if let Some(arg) = cli.output {
        return Ok(match arg {
            OutputArg::Reply => OutputMode::Reply,
            OutputArg::Explicit => OutputMode::Explicit,
            OutputArg::None => OutputMode::None,
        });
    }
    if let Some(raw) = env_non_empty("ANIX_OUTPUT") {
        return parse_output_mode(&raw).map_err(ConfigError::Invalid);
    }
    if let Some(m) = fm {
        return Ok(m);
    }
    if let Some(ref raw) = project.output {
        return parse_output_mode(raw).map_err(ConfigError::Invalid);
    }
    if let Some(ref raw) = global.output {
        return parse_output_mode(raw).map_err(ConfigError::Invalid);
    }
    Ok(OutputMode::Reply)
}

fn resolve_stderr(
    cli: &Cli,
    fm: Option<StderrMode>,
    project: &RawDefaults,
    global: &RawDefaults,
) -> Result<StderrMode, ConfigError> {
    if cli.quiet {
        return Ok(StderrMode::Explicit);
    }
    if let Some(arg) = cli.stderr {
        return Ok(match arg {
            StderrArg::Auto => StderrMode::Auto,
            StderrArg::Explicit => StderrMode::Explicit,
        });
    }
    if let Some(raw) = env_non_empty("ANIX_STDERR") {
        return parse_stderr_mode(&raw).map_err(ConfigError::Invalid);
    }
    if let Some(m) = fm {
        return Ok(m);
    }
    if let Some(ref raw) = project.stderr {
        return parse_stderr_mode(raw).map_err(ConfigError::Invalid);
    }
    if let Some(ref raw) = global.stderr {
        return parse_stderr_mode(raw).map_err(ConfigError::Invalid);
    }
    Ok(StderrMode::Auto)
}

fn resolve_sandbox(
    cli_sandbox: Option<&str>,
    project: &RawSandboxConfig,
    global: &RawSandboxConfig,
) -> Result<SandboxPolicy, ConfigError> {
    if let Some(mode) = cli_sandbox {
        return match mode {
            "strict" => Ok(SandboxPolicy::Strict),
            "best-effort" | "auto" | "true" | "1" | "" => Ok(SandboxPolicy::BestEffort),
            "off" | "false" | "0" => Ok(SandboxPolicy::Off),
            other => Err(ConfigError::Invalid(format!(
                "invalid --sandbox value `{other}` (expected `strict`, `best-effort`, or `off`)"
            ))),
        };
    }
    let enabled = project.enabled.or(global.enabled).unwrap_or(false);
    let strict = project.strict.or(global.strict).unwrap_or(false);
    if !enabled {
        Ok(SandboxPolicy::Off)
    } else if strict {
        Ok(SandboxPolicy::Strict)
    } else {
        Ok(SandboxPolicy::BestEffort)
    }
}

fn parse_toml_compact_threshold(v: &toml::Value) -> Result<CompactThreshold, ConfigError> {
    match v {
        toml::Value::Integer(i) if *i >= 0 => Ok(if *i == 0 {
            CompactThreshold::Disabled
        } else {
            CompactThreshold::Tokens(*i as u64)
        }),
        toml::Value::Float(f) => {
            if !f.is_finite() || *f < 0.0 {
                return Err(ConfigError::Invalid(format!(
                    "invalid compact_threshold `{f}`"
                )));
            }
            if *f == 0.0 {
                Ok(CompactThreshold::Disabled)
            } else if *f > 0.0 && *f < 1.0 {
                Ok(CompactThreshold::Ratio(*f))
            } else if *f >= 1.0 && f.fract() == 0.0 {
                Ok(CompactThreshold::Tokens(*f as u64))
            } else {
                Err(ConfigError::Invalid(
                    "compact_threshold must be 0, a float in (0.0, 1.0), or an integer >= 1".into(),
                ))
            }
        }
        other => Err(ConfigError::Invalid(format!(
            "invalid compact_threshold `{other}` in config.toml"
        ))),
    }
}

fn env_non_empty(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}

fn env_truthy(key: &str) -> bool {
    env::var(key)
        .ok()
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

pub fn expand_tilde(path: &str, home_dir: &Path) -> PathBuf {
    if path == "~" {
        return home_dir.to_path_buf();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir.join(rest);
    }
    PathBuf::from(path)
}
