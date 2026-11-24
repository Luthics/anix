# AGENTS.md

## 1. 规范源
- 协议、I/O、安全门禁与运行时行为以 [docs/v1.0.0-RFC.md](docs/v1.0.0-RFC.md) 为唯一事实源，本文档不重复规范正文。
- 如发现协议缺口或需调整协议/行为，**必须先与用户明确沟通确认，严禁擅自修改 RFC 或偏离规范实现**。

## 2. Workspace 边界
- `crates/os-agent-proto`：唯一线协议事实源，**禁止引入网络 / 异步 / OS 运行时依赖**。
- `crates/os-agent-core`（`agent`）、`crates/msh`（`msh`）、`crates/provider-openai-responses`（`agent-provider-openai-responses`）、`crates/agent-tools`（`aview`、`aclr`）：各二进制职责严格按 RFC 划分。
- Linux 专属系统调用（`pidfd`、`PR_SET_PDEATHSIG`、Landlock）严格隔离在 `#[cfg(target_os = "linux")]` 模块（`crates/os-agent-core/src/platform/linux.rs`）。

## 3. Rust 编码规范
- **零警告**：提交前必须通过 `cargo fmt --all -- --check` 与 `cargo xclippy`（`-D warnings`），保持 0 未使用依赖警告。
- **依赖管理**：公共依赖集中在根 `Cargo.toml`；为保证 `x86_64-unknown-linux-musl` 完全静态链接（`ldd` → `not a dynamic executable`），优先选用纯 Rust 库（如 `rustls`）。
- **错误处理**：库层用 `thiserror`，CLI 入口统一映射为 `os_agent_proto::exit_code::ExitCode`；处理外部输入严禁 `.unwrap()` / `.expect()`。
- **`unsafe` 纪律**：仅限底层 POSIX/Linux syscall 封装，每处必须附 `// SAFETY:` 注释。

## 4. 测试与验证
- 默认测试套件严禁依赖真实外网 LLM API，集成测试使用 Mock Provider 与临时目录。
- 常用命令：
  ```sh
  cargo fmt --all -- --check
  cargo xclippy
  cargo xtest
  ./scripts/install-local.sh           # 按 FHS 路径安装到 ~/.local/{bin,libexec/agent}
  ```
- 涉及 FD、进程组、信号、`flock`、Landlock 或静态链接的改动，必须在 Linux 环境完成目标架构验证。

## 5. Git 与隐私
- 本仓库强制 GPG 签名提交与打标（`commit.gpgsign = true`、`tag.gpgsign = true`），禁止 `--no-gpg-sign`。
- 使用 Conventional Commits（如 `feat(msh): ...`、`fix(core): ...`、`test(linux): ...`），保持提交小而原子。
