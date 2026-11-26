# Anix Runtime（参考运行底座与发行说明）

**状态**：Informative / Reference Distribution（配套 [RFC.md](RFC.md) `v0.2.0-rfc`）  
**适用范围**：`anix-runtime`（Layer 2）默认组合发行包——包含 `anix`（Core）、`msh`、`anix-provider`、`a*` 工具集（`aview`、`aclr`）、内置增强钩子（`20-skills`、`30-tips`）以及跨平台安装与自检约定。

> **分层约定**：本文档中的所有非 Core 组件均属于**可替换参考实现**。任何遵守 [RFC.md](RFC.md) 线协议的外部 Shell、Provider 驱动、工具或钩子都可以直接替换这里的默认组件；同时 `crates/anix-core` 在编译期对 `msh`、`anix-tools`、`anix-providers` 保持**零依赖**。更高阶的交互式控制壳（`anix-agent`）及第三方生态插件（`anix-*`）均在独立仓库演进，不归属本仓库。

---

## 1. 四层架构中的定位

```text
Layer 4: anix-*         独立分仓生态（第三方 Provider / 审批策略 / Trace 面板 / Skills 包）
                           ▲
Layer 3: anix-agent     独立分仓高阶交互壳（TUI / 会话管理 / 审批 UI / 动态上下文注入）
                           ▲ 仅通过 RFC 外部协议驱动
Layer 2: anix-runtime   本文档：开箱即用的参考发行包（anix + msh + a* + anix-provider）
                           ▲ 严格单向依赖
Layer 1: anix-core      RFC.md：最小 POSIX Agent 内核（anix + anix-proto）
```

在 `0.x` 协议收敛期，`anix-core` 与 `anix-runtime` 共享同一个 Rust Workspace（`Luthics/anix`），以便单次提交同步演进协议并运行全链路集成测试；Cargo 依赖图保证 `anix-core` 仅依赖 `anix-proto`，严禁反向依赖 `msh`、`anix-tools` 或 `anix-providers`。

---

## 2. FHS 安装布局与组件总览

参考发行包采用独立静态编译二进制按 POSIX/FHS 惯例分层安装，严禁将内部辅助程序塞入用户全局 `$PATH`（仅主入口 `anix` 进入 `~/.local/bin/`）：

| 组件 | 对应 Crate | 角色定位 | 默认安装路径 | 核心职责 |
| :--- | :--- | :--- | :--- | :--- |
| **`anix`** | `crates/anix-core` | 核心解释器（Layer 1） | `~/.local/bin/anix` | 实现 [RFC.md](RFC.md) 全部 Normative 契约：CLI/Frontmatter/配置、FD 路由、三轴输出、WAL、事件循环、信号与外部线协议。 |
| **`msh`** | `crates/msh` | 默认 Model-oriented Shell | `~/.local/libexec/anix/msh` | 非交互 Shell 包装器：防挂起环境注入、二进制输出熔断、ANSI 清洗、超长折叠、按需超时自动转后台与进程组回收。 |
| **`anix-provider`** | `crates/anix-providers` | 默认多方言 Provider 驱动 | `~/.local/libexec/anix/anix-provider`（BusyBox 软链 `anix-provider-{openai-responses,openai-compat,anthropic}`） | Canonical IR v1 与 OpenAI Responses、OpenAI Chat Completions 兼容端点及 Anthropic Messages 之间的双向转换（支持 `coprocess` / `oneshot`）。 |
| **`aview`、`aclr`** | `crates/anix-tools` | 默认显式模型工具集（`a*`） | `~/.local/libexec/anix/tools/` | 通过 `$ANIX_CTL`（`fd 6`）与内核通信：`aview` 负责多模态图片/PDF 渲染附加，`aclr` 负责显式全量/尾部上下文折叠。 |

* 拉起 `msh` 时，`anix` 自动将 `~/.local/libexec/anix/tools/` 前置到子进程 `PATH`，使 `aview`/`aclr` 开箱即用且不污染人类终端全局 `$PATH`。
* 用户可通过将同名可执行文件放入 `$PATH`，或在配置中填写含 `/` 的绝对/相对路径，无缝替换任意组件。

---

## 3. Model-oriented Shell（`msh`）参考实现

调用方式：`msh -c "<command_string>"`（底层调用 `/bin/sh -c`，置于独立会话与进程组 `setsid()` 中，确保信号可干净回收整棵子进程树）。

### 3.1 防挂起环境变量注入

强制注入以下非交互环境：
`NONINTERACTIVE=1`、`DEBIAN_FRONTEND=noninteractive`、`PAGER=cat`、`GIT_TERMINAL_PROMPT=0`、`TERM=dumb`、`NO_COLOR=1`。

### 3.2 输出清洗管线

1. **二进制输出熔断（优先于文本清洗）**：检测合并捕获的输出流，若前 `8192` 字节包含 NUL（`0x00`）或整段流无法通过 UTF-8 校验：
   * 严禁向模型输出乱码或擅自转多模态；
   * 将原始字节写入 `/tmp/msh_<uuid>.bin`（权限 `0600`），基于文件头魔数探测 MIME（回退 `application/octet-stream`）；
   * 仅返回单行元信息：
     `[binary output suppressed: <bytes>B, mime: <mime>, saved to /tmp/msh_<uuid>.bin. Use xxd/strings/pdftotext or aview to inspect]`。
2. **ANSI 清洗**：对合法 UTF-8 文本剥离所有 CSI / OSC 转义序列与覆盖式 `\r` 进度条。
3. **超长输出折叠**：阈值默认为 `MSH_MAX_LINES=200` 或 `MSH_MAX_BYTES=8192`（可通过环境变量覆写）。超出任一阈值时，将清洗后的完整文本写入 `/tmp/msh_<uuid>.log`（权限 `0600`），仅返回 `exit_code`、前 20 行、省略行统计、后 20 行及完整日志路径。

### 3.3 按需等待、自动转后台（Yield-to-Background）与可选强杀

* **策略输入**：通过 `anix` 注入的 `MSH_TIMEOUT_SEC`（默认 `120`）、`MSH_KILL_ON_TIMEOUT`（`"0"` | `"1"`，默认 `"0"`）与 `MSH_JOBS_DIR`（默认 `$XDG_RUNTIME_DIR/anix/jobs`，权限 `0700`）控制本次执行。
* **纯事件驱动等待与日志落盘**：`msh` 启动隔离会话（`setsid()`）子进程组，将合并的 stdout/stderr 实时写入 `$MSH_JOBS_DIR/<job_id>.log`（权限 `0600`），子进程退出时原子写入 `$MSH_JOBS_DIR/<job_id>.exit`（`tmp -> mv`）；主进程使用 Linux `pidfd_open + epoll` 或 macOS `kqueue` + `self-pipe` 等待退出或精确超时。
* **前台按时完成**：子进程在 `MSH_TIMEOUT_SEC` 内结束时，对日志执行二进制熔断、ANSI 清洗与折叠后输出至 stdout，清理临时任务文件并透传真实退出码。
* **超时自动转后台（默认，`MSH_KILL_ON_TIMEOUT=0`）**：
  * 当 `MSH_TIMEOUT_SEC == 0`（立即后台启动）或前台等待到期而子进程仍在运行时，`msh` **不杀死**该子进程组；`MSH_TIMEOUT_SEC == 0` 时在子进程 `exec` 前主动关闭 `fd 4/5/6` 并移除 `ANIX_OUT`/`ANIX_ERR`/`ANIX_CTL` 防止侧信道泄漏；
  * `msh` 读取当前日志尾部预览（经二进制与 ANSI 安全处理，默认最多尾部 `4096` 字节 / `40` 行），通过 `$ANIX_CTL` 发送 `{"op":"bg_job",...}` 控制帧通知 `anix`，在 stdout 输出结构化句柄提示后以退出码 **`0`** 返回：
    ```text
    [msh: still running after 30s — moved to background (job=j_a1b2c3d4, pid=12345, pgid=12345)]
    [msh: log=/tmp/anix-1000/jobs/j_a1b2c3d4.log | exit_file=/tmp/anix-1000/jobs/j_a1b2c3d4.exit | stop: kill -TERM -12345]
    ```
  * 模型可继续处理其他任务，或用原生 POSIX 命令（`tail`、`kill -0`、`kill -TERM -<pgid>`）观察或终止后台进程组。
* **显式超时强杀（`MSH_KILL_ON_TIMEOUT=1`）**：超时时向子进程组发送 `SIGTERM`（事件驱动等待最多 2 秒后仍未退出则 `SIGKILL`），清理临时任务文件，输出已有内容并追加 `[msh: command timed out after <N>s; process group terminated (exit=124)]`，以退出码 `124` 退出。
* **信号中断**：在前台等待期间收到 `SIGINT`/`SIGTERM` 时，无论 `MSH_KILL_ON_TIMEOUT` 取值如何，均终止当前前台子进程组（`SIGTERM` → 2s → `SIGKILL`）并返回 `130`/`143`。

---

## 4. 显式模型工具集（`a*`：`aview`、`aclr`）

**铁律**：`a*` 工具集绝不覆盖原生 POSIX 命令（`cat`/`ls`/`grep`），保证普通管道与脚本 100% 维持原生字节语义；所有控制指令均经由 `$ANIX_CTL`（`fd 6`）上报 `anix`。

### 4.1 `aview`（显式多模态查看）

```sh
aview [--max-dim <px, 默认 1568>] [--pages <1-3,5>] [FILE | -]
```

* 支持图片（PNG/JPEG/WebP/GIF）自动等比缩放与 PDF 指定页渲染为 PNG，结果落盘至 `/tmp/aview_<uuid>.png`（权限 `0600`）后向 `$ANIX_CTL` 写入 `{"op":"media","mime":"image/png","path":"...","source":"..."}` 控制帧。
* 实际是否附加给模型或转译为纯文本，完全由内核按 [RFC §5.5](RFC.md#55-视觉能力声明与非视觉模型降级vision--visionbin) 裁决。

### 4.2 `aclr`（显式上下文折叠）

```sh
aclr -s                       # 发送 {"op":"compact"}，触发全量压缩并写入 checkpoint
aclr -t <N> "<summary>"       # 发送 {"op":"fold","from_turn":N,"summary":"..."}，手工摘要尾部折叠（零额外 LLM 调用）
aclr -t <N>                   # 发送 {"op":"fold","from_turn":N}，按 [compact] 策略自动摘要尾部折叠
```

* 无参调用 `aclr` 严禁清空上下文，返回退出码 `2` 并打印用法（彻底重置会话直接 `rm <ctx.jsonl>`）。

---

## 5. 默认 Provider 驱动（`anix-provider`）

单个静态二进制 `anix-provider` 通过 `argv[0]` BusyBox 软链（或子命令/环境变量）分发三种内置方言：

| 软链名 / 方言 | 上游端点 | 系统前缀与缓存策略 |
| :--- | :--- | :--- |
| **`anix-provider-openai-responses`** | `{base_url}/responses` | 首条冻结 `role: "system"` 映射为 `input[0]` 的 **`role: "developer"`**（确保 GPT-6.2+ 及 Responses 网关完整命中前缀缓存）；顶层 `instructions` 留给 `params.instructions` 承载动态单轮指令；支持 `prompt_cache_retention` 透传；自动将上游 `encrypted_content` 推理项与 `x-codex-turn-state` 响应头通过 `provider_opaque` 跨轮透传。 |
| **`anix-provider-openai-compat`** | `{base_url}/chat/completions` | 首条冻结 `role: "system"` 映射为 `messages[0]`（`role: "system"`）；支持流式 `reasoning_content` 与非流式 JSON 回退。 |
| **`anix-provider-anthropic`** | `{base_url}/v1/messages` | 首条冻结 `role: "system"` 映射为顶层 `system`（含缓存标记）；自动将 `input_tokens` 归一化为包含式总输入口径（含 `cache_read_input_tokens` 与 `cache_creation_input_tokens`），并透传签名 `thinking` 块。 |

### 会话亲和映射约定（`session_id`）

内置 `anix-provider` 在收到非空 `session_id` 时，按各协议安全映射以避免未知字段 `400`：

* **HTTP Header（全部方言通用）**：默认发送 `x-session-id: <session_id>`；可通过 `params.session_header` 改名为自定义合法 Header 名，或设为 `false` / `null` 禁用。
* **`openai-responses`**：默认同时写入 Body 原生字段 `prompt_cache_key: <session_id>`；可通过 `params.prompt_cache_key` 传入显式字符串覆写，或设为 `false` / `null` 禁用（设为 `true` 等价于绑定当前 `session_id`）。若上游响应头返回 `x-codex-turn-state`，自动存入 `provider_opaque.responses_turn_state` 并在后续轮次原样回传请求头。
* **`openai-compat`**：默认仅发送 HTTP Header、不向 Body 注入未知字段；支持在 `params` 中将 `prompt_cache_key = true` 或 `session_id = true` 按需自动绑定为当前动态 `session_id`。
* **`anthropic`**：仅发送 HTTP Header（`x-session-id` 或 `params.session_header`），不向 Body 注入非标准字段。
* **免鉴权本地端点**：当 `ANIX_PROVIDER_API_KEY` 为空串时省略 `Authorization` 请求头，直接发起请求。

---

## 6. 内置增强上下文钩子（`20-skills` 与 `30-tips`）

为保证开箱即用且零额外子进程开销，参考发行包当前将 `20-skills` 与 `30-tips` 作为最低优先级内置逻辑随 `anix` 打包（仅在 `shell != "none"` 时生效）；用户或项目可按 [RFC §8.3](RFC.md#83-钩子覆盖与-anix-trust-安全门禁) 在 `hooks/pre-context.d/` 放置同名脚本覆写，或用 0 字节文件 / 指向 `/dev/null` 的软链彻底屏蔽。

### 6.1 `20-skills`（`.skills` 渐进式披露）

1. **扫描顺序（高优先同名覆盖低优先）**：  
   `./.anix/skills/` → `./.skills/` → `./.claude/skills/` → `~/.config/anix/skills/` → `~/.skills/` → `~/.claude/skills/`。
2. **索引提取**：仅解析 `<skill_dir>/SKILL.md` 的 YAML Frontmatter 中必需字段 `name` 与 `description`，按确定性 XML 格式追加至首条 `role: "system"` 消息文本末尾：
   ```xml
   <available_skills>
     <skill name="pdf-extract" path="/abs/path/SKILL.md">Extract tables from PDFs</skill>
   </available_skills>
   ```
3. **按需加载**：模型需要某项技能时通过 Shell 执行 `cat /abs/path/SKILL.md` 读取完整说明。

### 6.2 `30-tips`（渐进式运行时手册）

为避免在核心运行时中硬编码 `/loop`、`/goal`、MCP 客户端或多 Agent 编排等上层策略，`30-tips` 仅向模型注入按需查阅的指南路径索引：

* **零元数据索引**：`tips` 目录下的 `.md` 常规文件（忽略隐藏文件与子目录）不要求 Frontmatter，文件名本身即索引（推荐 `how-to-<topic>.md`）。
* **三层来源与同名覆写/单篇屏蔽**（高覆盖低；0 字节文件或指向 `/dev/null` 的软链屏蔽单篇）：
  1. **内置默认层**：二进制内置 `how-to-background.md`、`how-to-goal.md`、`how-to-loop.md`、`how-to-mcp.md`、`how-to-runtime.md`、`how-to-sub-agent.md`，首次执行时原子释放至 `${XDG_STATE_HOME:-$HOME/.local/state}/anix/builtin-tips/`（只读不可写时回退 `$XDG_RUNTIME_DIR/anix/builtin-tips/`）。
  2. **全局目录**：`${XDG_CONFIG_HOME:-$HOME/.config}/anix/tips/`
  3. **项目目录**：`<cwd>/.anix/tips/`
* **输出格式**：按文件名 ASCII 字典序输出 `<runtime_tips>` 索引块（合并为空时静默跳过，零 Token 开销）：
  ```xml
  <runtime_tips>
  For advanced POSIX workflows, read the relevant guide via msh when needed:
  /home/user/.local/state/anix/builtin-tips/how-to-background.md
  /home/user/.local/state/anix/builtin-tips/how-to-goal.md
  /home/user/.local/state/anix/builtin-tips/how-to-loop.md
  /home/user/.local/state/anix/builtin-tips/how-to-mcp.md
  /home/user/.local/state/anix/builtin-tips/how-to-runtime.md
  /home/user/.local/state/anix/builtin-tips/how-to-sub-agent.md
  </runtime_tips>
  ```
* **MCP 连接指南（`how-to-mcp.md`）原则**：严禁在内核内置 MCP 客户端或把大量 MCP Schema 预灌入 `header.tools`；指南面向模型提供发现 `.mcp.json`、优先调用 `mcporter`/`mcp-cli` 或原生 POSIX JSON-RPC 2.0 按需调用的最小范式。

---

## 7. 运行环境、Linux 内核强化与安装自检

### 7.1 容器底座建议

* **默认主力**：Debian Slim（~22 MB），提供原生 glibc 与完整 GNU Coreutils。
* **动态工具链环境**：Ubuntu Minimal（~30–75 MB）。
* **高密微沙盒**：Alpine Linux（~5–8 MB），适合全静态 musl 工具链场景。

### 7.2 Linux 内核特性优雅降级

* **全平台基础模式**：纯 POSIX + Linux `epoll`/`inotify` 或 macOS `kqueue`，完整支持 Linux、macOS、WSL2。
* **Linux ≥ 5.13 强化模式**：启动时自动探测内核能力：
  * **进程树回收（`pidfd` / `PR_SET_PDEATHSIG`）**：确保主进程异常终止时内核可靠回收全部子进程；
  * **Landlock LSM（可选开启：`--sandbox` 或 `[sandbox].enabled = true`）**：限制 Shell 子进程仅对 `$PWD`、`/tmp` 与 `$XDG_RUNTIME_DIR` 具备写权限，其余系统路径只读；内核不支持时警告降级（`--sandbox=strict` 直接退出 `1`）。

### 7.3 一行安装与自检（`anix --doctor`）

`install.sh` 以零 root 权限将 `anix-runtime` 发行包安装至 `~/.local/{bin,libexec/anix}`，初始化 starter `~/.config/anix/config.toml` 与 `~/.config/anix/agents/bash.agent`，不修改用户的 shell rc 文件。

`anix --doctor` 按序执行以下自检，全部通过返回 `0`，任一项失败返回 `1` 并给出修复建议：

1. 校验核心二进制 `anix` 与默认 Runtime 组件（`msh`、默认 Provider、`aview`、`aclr`）存在且具备 `+x` 权限。
2. 校验全局/项目 `config.toml` 语法合法，且默认 Provider 凭据就绪（`api_key_env` 对应变量非空或 `api_key` 非空；两者均显式为空时视作免鉴权端点放行）。
3. 校验 `/dev/fd` 挂载可用且匿名管道转发正常。
4. 校验 `$XDG_RUNTIME_DIR` 权限与 `flock` 文件锁可用性。
