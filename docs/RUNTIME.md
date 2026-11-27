# Anix Runtime（Layer 2 参考发行包）说明文档

**性质**：Informative（非 Core 规范，任何组件均可按 [RFC.md](RFC.md) 独立替换）  
**适用范围**：`anix-runtime`（Layer 2）默认组合发行包——包含 `anix`（Core）、`msh`（含后台 reaper）、`anix-approve`（Shell 审批包装器）、`anix-provider`、`a*` 工具集（`aview`、`aclr`、`aplan`）、独立前置钩子脚本（`10-agents-md`、`15-memory`、`20-skills`、`30-tips`）以及跨平台安装与自检约定。  

---

## 1. 与 Layer 1（Core）的边界

* **Layer 1（[RFC.md](RFC.md)）** 定义 `anix` 极简三原语内核（`*.agent` + 树状 WAL + Maildir IPC）与跨进程线协议；`crates/anix-core` 与 `crates/anix-proto` **零依赖**本文档中的任何非 Core 组件。
* **Layer 2（本文档）** 提供面向开发者的开箱即用参考实现。你可以自由替换其中任意一块：
  * 用原生 `/bin/sh`、`bash` 或自研沙箱 Shell 替换 `msh`；
  * 用自研规则引擎或 Layer 3 交互弹窗包装器替换 `anix-approve`；
  * 用任意语言（Python / Go / Node / Rust）编写的自定义驱动替换 `anix-provider`；
  * 覆写或屏蔽 `hooks/pre-context.d/` 下的 `20-skills`、`30-tips`。

---

## 2. FHS 安装布局与组件总览

| 组件 | 默认安装路径 | 职责边界 |
| :--- | :--- | :--- |
| **`anix`** | `~/.local/bin/anix` | Layer 1 核心解释器与工具链（`send`/`fork`/`fsck`/`log`/`--doctor`）。唯一默认进入用户 `$PATH` 的用户入口。 |
| **`msh`** | `~/.local/libexec/anix/msh` | 面向模型的非交互 Shell 包装器：防挂起环境、二进制熔断、ANSI 清洗、输出折叠、按需超时转后台 + reaper 自动通过 `anix send` 回流 `$ANIX_INBOX`。 |
| **`anix-approve`** | `~/.local/libexec/anix/anix-approve` | 独立 Shell 审批包装器（如 `shell: anix-approve -- msh`）：支持 `safe_commands` 白名单、外部裁决器 `--decider`、`/dev/tty` `[y/n/a/e]` 交互审批与无 TTY Fail-Closed（退出 `75`）。 |
| **`anix-provider`** | `~/.local/libexec/anix/anix-provider` | 统一多方言 HTTP/SSE Provider 二进制，通过同目录 BusyBox 软链 `anix-provider-{openai-responses,openai-compat,anthropic}` 自动分流方言。 |
| **`aview`、`aclr`、`aplan`** | `~/.local/libexec/anix/tools/` | 模型侧显式工具（通过 `$ANIX_CTL` 与 `anix` 通信）；由 `anix` 运行时前置注入子进程 `$PATH`。 |
| **`20-skills`、`30-tips`** | `~/.config/anix/hooks/pre-context.d/` | 独立 `pre-context.d/` 可执行钩子脚本（与内置 `builtin-tips/` 一并安装），遵循 POSIX `StrictModes`。 |

---

## 3. 参考 Shell 执行器：`msh`

`msh` 面向非人类模型优化，作为默认 `shell = "msh"`（或组合 `shell = "anix-approve -- msh"`）：

1. **进程组隔离与防挂起**：以 `setsid()` 独立会话/进程组通过 `/bin/sh -c` 执行，注入 `NONINTERACTIVE=1`、`DEBIAN_FRONTEND=noninteractive`、`PAGER=cat`、`GIT_TERMINAL_PROMPT=0`、`TERM=dumb`、`NO_COLOR=1`。
2. **按需超时与后台 Reaper（接入 v0.4.0 `hold` + `ANIX_INBOX`）**：
   * 读取 `ANIX_EXEC_TIMEOUT_SEC`（回退 `MSH_TIMEOUT_SEC`，默认 `120` 秒；`0` 表示立即转后台）与 `ANIX_EXEC_KILL_ON_TIMEOUT`（回退 `MSH_KILL_ON_TIMEOUT`，默认 `false`）。
   * 设为 `kill_on_timeout=true` 超时时：`SIGTERM` 整个进程组 → 最多等 2 秒 → `SIGKILL` → 退出 `124`。
   * 默认 `kill_on_timeout=false` 转后台时：
     * 向 `$ANIX_CTL` 写入标准租约帧 `{"op":"hold","id":"<job_id>","deadline_sec":...,"label":"<cmd>"}`；
     * 后台 reaper 进程 `wait` 命令结束后，原子写入 `<job_id>.exit`；若环境中存在 `ANIX_INBOX`，reaper 自动调用 `anix send "$ANIX_INBOX" --in-reply-to "<job_id>" --from "msh"` 将 `{job, exit_code, log_path, tail}` 投递回父 Agent 的 Maildir，立刻唤醒休眠中的常驻父 Agent 并释放 `hold` 租约！
3. **二进制熔断**：探测输出前 8 KB，含 NUL 字节或高比例不可打印控制字符时拦截输出、将原始字节转存 `/tmp/msh_<uuid>.bin`（`0600`），仅返回摘要与路径。
4. **ANSI 清洗与 `\r` 折叠**：剥离转义序列并将进度条 `\r` 折叠为末帧。
5. **输出折叠**：超过 `MSH_MAX_LINES`（默认 `200`）或 `MSH_MAX_BYTES`（默认 `8192`）时将全文存盘 `/tmp/msh_<uuid>.log`（`0600`），仅返回首 20 行 + 统计/路径 + 尾 20 行。

---

## 4. Shell 审批包装器：`anix-approve`

审批策略已从 Layer 1 内核移出，作为独立的可组合 POSIX 包装器存在：

```yaml
# 在 *.agent Frontmatter 中按需组合：
shell: anix-approve -- msh
# 或带参数：
shell: anix-approve --mode deny -- msh
shell: anix-approve --decider ~/.config/anix/my-policy -- msh
```

* **调用格式**：`anix-approve [--mode ask|auto|deny] [--safe-cmd <PREFIX>]... [--decider <BIN>] [--decider-timeout <SEC>] [--fail-on-deny] -- <SHELL_BIN> [SHELL_ARGS...] -c <CMD>`
* **决策顺序**：`auto` 直接 `exec` 转发；`deny` 仅放行白名单（被拒时输出拒绝信息并按 `--fail-on-deny` 退出 `75`）；`ask` 先查白名单与可选 `--decider`（退出 `0`=放行、`1`=拒绝、`2`=回退人工），最后以 `O_RDWR` 打开 `/dev/tty` 提供 `[y/n/e]` 交互（无法打开 `/dev/tty` 时 Fail-Closed 退出 `75`）。

---

## 5. 参考辅助工具集：`aview`、`aclr` 与 `aplan`

* **`aview [--max-dim 1568] [--pages 1-3] <path>`**：将本地图片/PDF 转码压缩至 `/tmp/aview_<uuid>.png`（`0600`），向 `$ANIX_CTL` 写入 `media` 帧。
* **`aclr -s`**：向 `$ANIX_CTL` 写入 `{"op":"compact"}`，触发全量压缩。
* **`aclr -t <turn_id> ["<summary>"]`**：向 `$ANIX_CTL` 写入 `fold` 帧，执行手工或自动摘要尾部折叠（保留冻结前缀缓存）。
* **`aplan`**：向 `$ANIX_CTL` 写入瞬态 `{"op":"plan","steps":[...]}` 帧，实时刷新 Layer 3 前端任务进度条而不写入 WAL、不破坏前缀缓存；支持标准输入 Markdown 清单（`- [x]` / `- [~]` / `- [ ]` / `- [-]` / `- [!]`）、标志参数（`-d`/`-i`/`-p`/`-s`/`-f`）及 `aplan clear`。

---

## 6. 参考 Provider 驱动：`anix-provider`

单二进制通过 `argv[0]` 软链分流（`openai-responses` / `openai-compat` / `anthropic`），支持 `coprocess`（HTTP/2 连接复用）与 `oneshot` 模式：

* **OpenAI Responses（`/v1/responses`）**：冻结首条 `system` 映射至 `input[0]` 的 `role: "developer"`（保障 GPT-6.2+ 前缀缓存命中），顶层 `instructions` 保留给 `params.instructions`；默认 `prompt_cache_key = session_id` 与 `session_id` 请求头；自动透传响应/请求头 `x-codex-turn-state` 及加密 `reasoning` 项 via `provider_opaque`。
* **OpenAI-Compatible（`/v1/chat/completions`）**：兼容 DeepSeek / vLLM / Ollama / OpenRouter；免鉴权本地端点自动省略 `Authorization`。
* **Anthropic Messages（`/v1/messages`）**：合并相邻同角色消息，处理 `thinking` `signature` 与 `cache_control: {"type":"ephemeral"}`。

---

## 7. 独立前置钩子脚本（`10-agents-md`、`15-memory`、`20-skills` 与 `30-tips`）

`10-agents-md`、`15-memory`、`20-skills` 与 `30-tips` 均以标准 `pre-context.d/` 可执行脚本随 Layer 2 分发（权限 `0755`，受 Core POSIX `StrictModes` 保护，且仅在启用工具/Shell 时生效）：

* **`10-agents-md`**：按广到窄顺序（`~/.config/anix/` → git root 沿途至 `cwd`）收集 `AGENTS.md`、`AGENTS.local.md`、`CLAUDE.md` 与 `CLAUDE.local.md`（路径与同内容自动去重，单文件 16 KiB / 总计 48 KiB 截断上限，遇不可读/非 UTF-8 文件 fail-open 跳过），以 `<project_instructions>` 包裹 `<agents_md>` / `<claude_md>` 块追加到首条 `system`（可设 `ANIX_AGENTS_MD=0` 或 mask 禁用）。
* **`15-memory`**：按主 Git 仓库根（自动跨 worktree 共享）定位 `${XDG_DATA_HOME:-~/.local/share}/anix/memory/<project-key>/`，将 `MEMORY.md` 索引前 200 行 / 25 KiB 与精简写入契约以 `<auto_memory>` 块注入首条 `system`（可设 `ANIX_MEMORY=0` 或 mask 禁用）。
* **`20-skills`**：按优先级扫描 `./.anix/skills/` → `./.agents/skills/` → `./.skills/` → `./.claude/skills/` → `~/.config/anix/skills/` → `~/.agents/skills/` → `~/.skills/` → `~/.claude/skills/`，向首条 `system` 追加 `<available_skills>` 索引。
* **`30-tips`**：合并内置指南（`how-to-background.md`、`how-to-goal.md`、`how-to-loop.md`、`how-to-mcp.md`、`how-to-runtime.md`、`how-to-sub-agent.md`）、`~/.config/anix/tips/` 与 `./.anix/tips/`，输出 `<runtime_tips>` 索引。
* 用户随时可用 0 字节文件或 `/dev/null` 软链屏蔽任意脚本，无需重新编译内核。

---

## 8. 安装分发与自检（`install.sh` / `anix --doctor`）

* **一键安装**：`install.sh` / `scripts/install-local.sh` 安装静态零依赖二进制至 `~/.local/{bin,libexec/anix}`，释放默认 `hooks/pre-context.d/{20-skills,30-tips}` 与初始配置模板。
* **Linux 强化**：支持纯静态 `x86_64/aarch64-unknown-linux-musl` 构建，及 `--sandbox` Linux Landlock 只读/读写隔离。
* **`anix --doctor`**：检查内核环境、配置文件、权限位、`shell` 与默认 Provider 连通性。
