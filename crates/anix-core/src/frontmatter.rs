//! `*.agent` script parser: UTF-8, CRLF/LF, Shebang, and fail-closed YAML 1.2 Frontmatter (Sections 3.2 & 3.3).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anix_proto::ir::{ApprovalMode, StderrMode};
use serde_json::Value as JsonValue;
use serde_yaml_ng::Value as YamlValue;
use thiserror::Error;

/// Compact threshold parsed from Frontmatter, config.toml, env, or CLI (Section 3.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompactThreshold {
    /// Disabled (`0` or `--no-compact`).
    Disabled,
    /// Fractional ratio strictly within `(0.0, 1.0)`.
    Ratio(f64),
    /// Absolute token count (`>= 1`).
    Tokens(u64),
}

impl CompactThreshold {
    /// Parse a CLI or env string representation into [`CompactThreshold`].
    pub fn parse_str(raw: &str) -> Result<Self, String> {
        let s = raw.trim();
        if s.is_empty() {
            return Err("compact_threshold cannot be empty".into());
        }
        if let Ok(int_val) = s.parse::<u64>() {
            return if int_val == 0 {
                Ok(Self::Disabled)
            } else {
                Ok(Self::Tokens(int_val))
            };
        }
        if let Ok(f) = s.parse::<f64>() {
            if !f.is_finite() || f < 0.0 {
                return Err(format!("invalid compact_threshold `{raw}`"));
            }
            if f == 0.0 {
                return Ok(Self::Disabled);
            }
            if f > 0.0 && f < 1.0 {
                return Ok(Self::Ratio(f));
            }
            if f >= 1.0 && f.fract() == 0.0 {
                return Ok(Self::Tokens(f as u64));
            }
            return Err(format!(
                "compact_threshold `{raw}` must be 0, a float in (0.0, 1.0), or an integer >= 1"
            ));
        }
        Err(format!("invalid compact_threshold `{raw}`"))
    }

    /// Resolve effective token threshold against `max_tokens`.
    /// Returns `None` if disabled or `>= max_tokens`.
    pub fn resolve(self, max_tokens: u64) -> Option<u64> {
        let resolved = match self {
            Self::Disabled => return None,
            Self::Ratio(r) => ((max_tokens as f64) * r).floor() as u64,
            Self::Tokens(t) => t,
        };
        if resolved == 0 || resolved >= max_tokens {
            None
        } else {
            Some(resolved)
        }
    }
}

/// Parsed fields from a `*.agent` script's YAML Frontmatter (all optional).
#[derive(Debug, Clone, Default)]
pub struct AgentFrontmatter {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub vision: Option<bool>,
    pub shell: Option<String>,
    pub max_turns: Option<u32>,
    pub max_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub compact_threshold: Option<CompactThreshold>,
    pub approval: Option<ApprovalMode>,
    pub stderr: Option<StderrMode>,
    pub context: Option<String>,
    pub params: BTreeMap<String, JsonValue>,
}

/// Parsed `*.agent` script (Frontmatter + prompt body).
#[derive(Debug, Clone)]
pub struct AgentScript {
    pub default_name: String,
    pub frontmatter: AgentFrontmatter,
    pub prompt: String,
}

#[derive(Debug, Error)]
pub enum FrontmatterError {
    #[error("failed to read script `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("script `{0}` is not valid UTF-8")]
    InvalidUtf8(String),
    #[error("unclosed YAML frontmatter block (missing closing `---`)")]
    UnclosedFrontmatter,
    #[error("invalid YAML frontmatter: {0}")]
    Yaml(String),
    #[error(
        "unknown frontmatter field `{0}` (only defined fields and `x-*` extensions are allowed)"
    )]
    UnknownField(String),
    #[error("invalid frontmatter value for `{field}`: {reason}")]
    InvalidValue { field: &'static str, reason: String },
}

/// Load and parse a `*.agent` file from disk.
pub fn parse_script_file(path: &Path) -> Result<AgentScript, FrontmatterError> {
    let bytes = fs::read(path).map_err(|source| FrontmatterError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let raw = String::from_utf8(bytes)
        .map_err(|_| FrontmatterError::InvalidUtf8(path.display().to_string()))?;
    let default_name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "anix".into());
    parse_script_str(&raw, &default_name)
}

/// Parse a `*.agent` script string (supports `\n` & `\r\n`, optional Shebang, optional YAML 1.2 Frontmatter).
pub fn parse_script_str(raw: &str, default_name: &str) -> Result<AgentScript, FrontmatterError> {
    let normalized = raw.replace("\r\n", "\n");
    let mut lines: Vec<&str> = normalized.split('\n').collect();

    // 1. Ignore leading Shebang line (`#!...`).
    if lines.first().is_some_and(|l| l.starts_with("#!")) {
        lines.remove(0);
    }

    // 2. Check for opening `---` on its own line immediately after Shebang (or line 1).
    let (frontmatter, body_lines) = if lines.first().copied() == Some("---") {
        let Some(end_idx) = lines.iter().skip(1).position(|&l| l == "---") else {
            return Err(FrontmatterError::UnclosedFrontmatter);
        };
        let yaml_block = lines[1..=end_idx].join("\n");
        let fm = parse_yaml_frontmatter(&yaml_block)?;
        (fm, &lines[(end_idx + 2)..])
    } else {
        (AgentFrontmatter::default(), &lines[..])
    };

    let prompt = trim_blank_edges(body_lines);
    let sanitized_default = sanitize_default_name(default_name);

    Ok(AgentScript {
        default_name: sanitized_default,
        frontmatter,
        prompt,
    })
}

fn parse_yaml_frontmatter(yaml_block: &str) -> Result<AgentFrontmatter, FrontmatterError> {
    if yaml_block.trim().is_empty() {
        return Ok(AgentFrontmatter::default());
    }
    let doc: YamlValue =
        serde_yaml_ng::from_str(yaml_block).map_err(|e| FrontmatterError::Yaml(e.to_string()))?;
    let Some(map) = doc.as_mapping() else {
        if doc.is_null() {
            return Ok(AgentFrontmatter::default());
        }
        return Err(FrontmatterError::Yaml(
            "frontmatter root must be a YAML mapping".into(),
        ));
    };

    let mut fm = AgentFrontmatter::default();
    for (k, v) in map {
        let Some(key) = k.as_str() else {
            return Err(FrontmatterError::Yaml(
                "frontmatter keys must be strings".into(),
            ));
        };
        if key.starts_with("x-") {
            continue;
        }
        match key {
            "name" => {
                let s = expect_string("name", v)?;
                validate_agent_name(&s)?;
                fm.name = Some(s);
            }
            "provider" => {
                let s = expect_string("provider", v)?;
                if s.trim().is_empty() {
                    return Err(FrontmatterError::InvalidValue {
                        field: "provider",
                        reason: "must not be empty".into(),
                    });
                }
                fm.provider = Some(s);
            }
            "model" => {
                let s = expect_string("model", v)?;
                if s.trim().is_empty() {
                    return Err(FrontmatterError::InvalidValue {
                        field: "model",
                        reason: "must not be empty".into(),
                    });
                }
                fm.model = Some(s);
            }
            "vision" => {
                let b = v.as_bool().ok_or_else(|| FrontmatterError::InvalidValue {
                    field: "vision",
                    reason: "expected boolean (`true` or `false`)".into(),
                })?;
                fm.vision = Some(b);
            }
            "shell" => {
                let s = expect_string("shell", v)?;
                if s.trim().is_empty() {
                    return Err(FrontmatterError::InvalidValue {
                        field: "shell",
                        reason: "must not be empty".into(),
                    });
                }
                fm.shell = Some(s);
            }
            "max_turns" => {
                let n = expect_u32("max_turns", v)?;
                if n < 1 {
                    return Err(FrontmatterError::InvalidValue {
                        field: "max_turns",
                        reason: "must be >= 1".into(),
                    });
                }
                fm.max_turns = Some(n);
            }
            "max_tokens" => {
                let n = expect_u32("max_tokens", v)?;
                if n < 1024 {
                    return Err(FrontmatterError::InvalidValue {
                        field: "max_tokens",
                        reason: "must be >= 1024".into(),
                    });
                }
                fm.max_tokens = Some(n);
            }
            "max_output_tokens" => {
                let n = expect_u32("max_output_tokens", v)?;
                fm.max_output_tokens = Some(n);
            }
            "compact_threshold" => {
                fm.compact_threshold = Some(parse_yaml_compact_threshold(v)?);
            }
            "approval" => {
                let s = expect_string("approval", v)?;
                fm.approval = Some(parse_approval_mode(&s).map_err(|reason| {
                    FrontmatterError::InvalidValue {
                        field: "approval",
                        reason,
                    }
                })?);
            }
            "stderr" => {
                let s = expect_string("stderr", v)?;
                fm.stderr = Some(parse_stderr_mode(&s).map_err(|reason| {
                    FrontmatterError::InvalidValue {
                        field: "stderr",
                        reason,
                    }
                })?);
            }
            "context" => {
                let s = expect_string("context", v)?;
                fm.context = Some(s);
            }
            "params" => {
                // Open key-value pass-through dictionary (internal keys exempt from Fail-Closed).
                let json_val =
                    serde_json::to_value(v).map_err(|e| FrontmatterError::InvalidValue {
                        field: "params",
                        reason: e.to_string(),
                    })?;
                let Some(obj) = json_val.as_object() else {
                    return Err(FrontmatterError::InvalidValue {
                        field: "params",
                        reason: "must be a mapping/object".into(),
                    });
                };
                for (pk, pv) in obj {
                    fm.params.insert(pk.clone(), pv.clone());
                }
            }
            other => return Err(FrontmatterError::UnknownField(other.to_owned())),
        }
    }

    Ok(fm)
}

pub fn validate_agent_name(name: &str) -> Result<(), FrontmatterError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(FrontmatterError::InvalidValue {
            field: "name",
            reason: format!("`{name}` must match [a-zA-Z0-9._-]+"),
        });
    }
    Ok(())
}

pub fn parse_approval_mode(raw: &str) -> Result<ApprovalMode, String> {
    match raw.trim() {
        "ask" => Ok(ApprovalMode::Ask),
        "auto" => Ok(ApprovalMode::Auto),
        "deny" => Ok(ApprovalMode::Deny),
        other => Err(format!("expected `ask`, `auto`, or `deny`, got `{other}`")),
    }
}

pub fn parse_stderr_mode(raw: &str) -> Result<StderrMode, String> {
    match raw.trim() {
        "auto" => Ok(StderrMode::Auto),
        "explicit" => Ok(StderrMode::Explicit),
        other => Err(format!("expected `auto` or `explicit`, got `{other}`")),
    }
}

fn expect_string(field: &'static str, v: &YamlValue) -> Result<String, FrontmatterError> {
    v.as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| FrontmatterError::InvalidValue {
            field,
            reason: "expected string".into(),
        })
}

fn expect_u32(field: &'static str, v: &YamlValue) -> Result<u32, FrontmatterError> {
    let n = v.as_u64().ok_or_else(|| FrontmatterError::InvalidValue {
        field,
        reason: "expected non-negative integer".into(),
    })?;
    u32::try_from(n).map_err(|_| FrontmatterError::InvalidValue {
        field,
        reason: "integer out of u32 range".into(),
    })
}

fn parse_yaml_compact_threshold(v: &YamlValue) -> Result<CompactThreshold, FrontmatterError> {
    if let Some(u) = v.as_u64() {
        return Ok(if u == 0 {
            CompactThreshold::Disabled
        } else {
            CompactThreshold::Tokens(u)
        });
    }
    if let Some(f) = v.as_f64() {
        if !f.is_finite() || f < 0.0 {
            return Err(FrontmatterError::InvalidValue {
                field: "compact_threshold",
                reason: format!("invalid float `{f}`"),
            });
        }
        if f == 0.0 {
            return Ok(CompactThreshold::Disabled);
        }
        if f > 0.0 && f < 1.0 {
            return Ok(CompactThreshold::Ratio(f));
        }
        if f >= 1.0 && f.fract() == 0.0 {
            return Ok(CompactThreshold::Tokens(f as u64));
        }
        return Err(FrontmatterError::InvalidValue {
            field: "compact_threshold",
            reason: "must be 0, a float in (0.0, 1.0), or an integer >= 1".into(),
        });
    }
    Err(FrontmatterError::InvalidValue {
        field: "compact_threshold",
        reason: "expected float or non-negative integer".into(),
    })
}

fn trim_blank_edges(lines: &[&str]) -> String {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(start);
    if start >= end {
        String::new()
    } else {
        lines[start..end].join("\n")
    }
}

fn sanitize_default_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "anix".into()
    } else {
        cleaned
    }
}
