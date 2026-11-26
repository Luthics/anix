//! XDG directory resolution, single explicit `config.toml` loading, 4-layer merge
//! precedence (`CLI > Env > Frontmatter > explicit config > built-in defaults`),
//! shell wrapper chain parsing, and private binary lookup (Sections 2, 3.4 & 7.1).

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

use anix_proto::ir::{OutputMode, StderrMode};
use anix_proto::provider::ProviderMode;
use serde::Deserialize;
use thiserror::Error;

use crate::cli::{Cli, OutputArg, StderrArg};
use crate::frontmatter::{
    AgentScript, CompactThreshold, parse_output_mode, parse_stderr_mode, validate_agent_name,
    validate_tool_name,
};
use crate::platform::{SandboxPolicy, posix};

const DEFAULT_PROVIDER: &str = "openai-responses";
const DEFAULT_SHELL: &str = "msh";
const DEFAULT_TOOL_NAME: &str = "shell";
const DEFAULT_MAX_TURNS: u32 = 20;
const DEFAULT_MAX_TOKENS: u32 = 128_000;
const DEFAULT_COMPACT_THRESHOLD: CompactThreshold = CompactThreshold::Ratio(0.8);
const DEFAULT_PROVIDER_TIMEOUT_SEC: u64 = 120;
const DEFAULT_IDLE_TIMEOUT_SEC: u64 = 300;
const DEFAULT_INBOX_DEBOUNCE_MS: u64 = 100;

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

/// Resolved XDG directories (no implicit upward `.anix/` traversal in v0.4.0).
#[derive(Debug, Clone)]
pub struct AgentPaths {
    pub home_dir: PathBuf,
    pub global_config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub libexec_dir: PathBuf,
    pub tools_dir: PathBuf,
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

        Ok(Self {
            home_dir,
            global_config_dir,
            state_dir,
            runtime_dir,
            libexec_dir,
            tools_dir,
            cwd: cwd.to_path_buf(),
        })
    }

    pub fn default_config_file(&self) -> PathBuf {
        self.global_config_dir.join("config.toml")
    }

    /// Resolve the single explicit config file (`--config` > `$ANIX_CONFIG` > `~/.config/anix/config.toml`).
    /// Returns `(path, must_exist)` where `must_exist` is `true` when explicitly provided.
    pub fn resolve_config_file(&self, cli_config: Option<&Path>) -> (PathBuf, bool) {
        if let Some(p) = cli_config {
            return (self.resolve_user_path(&p.display().to_string()), true);
        }
        if let Some(env_p) = env_non_empty("ANIX_CONFIG") {
            return (self.resolve_user_path(&env_p), true);
        }
        (self.default_config_file(), false)
    }

    /// Resolve `hooks/` root directory (`$ANIX_HOOKS_DIR` > `<config_file_dir>/hooks`).
    pub fn resolve_hooks_dir(&self, config_file: &Path) -> PathBuf {
        if let Some(env_h) = env_non_empty("ANIX_HOOKS_DIR") {
            return self.resolve_user_path(&env_h);
        }
        config_file
            .parent()
            .unwrap_or(&self.global_config_dir)
            .join("hooks")
    }

    pub fn resolve_user_path(&self, raw: &str) -> PathBuf {
        let expanded = expand_tilde(raw, &self.home_dir);
        if expanded.is_absolute() {
            expanded
        } else {
            self.cwd.join(expanded)
        }
    }

    pub fn journal_socket_path(&self) -> PathBuf {
        self.runtime_dir.join("journal.sock")
    }

    /// Resolve a component binary (`shell` program, provider `bin`, or `[compact].bin`) per Section 2:
    /// - If `spec` contains `/`, expand leading `~/` and resolve relative paths against `cwd`;
    /// - Else search `$PATH` first, then `$HOME/.local/libexec/anix/<spec>`, then sibling of current exe.
    pub fn resolve_component_bin(&self, spec: &str) -> PathBuf {
        if spec.contains('/') {
            return self.resolve_user_path(spec);
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
        if posix::is_executable_file(&libexec_candidate) {
            return libexec_candidate;
        }

        // Dev fallback when running uninstalled directly from `target/debug` or `target/release`.
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

    /// Resolve tools directory for shell child `PATH` prepend (`~/.local/libexec/anix/tools`,
    /// falling back to sibling dir when running uninstalled in dev).
    pub fn effective_tools_dir(&self) -> PathBuf {
        if self.tools_dir.is_dir()
            && fs::read_dir(&self.tools_dir)
                .map(|mut rd| rd.next().is_some())
                .unwrap_or(false)
        {
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

/// Raw `config.toml` file structure (strict: rejects unknown fields per Section 7.1).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfigFile {
    #[serde(default)]
    pub defaults: RawDefaults,
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
    pub shell: Option<String>,
    pub tool_name: Option<String>,
    pub max_turns: Option<u32>,
    pub max_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub compact_threshold: Option<toml::Value>,
    pub output: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProviderConfig {
    pub bin: Option<String>,
    pub mode: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
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

/// Load a `config.toml` file. If `must_exist` is false and the file is absent, returns `Ok(None)`.
pub fn load_config_file(
    path: &Path,
    must_exist: bool,
) -> Result<Option<RawConfigFile>, ConfigError> {
    if !path.exists() {
        if must_exist {
            return Err(ConfigError::Invalid(format!(
                "explicit config file `{}` does not exist",
                path.display()
            )));
        }
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
    Env(String),
    Config,
    NoneRequired,
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
    pub timeout_sec: u64,
    pub params: BTreeMap<String, serde_json::Value>,
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
            .field("timeout_sec", &self.timeout_sec)
            .field("params", &self.params)
            .finish()
    }
}

impl ProviderConfig {
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

/// Merged `[compact]` configuration.
#[derive(Debug, Clone, Default)]
pub struct CompactConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub prompt_file: Option<PathBuf>,
    pub bin: Option<String>,
}

/// Fully merged, validated runtime configuration for an `anix` invocation (v0.4.0).
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub paths: AgentPaths,
    pub config_file: PathBuf,
    pub hooks_dir: PathBuf,
    pub name: String,
    pub provider: String,
    pub model: String,
    /// Raw shell specification (e.g. `"msh"`, `"anix-approve -- msh"`, or `"none"`).
    pub shell: String,
    /// First token of `shell` when `shell != "none"`.
    pub shell_program: String,
    /// Remaining prefix arguments before `-c <CMD>` when `shell` is a wrapper chain.
    pub shell_prefix_args: Vec<String>,
    /// Exposed tool name in Canonical IR (default: `"shell"`).
    pub tool_name: String,
    pub max_turns: u32,
    pub max_tokens: u32,
    pub max_output_tokens: Option<u32>,
    pub compact_threshold: CompactThreshold,
    pub no_compact: bool,
    pub output: OutputMode,
    pub stderr: StderrMode,
    pub trace_fd: Option<RawFd>,
    pub context: Option<PathBuf>,
    pub leaf: Option<u64>,
    /// `None` = non-blocking flock; `Some(None)` = infinite `--lock-wait`; `Some(Some(sec))` = bounded wait.
    pub lock_wait: Option<Option<u64>>,
    pub inbox: Option<PathBuf>,
    pub idle_timeout_sec: u64,
    pub inbox_debounce_ms: u64,
    pub notify: Option<PathBuf>,
    pub notify_id: Option<String>,
    pub params: BTreeMap<String, serde_json::Value>,
    pub sandbox: SandboxPolicy,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub compact: CompactConfig,
}

impl ResolvedConfig {
    /// Merge all 4 explicit layers strictly in RFC §3.4 order:
    /// `CLI > Env > Frontmatter > single explicit config.toml > Built-in defaults`.
    pub fn resolve(
        cli: &Cli,
        script: &AgentScript,
        paths: AgentPaths,
    ) -> Result<Self, ConfigError> {
        let (config_file, must_exist) = paths.resolve_config_file(cli.config.as_deref());
        let hooks_dir = paths.resolve_hooks_dir(&config_file);
        let file_cfg = load_config_file(&config_file, must_exist)?.unwrap_or_default();

        // 1. Merge providers map (built-in default -> explicit config)
        let mut providers = default_providers();
        merge_providers(&mut providers, &file_cfg.providers)?;

        // 2. Compact config
        let compact = parse_compact(&file_cfg.compact, &paths.home_dir)?;

        // 3. Scalar fields following exact 4-layer precedence
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
            .or_else(|| file_cfg.defaults.provider.clone())
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
            .or_else(|| file_cfg.defaults.model.clone())
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
            .or_else(|| file_cfg.defaults.shell.clone())
            .unwrap_or_else(|| DEFAULT_SHELL.to_owned());

        let tool_name = fm
            .tool_name
            .clone()
            .or_else(|| file_cfg.defaults.tool_name.clone())
            .unwrap_or_else(|| DEFAULT_TOOL_NAME.to_owned());
        validate_tool_name(&tool_name).map_err(|e| ConfigError::Invalid(e.to_string()))?;

        let max_turns = match cli.max_turns {
            Some(v) => v,
            None => match env_non_empty("ANIX_MAX_TURNS") {
                Some(s) => s
                    .parse::<u32>()
                    .map_err(|_| ConfigError::Invalid(format!("invalid ANIX_MAX_TURNS `{s}`")))?,
                None => fm
                    .max_turns
                    .or(file_cfg.defaults.max_turns)
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
                    .or(file_cfg.defaults.max_tokens)
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
                    .or(file_cfg.defaults.max_output_tokens)
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

        // Shallow merge `params`: CLI `--param` > frontmatter `params` > `[providers.<name>.params]`.
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

        // HARD RULE (RFC §3.4): `ANIX_INBOX` is a write-only child address injected by a parent
        // `--inbox` agent; `anix` MUST NEVER read `ANIX_INBOX` as its own `--inbox` configuration.
        let inbox = cli
            .inbox
            .as_ref()
            .map(|p| paths.resolve_user_path(&p.display().to_string()));

        let idle_timeout_sec = match cli.idle_timeout {
            Some(v) => v,
            None => match env_non_empty("ANIX_IDLE_TIMEOUT") {
                Some(s) => s.parse::<u64>().map_err(|_| {
                    ConfigError::Invalid(format!("invalid ANIX_IDLE_TIMEOUT `{s}`"))
                })?,
                None => DEFAULT_IDLE_TIMEOUT_SEC,
            },
        };

        let inbox_debounce_ms = cli.inbox_debounce_ms.unwrap_or(DEFAULT_INBOX_DEBOUNCE_MS);

        let notify = cli
            .notify
            .as_ref()
            .map(|p| paths.resolve_user_path(&p.display().to_string()));
        let notify_id = cli.notify_id.clone().filter(|s| !s.trim().is_empty());

        let lock_wait = match cli.lock_wait.as_deref() {
            None => None,
            Some("infinite") | Some("") => Some(None),
            Some(raw) => {
                let sec = raw.trim().parse::<u64>().map_err(|_| {
                    ConfigError::Invalid(format!(
                        "invalid --lock-wait `{raw}` (expected integer seconds >= 1 or omit value)"
                    ))
                })?;
                if sec == 0 {
                    return Err(ConfigError::Invalid(
                        "--lock-wait seconds must be >= 1".into(),
                    ));
                }
                Some(Some(sec))
            }
        };

        let compact_threshold = if cli.no_compact {
            CompactThreshold::Disabled
        } else if let Some(raw) = &cli.compact_threshold {
            CompactThreshold::parse_str(raw).map_err(ConfigError::Invalid)?
        } else if let Some(raw) = env_non_empty("ANIX_COMPACT_THRESHOLD") {
            CompactThreshold::parse_str(&raw).map_err(ConfigError::Invalid)?
        } else if let Some(t) = fm.compact_threshold {
            t
        } else if let Some(ref v) = file_cfg.defaults.compact_threshold {
            parse_toml_compact_threshold(v)?
        } else {
            DEFAULT_COMPACT_THRESHOLD
        };

        let output = resolve_output(cli, fm.output, &file_cfg.defaults)?;
        let stderr = resolve_stderr(cli, fm.stderr, &file_cfg.defaults)?;
        let trace_fd = resolve_trace_fd(cli.trace_fd)?;

        // Empty-string override rule (RFC §3.4):
        // CLI `-c ""` or `ANIX_CONTEXT=""` overrides frontmatter `context`.
        let context_raw: Option<String> = if let Some(ref c) = cli.context {
            if c.is_empty() { None } else { Some(c.clone()) }
        } else if let Some(env_c) = env::var_os("ANIX_CONTEXT") {
            let s = env_c.to_string_lossy().into_owned();
            if s.is_empty() { None } else { Some(s) }
        } else {
            fm.context.as_ref().filter(|s| !s.is_empty()).cloned()
        };

        let context = context_raw.map(|raw| paths.resolve_user_path(&raw));

        let leaf = match cli.leaf {
            Some(v) => Some(v),
            None => match env_non_empty("ANIX_LEAF") {
                Some(raw) => Some(raw.trim().parse::<u64>().map_err(|_| {
                    ConfigError::Invalid(format!(
                        "invalid ANIX_LEAF `{raw}` (expected positive integer >= 1)"
                    ))
                })?),
                None => None,
            },
        };
        if let Some(leaf_id) = leaf {
            if leaf_id == 0 {
                return Err(ConfigError::Invalid(
                    "--leaf / ANIX_LEAF must be >= 1".into(),
                ));
            }
            if context.is_none() {
                return Err(ConfigError::Invalid(
                    "--leaf / ANIX_LEAF requires a persistent WAL context file (-c / --context)"
                        .into(),
                ));
            }
        }

        let sandbox = resolve_sandbox(cli.sandbox.as_deref(), &file_cfg.sandbox)?;

        // Startup Fail-Fast validation (RFC §3.4) BEFORE WAL is opened or written.
        let shell_enabled = !shell.trim().eq_ignore_ascii_case("none");
        if output == OutputMode::Explicit && !shell_enabled {
            return Err(ConfigError::Invalid(
                "output=explicit requires an executable shell (cannot combine with shell=none)"
                    .into(),
            ));
        }

        let (shell_program, shell_prefix_args) = if shell_enabled {
            let parts = split_shell_words(&shell);
            if parts.is_empty() {
                return Err(ConfigError::Invalid("shell cannot be empty".into()));
            }
            let program = parts[0].clone();
            let prefix_args = parts[1..].to_vec();
            let resolved_shell = paths.resolve_component_bin(&program);
            if !posix::is_executable_file(&resolved_shell) {
                return Err(ConfigError::Invalid(format!(
                    "shell executable `{program}` not found or not executable (resolved `{}`)",
                    resolved_shell.display()
                )));
            }
            (program, prefix_args)
        } else {
            (String::new(), Vec::new())
        };

        Ok(Self {
            paths,
            config_file,
            hooks_dir,
            name,
            provider,
            model,
            shell,
            shell_program,
            shell_prefix_args,
            tool_name,
            max_turns,
            max_tokens,
            max_output_tokens,
            compact_threshold,
            no_compact: cli.no_compact,
            output,
            stderr,
            trace_fd,
            context,
            leaf,
            lock_wait,
            inbox,
            idle_timeout_sec,
            inbox_debounce_ms,
            notify,
            notify_id,
            params,
            sandbox,
            providers,
            compact,
        })
    }

    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        self.providers.get(&self.provider)
    }

    pub fn shell_enabled(&self) -> bool {
        !self.shell.trim().eq_ignore_ascii_case("none")
    }

    /// Non-blocking startup diagnostics (RFC §3.4).
    pub fn startup_warnings(&self, prompt: &str) -> Vec<String> {
        let mut warnings = Vec::new();
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
            timeout_sec: DEFAULT_PROVIDER_TIMEOUT_SEC,
            params: BTreeMap::new(),
        },
    );
    map
}

pub(crate) fn merge_providers(
    target: &mut BTreeMap<String, ProviderConfig>,
    layer: &BTreeMap<String, RawProviderConfig>,
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
                timeout_sec: DEFAULT_PROVIDER_TIMEOUT_SEC,
                params: BTreeMap::new(),
            });
        if let Some(ref bin) = raw.bin {
            entry.bin = bin.clone();
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

/// Split a shell specification (e.g. `anix-approve --mode deny -- msh`) into argv tokens.
pub fn split_shell_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                cur.push(ch);
            }
        } else if in_double {
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            } else if ch == '"' {
                in_double = false;
            } else {
                cur.push(ch);
            }
        } else {
            match ch {
                '\'' => in_single = true,
                '"' => in_double = true,
                '\\' => {
                    if let Some(next) = chars.next() {
                        cur.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                    }
                }
                _ => cur.push(ch),
            }
        }
    }

    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

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

fn parse_compact(raw: &RawCompactConfig, home_dir: &Path) -> Result<CompactConfig, ConfigError> {
    if raw.prompt.is_some() && raw.prompt_file.is_some() {
        return Err(ConfigError::Invalid(
            "[compact].prompt and [compact].prompt_file are mutually exclusive".into(),
        ));
    }
    Ok(CompactConfig {
        provider: raw.provider.clone(),
        model: raw.model.clone(),
        prompt: raw.prompt.clone(),
        prompt_file: raw
            .prompt_file
            .as_deref()
            .map(|p| expand_tilde(p, home_dir)),
        bin: raw
            .bin
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned),
    })
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

fn resolve_output(
    cli: &Cli,
    fm: Option<OutputMode>,
    defaults: &RawDefaults,
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
    if let Some(ref raw) = defaults.output {
        return parse_output_mode(raw).map_err(ConfigError::Invalid);
    }
    Ok(OutputMode::Reply)
}

fn resolve_stderr(
    cli: &Cli,
    fm: Option<StderrMode>,
    defaults: &RawDefaults,
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
    if let Some(ref raw) = defaults.stderr {
        return parse_stderr_mode(raw).map_err(ConfigError::Invalid);
    }
    Ok(StderrMode::Auto)
}

fn resolve_sandbox(
    cli_sandbox: Option<&str>,
    cfg_sb: &RawSandboxConfig,
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
    let enabled = cfg_sb.enabled.unwrap_or(false);
    let strict = cfg_sb.strict.unwrap_or(false);
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

pub fn expand_tilde(path: &str, home_dir: &Path) -> PathBuf {
    if path == "~" {
        return home_dir.to_path_buf();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir.join(rest);
    }
    PathBuf::from(path)
}
