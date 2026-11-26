# Anix Core（POSIX 原生 Agent 内核）v0.4 技术规范

**版本**：v0.4.0-rfc  
**状态**：Working Specification（`0.x` 演进期草案；`v1.0.0` 正式定稿前保留契约收口空间）  
**适用范围**：`anix-core` 内核与全部外部线协议（参考运行底座组件见 [RUNTIME.md](RUNTIME.md)）  
**协议版本**：`ir_version = 1`  
**目标平台**：Linux (`x86_64` / `arm64`)、macOS (Apple Silicon / Intel)、WSL2  

---

## 1. 设计哲学、四层架构边界与演进约定（Normative）

### 1.1 规范性关键词（RFC 2119）

本文档中的关键词 **必须（MUST）**、**严禁（MUST NOT）**、**应当（SHOULD）**、**可以（MAY）** 均按 RFC 2119 解释。凡标注“必须/严禁”的条款，参考实现与第三方兼容实现均不得偏离。

### 1.2 核心定位：三原语极简内核

Anix 将传统应用层 Agent Harness 溶解进 POSIX 操作系统。**内核（`anix-core`）只拥有三样东西：一个 `*.agent` 文件、一条树状 WAL、一个 Maildir。所有唤醒都是“有消息到了”，所有回流都是 `anix send`。**

* **Agent 即标准 POSIX 进程**：每个 `*.agent` 文件通过 Shebang 直接执行，与 `grep`、`awk`、`jq` 享有同等进程、管道、重定向与信号语义。
* **两轴正交内核**：内核只保留 **`shell`**（能力轴，支持任意包装器链如 `anix-approve -- msh`）与 **`output`**（静态 `fd 1` 输出契约轴）；**严禁**在内核内置审批 UI/策略、LLM SDK、MCP 客户端、技能/指南 Prompt 注入、视觉转译器、安全分类器、后台孙进程 `pidfd` 监视、Socket 服务器或项目目录隐式发现。
* **机制与策略彻底分离**：Shell 包装链（含审批、输出过滤与后台 reaper）、LLM 驱动（`[providers.<name>]`）、上下文摘要策略（`[compact].bin`）与上下文前置钩子（`hooks/pre-context.d/`）均通过语言无关的 POSIX 子进程线协议解耦。
* **确定性边界包裹概率性内核**：除模型推理本身外，所有 I/O 通道、FD 契约、Maildir 生命周期、WAL 校验、错误码、超时与权限位检查均遵循确定性行为（Fail-Fast / Fail-Closed）。

### 1.3 四层递进架构与规范边界

Anix 生态按**逐级递进、严格单向依赖**划分为四层；**本 RFC 仅规范 Layer 1（`anix-core`）的强制契约与跨进程线协议**：

| 层级 | 名称 | 归属与规范定位 | 职责边界 |
| :--- | :--- | :--- | :--- |
| **Layer 1** | **`anix-core`** | **本文档（Normative）** | 仅包含 `anix` 核心解释器与 `anix-proto` 线协议契约：Agent 文件、树状 WAL（含跨文件 `fork`/`fsck`/`log`）、Maildir 统一事件循环（`--inbox`/`send`/`--wait`/`--notify`/`hold` 租约）、FD 契约、信号与 Provider/Compact/`pre-context.d` 协议。**零依赖**任何具体 Shell、审批策略、LLM SDK 或 TUI。 |
| **Layer 2** | **`anix-runtime`** | [RUNTIME.md](RUNTIME.md)（Informative） | 开箱即用的参考发行包：`anix` + `msh`（含后台 reaper）+ `anix-approve`（Shell 审批包装器）+ `a*`（`aclr`/`aview`）+ `anix-provider` + 独立钩子脚本（`20-skills`/`30-tips`）+ 安装分发。所有非 Core 组件均可按本 RFC 自由替换。 |
| **Layer 3** | **`anix-agent`** | **独立分仓**（非本 RFC 范围） | 面向人类日常工作流的交互式高阶控制壳（类 Claude Code / Pi）：TUI、可视化审批弹窗、多会话/分支浏览、团队编排、Diff 预览。完全通过本 RFC 的外部协议（`--trace-fd`、`-c`/`--leaf`/`fork`、`--inbox`/`anix send --wait`、`shell` 包装链）驱动底层，不限定实现语言。 |
| **Layer 4** | **`anix-*`** | **独立分仓**（非本 RFC 范围） | 第三方与社区生态项目（自定义 Provider、审批策略引擎、Socket 桥接、Trace 采集面板、领域 `*.agent` / Skills 包等），只需遵守本 RFC 线协议即可即插即用。 |

### 1.4 `0.x` 演进与兼容性声明

* **版本编号约定**：本规范处于 **`0.x` 演进阶段**（`v0.0` → `v0.1.x` → `v0.2.0` → `v0.3.0` → `v0.4.0`），线协议版本字段维持 `ir_version = 1` 不变。
* **`v0.3.0 → v0.4.0` 纯净 Unix 内核定型（重大契约收口）**：
  1. **唤醒与 IPC 彻底收敛至 Maildir**：新增 `anix send <MAILDIR> [--wait]`（借助临时 `reply-to` Maildir 统一异步投递与同步一问一答）、`--notify <MAILDIR>`、`ANIX_INBOX` 只写方向注入、`--lock-wait[=SEC]`；事件循环强制支持批量取空 + 去抖 + 全量 rescan 兜底 + 60s 安全扫描 + `new → cur → WAL fsync(message-id) → done` 崩溃去重；`$ANIX_CTL` 以通用 `hold`/`release` 租约帧取代 `msh` 专属的 `bg_job` 帧（`--idle-timeout` 仅在无活跃 `hold` 且队列为空时计时）。
  2. **树状 WAL 工具链与跨文件并发分叉**：新增 `anix fork <SRC> [--leaf <ID>] -o <DST>`（通过首行 `branch_base` 只读引用父链，单文件单写者、多文件并行分支）、`anix fsck <WAL>` 与 `anix log <WAL> [--leaf <ID>]`；明确 WAL `flock` 仅承诺本地 POSIX 文件系统。
  3. **内核去脂（剥离 Harness 策略至 Layer 2/3）**：
     * 砍除 `approval` 轴、`[approval].bin` 与内置 `/dev/tty` 审批（改为 `shell` 包装链 `anix-approve -- msh`，Core 仅留 `shell` + `output` 两轴）；
     * 配置收口为 **4 层显式优先级**（`CLI > Env > Frontmatter > 单一显式配置文件`），移除向上遍历 `.anix/config.toml`；
     * 移除内置 `20-skills`/`30-tips`（改为 Layer 2 独立 `pre-context.d/` 脚本）、移除 `[vision].bin`（媒体帧透传交由 Provider 处理）、移除 `post-tool.d/`（由 Shell 包装链处理）与 `on-exit.d/`（由 `--notify` 与父进程 `wait` 覆盖）；
     * 移除 `anix trust` SHA-256 数据库，Core 改用类 `ssh StrictModes` 的 POSIX 属主与权限位门禁；
     * 移除内核中硬编码的 `"msh"` 工具名（默认工具名 `"shell"`，支持 Frontmatter `tool_name` 覆盖，且 `shell` 支持带参包装器链）；
     * 环境变量 `$ANIX_OUT`/`$ANIX_ERR`/`$ANIX_CTL` 为唯一权威接口，内部 FD 号为实现细节；`SIGUSR1` 补充等价控制消息 `Anix-Control: compact`。
  4. **历史 WAL 兼容**：保留对 `v0.1`–`v0.3` 线性与树状 WAL 文件的逆读兼容（未知历史 `config` 字段忽略，缺失 `id` 回退线性模式）。

---

## 2. 外部组件解析与执行边界（Normative）

`anix` 内核不内置具体 Shell、审批器、LLM SDK 或增强脚本，所有外部可执行组件均遵循统一的确定性解析契约：

1. **组件可执行路径解析顺序**：`anix` 解析 `shell` 的首词可执行文件以及配置中的外部可执行文件（`[providers.<name>].bin`、`[compact].bin`）时：
   * 若值包含 `/`：展开前导 `~/` 后按路径解析（绝对路径直接使用；相对路径按当前工作目录解析）；
   * 若值不含 `/`：先按用户 `$PATH` 依次查找具备 `+x` 权限的文件，未命中时再回退私有组件目录 `$HOME/.local/libexec/anix/<name>`（开发未安装态可回退同目录兄弟可执行文件）。
2. **工具目录前置注入**：当 `shell != "none"` 时，`anix` 拉起 Shell 子进程前必须将私有工具目录（默认 `$HOME/.local/libexec/anix/tools`）前置（prepend）到子进程 `PATH` 中，避免内部辅助工具污染人类全局 `$PATH`。
3. **无反向依赖铁律**：任何满足本 RFC 第 4、5、6、7、8 章线协议的 Shell、包装器、Provider 或钩子均可无修改替换参考发行包中的对应组件。

---

## 3. CLI 接口与 `*.agent` 文件规范（Normative）

### 3.1 CLI 调用语法

```sh
anix [OPTIONS] <SCRIPT.agent> [--] [<SCRIPT_ARGS>...]
anix send <MAILDIR> [--wait [--timeout <SEC>]] [--in-reply-to <ID>] [--control <OP>] [--from <LABEL>] [MSG...]
anix fork <SRC_WAL> [--leaf <ID>] -o <DST_WAL>
anix fsck <WAL>
anix log <WAL> [--leaf <ID>]
anix --doctor
anix --version
```

`--` 可选，仅在传递以 `-` 开头的裸参数且不希望被 `anix` 解析为自身选项时用于分隔。  
核心 CLI 选项（优先级最高）：

* `--config <FILE>`：显式指定单一 `config.toml` 路径（未传时回退 `$ANIX_CONFIG`，再回退 `${XDG_CONFIG_HOME:-$HOME/.config}/anix/config.toml`）。
* `-c, --context <FILE>`：指定持久化 Tree WAL JSONL 文件路径；传空串 `""` 强制无状态纯内存运行。
* `--leaf <ID>`：仅配合 `-c <FILE>` 使用；从指定历史 WAL 节点 `id`（`uint64 >= 1`）原地分叉续跑，缺省沿用文件末尾有效叶节点。
* `--lock-wait[=<SEC>]`：打开 WAL 获取 `flock(LOCK_EX)` 冲突时阻塞等待而非立即退出 `1`；省略秒数时无限等待（可被 `SIGINT`/`SIGTERM` 中断），指定秒数超时未获锁以退出码 `124` 退出。
* `--provider <NAME>` / `--model <ID>` / `--shell <CMD|none>`（支持带参包装链，如 `"anix-approve -- msh"`；`none` 禁用工具进入纯文本模式）。
* `-o, --output <reply|explicit|none>`：声明 `fd 1`（`stdout`）唯一静态输出契约（默认 `reply`，见 §4.1）。
* `--max-turns <N>` / `--max-tokens <N>` / `--max-output-tokens <N>` / `--compact-threshold <VAL>` / `--no-compact`
* `--param <KEY=VALUE>`：透传给 Provider 的额外模型参数（可重复指定；值优先按 JSON 字面量解析，否则作为字符串）。
* `--stderr <auto|explicit>` / `-q, --quiet`（等价于 `--stderr explicit`）
* `--trace-fd <N>`：显式指定结构化 Trace（JSONL）输出的目标文件描述符编号（`>= 3`，见 §4.1、§8.4）。
* `--inbox <DIR>`：进入常驻 Maildir 队列消费模式（可与初始输入共存：先处理初始输入作为首轮，再进入事件循环）。**严禁**从环境变量 `ANIX_INBOX` 读取此选项。
* `--idle-timeout <SEC>`：常驻模式闲置自动退出秒数（默认 `300`；`0` 表示永不因闲置退出）。仅在本轮结束、`new/` 为空且**无未到期 `hold` 租约**时开始计时。
* `--inbox-debounce-ms <MS>`：常驻模式唤醒后的批量合并去抖窗口毫秒数（默认 `100`；`0` 表示立即消费）。
* `--notify <MAILDIR>`：进程退出前（覆盖正常退出、`1`、`75`、`124`、`130`、`143`）原子向 `<MAILDIR>` 投递退出状态与本轮最终输出摘要（可配合 `--notify-id <ID>` 写入 `Anix-In-Reply-To` 头以释放调用方 `hold` 租约）。
* `--sandbox[=best-effort|strict]`：启用 Linux Landlock 沙箱。

### 3.2 `*.agent` 文件格式与解析规则

1. **编码与行尾**：文件必须为合法 UTF-8；同时兼容 `\n` 与 `\r\n`。
2. **Shebang**：首行以 `#!` 开头时忽略该行。
3. **YAML Frontmatter**：Shebang 之后（或首行）若出现独占一行的 `---`，则至下一个独占一行的 `---` 之间作为 YAML 1.2 解析；其余文本（去首尾空行）作为 `agent_prompt`。无 `---` 块时全文作为 `agent_prompt`。
4. **未知字段处理（Fail-Closed）**：Frontmatter 中出现未定义字段（除 `x-*` 扩展命名空间外）时，必须向 `fd 2` 报错并以退出码 `2` 退出。

示例：

```markdown
#!/usr/bin/env anix
---
name: security-auditor
provider: openai-responses
model: deepseek-v4.1-flash
shell: anix-approve -- msh
output: reply
max_turns: 15
max_tokens: 128000
compact_threshold: 0.8
stderr: auto
---
你是一个安全审计专家。读取传入的代码变更，分析潜在的并发竞态、注入与越权漏洞，输出高危漏洞的 JSON 数组。
```

### 3.3 Frontmatter 字段定义

| 字段 | 类型 | 默认值 | 确定性约束 |
| :--- | :--- | :--- | :--- |
| `name` | string | 文件名（去后缀） | 仅允许 `[a-zA-Z0-9._-]`；用于 Trace 与日志标识。 |
| `provider` | string | 全局 `defaults.provider` | 必须匹配配置文件中已定义的 `[providers.<name>]`。 |
| `model` | string | 全局 `defaults.model` | 非空字符串，传给 Provider 驱动。 |
| `shell` | string | `"msh"` | 能力轴：可执行命令（可含包装器参数链，如 `"anix-approve -- msh"`），或字面量 `"none"`（禁用工具，`tools: []`）。 |
| `tool_name` | string | `"shell"` | 暴露给模型的 Shell 工具名称（仅允许 `[a-zA-Z0-9_-]`；不得在内核硬编码具体发行版外壳名）。 |
| `output` | enum | `"reply"` | 契约轴：仅允许 `reply` \| `explicit` \| `none`，静态声明 `fd 1`（`stdout`）唯一写者（见 §4.1）。 |
| `max_turns` | uint32 | `20` | 必须 `>= 1`；达到上限且任务未完成时以退出码 `124` 退出。 |
| `max_tokens` | uint32 | `128000` | 必须 `>= 1024`；上下文硬上限，超出且无法压缩时以退出码 `124` 退出。 |
| `max_output_tokens` | uint32 | `0`（由上游决定） | 为 `0` 或省略时不写入 IR；若 `>= 1` 则必须 `< max_tokens` 并透传至 IR `header.max_output_tokens`。 |
| `compact_threshold` | float \| uint32 | `0.8` | `(0.0, 1.0)` 表示 `floor(max_tokens * val)`；`>= 1` 表示绝对 Token 数；`0` 或 `>= max_tokens` 或 `--no-compact` 禁用自动压缩。 |
| `stderr` | enum | `"auto"` | 仅允许 `auto` \| `explicit`。 |
| `context` | string | `""`（无状态） | 相对路径按当前工作目录（`cwd`）解析；为空时严格无状态。 |
| `params` | object | `{}` | 开放键值对透传字典，浅合并后原样透传至 IR `header.params` 交由 Provider 处理。 |

### 3.4 四层显式配置优先级与环境变量隔离

配置合并严格遵循以下 **4 层显式优先级**（高覆盖低；严禁向上级目录隐式搜寻项目 `.anix/config.toml`）：  
`CLI 参数 > 环境变量 > *.agent Frontmatter > 单一显式配置文件（--config / $ANIX_CONFIG / ~/.config/anix/config.toml） > 内置默认值`

标准环境变量映射：

* `ANIX_CONFIG`、`ANIX_HOOKS_DIR`、`ANIX_PROVIDER`、`ANIX_MODEL`、`ANIX_SHELL`、`ANIX_OUTPUT`、`ANIX_MAX_TURNS`、`ANIX_MAX_TOKENS`、`ANIX_MAX_OUTPUT_TOKENS`、`ANIX_COMPACT_THRESHOLD`、`ANIX_STDERR`、`ANIX_LOG`、`ANIX_TRACE_FD`、`ANIX_TRACE_LEVEL`、`ANIX_CONTEXT`、`ANIX_LEAF`、`ANIX_IDLE_TIMEOUT`。
* **`ANIX_INBOX` 永不作为自身配置读取（硬铁律）**：`ANIX_INBOX` 仅由运行在 `--inbox` 模式的父 `anix` 主动注入给 Shell 子进程作为回流投递地址；任何 `anix` 进程启动时 **严禁（MUST NOT）** 将 `ANIX_INBOX` 解析为自身的 `--inbox` 目录。
* **空串覆盖规则**：`ANIX_CONTEXT=""` 或 `-c ""` 必须覆盖 Frontmatter 中的 `context`，强制本次运行无状态。
* **启动期静态校验（Fail-Fast）**：在配置合并完成后、打开/写入 WAL 之前执行确定性校验，违规立即向 `fd 2` 报错并以退出码 **`2`** 退出：
  1. `output`、`stderr` 枚举值非法或 `tool_name` 非法 → 退出 `2`；
  2. `output == explicit` 且 `shell == none` → 退出 `2`；
  3. `shell != none` 但首词可执行文件无法解析或无 `+x` 权限 → 退出 `2`；
  4. 指定 `--leaf <ID>` / `ANIX_LEAF` 但无有效 `-c <FILE>`，或 `<ID> == 0` / 非法整数，或目标 WAL 为空、旧线性格式、节点缺失、指向分节头、或破坏工具闭合 → 退出 `2`；
  5. **非阻断警告（输出至 `fd 2`）**：`output == none && shell == none`，以及 `output != explicit` 但脚本正文出现 `ANIX_OUT`。
* **子进程隔离规则（默认清空 + 白名单透传 + 主动注入）**：`anix` 拉起 Shell 子进程前必须**清空所有 `ANIX_*` 环境变量**，仅按以下规则向下传递：
  * **观测白名单透传**：若父环境存在 `ANIX_LOG` 或 `ANIX_TRACE_LEVEL`，原样透传（`ANIX_TRACE_FD` 绝不透传）；
  * **只写回流地址主动注入**：仅当父进程运行在 `--inbox <DIR>` 模式时，主动注入 `ANIX_INBOX=<abs_dir>`；
  * **侧信道主动注入**：仅当 `output == explicit` 时注入 `ANIX_OUT`（指向已挂载匿名写端管道），否则移除 `ANIX_OUT` 并显式关闭对应 FD；始终注入 `ANIX_ERR` 与 `ANIX_CTL`；
  * **通用调用参数注入**：如工具入参提供 `timeout_sec` / `kill_on_timeout`，按配置裁剪后注入 `ANIX_EXEC_TIMEOUT_SEC` 与 `ANIX_EXEC_KILL_ON_TIMEOUT`（并注入完整工具入参 JSON 至 `ANIX_TOOL_ARGS`）；
  * 非 `ANIX_` 前缀变量（如 `OPENAI_API_KEY`）保持常规继承。
* **`params` 浅合并规则**：按 `CLI --param > Frontmatter params > 配置文件 [providers.<name>.params]` 键级浅合并。

---

## 4. I/O 协议、FD 分配与 Maildir 统一事件循环（Normative）

### 4.1 外部 `anix` 进程 FD 契约

| FD | 方向 | 角色 | 确定性行为规范 |
| :--- | :--- | :--- | :--- |
| **`fd 0`** | R | 主输入（stdin） | 阻塞读取全部字节直至 EOF。CLI 位置参数与 `fd 0` 内容均保持纯文本（不加外层包装标签；与 `fd 4..=9` 各非空段之间以 `\n\n` 连接）。当 `isatty(0)` 为真且已传入位置参数（或 `--inbox` 模式下未传位置参数）时，**严禁阻塞等待 `stdin` EOF**。 |
| **`fd 1`** | W | 纯净 Payload（stdout） | 遵循静态单写者 **`output`** 契约：<br>• **`output = reply`（默认）**：仅输出本轮周期成功结束（无 `tool_calls` 正常 `done`）时最终轮的 assistant 正文（不再重复写入 `fd 2`）。`shell = none` 时实时流式直出 `fd 1`；`shell != none` 时按轮缓冲，确认无 `tool_calls` 后整块写出 `fd 1`（否则按 `stderr` 模式刷至 `fd 2`，`fd 1` 保持 0 字节）。仅当 `isatty(1)` 为真、内容非空且末尾非 `\n` 时补 1 个 `\n`；非 TTY 字节级原样输出。<br>• **`output = explicit`**：仅转发子进程写入 `$ANIX_OUT` 的原始字节流，严禁写入模型文本、日志或自动补换行。<br>• **`output = none`**：严禁向 `fd 1` 写入任何字节（严格 0 字节）。 |
| **`fd 2`** | W | 人类诊断流（stderr） | `auto` 模式：接收中间文本、思考摘要、工具执行状态、`$ANIX_ERR` 内容、致命错误及静态 ANSI Dim `[usage]` 汇总行（阶段边界与退出前强制 `\n` 收尾，ANSI 重置必须在 `\n` 之前）。`explicit` 模式（`-q`）：仅输出 `$ANIX_ERR` 与致命错误，不输出 `[usage]`。 |
| **`--trace-fd=<N>`** | W（可选） | 结构化 Trace（JSONL） | 严禁默认探测 `fd 3`。仅当显式指定 `--trace-fd=<N>` 或 `ANIX_TRACE_FD=<N>`（`N >= 3`）时校验可写并输出 JSONL；无效时退出 `2`。 |
| **`fd 4..=9`** | R（可选） | 静态多路输入 | 启动时按序探测 `fd 4..=9`（跳过 `--trace-fd`）：凡可读者阻塞读至 EOF，包装为 `<input fd="N">\n...\n</input>` 附加在主输入之后，读毕立即关闭。**注意**：外部 `fd 4..=9` 是只读静态输入，与内部 Shell 子进程侧信道完全隔离、语义独立。 |

### 4.2 内部 Shell 子进程 FD 契约（环境变量权威原则）

`anix` 拉起 Shell 子进程时构造独立进程组（`setpgid(0, 0)`）与以下通道；**环境变量（`$ANIX_OUT` / `$ANIX_ERR` / `$ANIX_CTL`）是唯一权威接口**，底层 FD 编号属于内核实现细节，外部脚本 **必须（MUST）** 使用环境变量重定向（如 `> "$ANIX_OUT"`），**严禁**硬编码数字 FD 避免与用户脚本 `exec 4>` 冲突：

| 环境变量 | 方向 | 底层实现与防截断语义 |
| :--- | :--- | :--- |
| **`fd 0`** | R | 绑定 `/dev/null`（严禁继承外部 stdin）。 |
| **`fd 1, fd 2`** | W | 绑定主进程捕获管道，作为工具结果返回模型。 |
| **`ANIX_OUT`**（仅 `output=explicit`） | W | 指向匿名 `pipe()` 写端（如 `/dev/fd/4`），主进程中转至外部 `fd 1`。当 `output != explicit` 时移除该变量并显式关闭对应 FD。经匿名管道中转自动规避普通文件重定向时的 `O_TRUNC` 覆盖陷阱。 |
| **`ANIX_ERR`** | W | 指向匿名 `pipe()` 写端，主进程中转至外部 `fd 2`。 |
| **`ANIX_CTL`** | W | 指向匿名 `pipe()` 写端，供子进程写入单行 UTF-8 JSONL 控制帧（单帧上限 64 KB，见 §6.2）。 |

### 4.3 统一 Maildir IPC、消息信封与事件循环（`--inbox` / `anix send` / `--notify`）

Maildir 是 `anix-core` 唯一的进程间通信与唤醒原语。

#### 4.3.1 Maildir 目录与消息信封格式

1. **目录结构**：`<MAILDIR>/{tmp,new,cur,done}`（不存在时以权限 `0700` 自动创建）。
2. **原子投递契约（`anix send <MAILDIR>`）**：先写入 `tmp/<ts_ns>-<pid>-<rand>`，执行 `fsync`（`sync_data()`）关闭文件后，通过原子 `rename()` 移入 `new/`。
3. **可选头部信封（纯文本完全兼容）**：
   * 若消息首行不以 `Anix-` 头开头，整个文件内容直接视作纯文本正文，`message_id` 取文件名。
   * 若消息顶部包含 `Anix-*:` 头行（每行 `Key: Value`，以首个空行 `\n\n` 与正文分隔），内核解析以下标准头：
     * `Anix-Message-Id: <id>`：全局唯一消息 ID（用于崩溃去重与 `hold` 匹配；省略时回退文件名）；
     * `Anix-Reply-To: <maildir_path>`：同步应答目标目录（`anix send --wait` 使用）；
     * `Anix-In-Reply-To: <id>`：关联的租约/消息 ID（收到时自动释放匹配的 `hold` 租约）；
     * `Anix-From: <label>`：可选发送方标识；
     * `Anix-Control: <op>`：内核控制指令（支持 `compact`，等价于 `SIGUSR1`）；
     * `Anix-Exit-Code: <i32>`：由 `reply-to` 响应或 `--notify` 携带的退出码。

#### 4.3.2 同步一问一答（`anix send --wait [--timeout <SEC>]`）

1. 发送方创建私有临时回复目录 `<reply_dir>`（权限 `0700`），在消息头写入 `Anix-Reply-To: <reply_dir>` 后原子投递至目标 `<MAILDIR>/new/`；
2. 目标常驻 Agent 完成包含该消息的推理循环后，将本轮 `fd 1` 交付内容及退出码原子投递至 `<reply_dir>/new/`（携带 `Anix-In-Reply-To` 与 `Anix-Exit-Code`）；
3. 发送方事件驱动等待 `<reply_dir>/new/`（含 rescan 兜底），收到回复后将正文原样写入自身 `stdout`，清理 `<reply_dir>` 并以对端 `Anix-Exit-Code` 退出（超时以退出码 `124` 退出）。

#### 4.3.3 统一事件循环、批量去抖、租约与崩溃去重

当指定 `--inbox <DIR>` 时：
1. **初始输入共存**：若启动时同时提供了位置参数 / `fd 0` / `fd 4..=9` 非空输入，先将其作为首轮推理执行并提交 WAL，再进入 Maildir 循环。
2. **启动崩溃去重（`cur/` 恢复）**：进入主循环前，先按 ASCII 序扫描 `cur/`：
   * 若某消息的 `message_id` 已存在于当前 WAL 活跃分支（已提交落盘），直接原子移入 `done/`（不重复执行）；
   * 否则将其作为待处理消息进入首轮恢复批次。
3. **事件监听 + 兜底 rescan**：使用 `inotify`（Linux）/ `kqueue`（macOS/BSD）监听 `new/`；同时由于容器 overlayfs / bind mount 存在丢事件与 `IN_Q_OVERFLOW` 风险，**必须**在每次事件唤醒后全量 rescan `new/`，并保留低频安全扫描超时（默认 `60s`，无活跃事件时仍极低功耗）。
4. **去抖与批量取空（防事件风暴）**：检测到 `new/` 非空后，等待 `--inbox-debounce-ms`（默认 `100ms`）合并突发到达的并发消息，随后将当前 `new/` 中**全部**文件按 ASCII 序原子移至 `cur/`，合并为单条 `role: "user"` 批次输入（单条消息纯文本或 `<input source="...">`，多条按序分段）触发**一轮**推理。
5. **严格提交屏障**：`new → cur` → 追加 WAL（记录 `message_ids`）并 **`fsync`** → 执行推理并提交 WAL `fsync` → 移入 `done/` → 投递所有 `Anix-Reply-To` 响应 → 发出 `cycle_end` Trace 事件。
6. **`hold` 租约与 `--idle-timeout` 精确语义**：
   * 工具通过 `$ANIX_CTL` 发送 `{"op":"hold","id":"<id>","deadline_sec":<N>,"label":"..."}` 登记租约（可选 `{"op":"release","id":"<id>"}` 主动释放）；
   * 收到带 `Anix-In-Reply-To: <id>` 的消息时自动释放租约 `<id>`；
   * 若租约到达 `deadline` 仍未释放，内核自动合成一条 `<runtime_update>Hold expired: ...</runtime_update>` 唤醒事件并触发新一轮推理（兜底外部进程/reaper 被 `kill -9`）；
   * **`--idle-timeout <SEC>` 计时规则**：仅当本轮推理结束、`new/` 为空且**当前无任何未到期 `hold` 租约**时开始计时（有 `hold` 时绝不空闲退出）；若超时仍无新消息，优雅关闭 Provider、释放锁并以 `0` 退出（`0` 表示永不因闲置退出）。

#### 4.3.4 退出通知（`--notify <MAILDIR>`）

指定 `--notify <MAILDIR>` 时，进程在退出前（无论 `0`、`1`、`75`、`124`、`130`、`143`）必须向 `<MAILDIR>` 原子投递一条通知消息（携带 `Anix-Exit-Code`、可选 `Anix-In-Reply-To`（来自 `--notify-id`）、`Anix-From` 及本轮最终回复/错误摘要）。

### 4.4 核心契约注入钩子（`00-posix-runtime`）

`00-posix-runtime` 默认内置于 `anix` 二进制中（可被 `hooks/pre-context.d/00-posix-runtime` 覆写或 0 字节 / `/dev/null` 软链屏蔽），在首轮组装冻结 `system_context` 时按 `(output, shell, stderr, tool_name)` 注入极简 `<posix_runtime>` 契约（声明 `fd 1` 输出契约、`$ANIX_OUT`/`$ANIX_ERR`/`$ANIX_CTL` 只写管道、以及如处于 `--inbox` 模式时的 `$ANIX_INBOX` 投递地址）。

---

## 5. 上下文管理、Tree WAL、分叉工具链、信号与退出码（Normative）

### 5.1 自动上下文压缩、硬上限熔断与折叠原语

1. **Turn ID 锚点规则**：活跃分支维护单调递增 `turn_id`（从 `1` 开始）；工具输出由内核统一拼接 `[t<turn_id>] exit=<code>\n` 锚点。
2. **每轮推理前预算检查（有效上下文水位 `T_cur`）**：以活跃窗口最后一条带 `usage` 的存活 `assistant` 的 `input_tokens + output_tokens` 加其后消息 `ceil(bytes / 3)` 估算（无存活 `assistant` 时对整个活跃窗口执行 `ceil(bytes / 3)`）；压缩后或禁用压缩仍 `T_cur > max_tokens` 时以退出码 **`124`** 熔断。
3. **显式压缩与尾部折叠控制帧语义（通过 `$ANIX_CTL`）**：
   * `{"op":"compact"}`（以及 `SIGUSR1` 或 `Anix-Control: compact`）：全量压缩，写入 `checkpoint` 分节并重新执行 `pre-context.d/` 刷新冻结 `system_context`。
   * `{"op":"fold","from_turn":N,"summary":"..."}` / 省略 `summary` 自动摘要：边界校验 + 工具对闭合对齐后，写入一条 `parent_id` 指向 `aligned_from` 之前保留节点的 `fold` 节点（含可选 `fold_usage` 与 `pruned_usage`），不修改头部冻结前缀。
4. **可覆写压缩策略（`[compact]`）**：优先 `[compact].bin`（外部自定义压缩器，受 §7.2 `StrictModes` 校验），否则使用内置 Provider 压缩器（`tools: []`）。

### 5.2 树状持久化上下文（Tree WAL）、跨文件 `fork` 与工具链

`anix` 本体不维护中心化数据库；`-c <file>` 采用单文件 **Append-Only 树状 JSONL（Tree WAL）** 格式。

1. **本地文件系统与 `flock` 边界**：打开 `<file>` 时必须调用 `flock(LOCK_EX | LOCK_NB)`（或 `--lock-wait[=SEC]` 阻塞等锁）。**规范明确声明**：`flock` 并发保护与 WAL 原子性**仅承诺在本地 POSIX 文件系统上成立**（不承诺 NFS、SMB 或容器跨宿主机共享卷）。
2. **树节点拓扑与不可变 Prefix**：
   * 新建 WAL 中每条事件携带文件级严格单调递增的 `id`（`uint64 >= 1`）与可选 `parent_id`（`1 <= parent_id < id`，根节点省略）；
   * 首轮写入 `session_init`（`id: 1`）与冻结 `role: "system"`（`id: 2, parent_id: 1`）；Config Drift 在当前分支尾部追加 `runtime_update`；`fold` 通过 `parent_id` 直接回挂祖先节点；`checkpoint` 作为分支上的压缩屏障，跨 `checkpoint` 仍可通过 `--leaf` 回到旧分支。
   * **WAL 崩溃落盘**：每轮或每批 Maildir 消息写入后执行 `flush` + `sync_data()`（`fsync`）；消费自 `--inbox` 的 `message` 事件携带可选 `message_ids: string[]` 供崩溃重启去重。
3. **跨文件并行分叉（`anix fork`）**：
   * 为在不引入细粒度锁的前提下支持真正的多进程并行分支探索，`anix fork <SRC_WAL> [--leaf <ID>] -o <DST_WAL>` 创建新 WAL 文件，首行写入 `branch_base` 节点：
     ```json
     {"ir_version":1,"type":"branch_base","id":1,"base":"/abs/path/to/src.jsonl#4","ts":"2026-11-27T00:00:00Z","session_id":"...","config":{...}}
     ```
   * 逆读 `<DST_WAL>` 到达 `branch_base` 时，内核以**只读无写锁**方式打开 `base` 指向的 `<SRC_WAL>` 并从 `<leaf_id>` 继续沿 `parent_id` 逆读父链（支持多级 `branch_base` 链，内置环检测防死循环）；
   * 每个分支拥有独立文件与独立 `flock`，互不阻塞、零全量拷贝。
4. **WAL 检查与检视工具链（`anix fsck` / `anix log`）**：
   * `anix fsck <WAL>`：全量校验 UTF-8、JSON 语法、`ir_version`、单调 `id`、拓扑序 `parent_id < id`、工具对闭合性、`branch_base` 可达性，异常返回非零退出码；
   * `anix log <WAL> [--leaf <ID>]`：输出树拓扑或指定分支链路摘要（`id`、`parent_id`、`type`/`role`、`turn_id`、时间与首行摘要）。
5. **`ir_version` 迁移契约**：同一 `ir_version = 1` 内的新增字段必须保持可选前向兼容；若未来升级至 `ir_version = 2`，内核遇到未知主版本必须 Fail-Closed 退出 `2`，并由独立离线迁移工具转换，严禁运行时隐式原地改写旧 WAL。
6. **单遍逆读回放与尾行 Fail-Closed**：
   * 尾行损坏一律 Fail-Closed 退出 `1`；
   * Tree Mode 沿 `want_id -> parent_id` 单遍逆读，O(1) 跳过兄弟分支；显式 `--leaf <ID>` 对缺失/旧线性格式/分节头/未闭合 `tool_calls` 严格退出 `2`；默认续跑对未闭合 `tool_calls` 执行崩溃自愈并将 `head_id` 回退至最后存活节点；缺失 `id` 的历史线性 WAL 自动回退旧版线性回放。

### 5.3 POSIX 信号处理规范

| 信号 | 确定性行为 |
| :--- | :--- |
| **`SIGINT`** | 向当前活跃子进程组发送 `SIGTERM`（最多等待 3 秒后 `SIGKILL`），丢弃未提交半成品 Turn，执行 `--notify`（如配置），释放 `flock`，以 **`130`** 退出。 |
| **`SIGTERM`** | 同 `SIGINT` 优雅清理流程，以 **`143`** 退出。 |
| **`SIGSTOP` / `SIGCONT`** | 遵循内核默认行为。 |
| **`SIGUSR1`** | 置位原子标志，下一轮 Provider 调用前触发全量压缩（另有等价 Maildir 控制消息 `Anix-Control: compact`）。 |
| **`SIGUSR2`** | 将当前内存 Canonical IR JSONL 快照原子写入 `/tmp/anix_<pid>_<epoch_ms>.ctx.jsonl`（权限 `0600`），在 `fd 2` 打印路径。 |

### 5.4 标准退出码表

| 退出码 | 符号语义 | 触发条件 |
| :--- | :--- | :--- |
| **`0`** | `EX_OK` | 任务正常完成（或 `--inbox` 闲置优雅退出）。 |
| **`1`** | `GENERAL_ERR` | 通用运行时错误：Provider 网络/协议失败、I/O 异常、WAL 损坏、`flock` 锁冲突（未启用 `--lock-wait`）、`fsck` 检出损坏。 |
| **`2`** | `EX_USAGE` | 命令行参数非法、无效 `--trace-fd`、无效 `--leaf`、启动静态校验失败、Frontmatter/`config.toml` 语法或未知字段错误、`ir_version` 不兼容。 |
| **`75`** | `EX_TEMPFAIL` | 门禁拦截：钩子或外部可执行文件未通过 POSIX `StrictModes` 属主/权限位校验，或 Shell 包装器（如 `anix-approve`）返回退出码 `75` 请求挂起/外部介入。 |
| **`124`** | `EX_TIMEOUT` | 预算或超时熔断：`max_turns`、`max_tokens`、Provider 超时、`--lock-wait` 超时、`anix send --wait --timeout` 超时。 |
| **`128+N`** | `SIGNAL` | 被信号 `N` 终止（`130` = `SIGINT`，`143` = `SIGTERM`）。 |

---

## 6. Shell 执行器协议与 `$ANIX_CTL` 侧信道规范（Normative）

### 6.1 Shell 执行器子进程调用协议

当 `shell != "none"` 时，内核向模型暴露单一通用工具（名称由 `tool_name` 指定，默认 `"shell"`；参数 `cmd` 必填，可选 `timeout_sec` / `kill_on_timeout` 等透传参数），并将 `shell` 拆分为 `<program> <prefix_args...>` 按以下契约执行：

1. **调用形式**：`<program> <prefix_args...> -c "<cmd>"`，工作目录为当前 `cwd`，独立进程组（`setpgid(0, 0)`）。
2. **环境与 FD 映射**：严格遵循 §3.4 与 §4.2（清空 `ANIX_*`，注入白名单、`$ANIX_INBOX`（若常驻）、`$ANIX_OUT`（若 `output=explicit`）、`$ANIX_ERR`、`$ANIX_CTL`、`$ANIX_TOOL_ARGS` 与可选 `$ANIX_EXEC_TIMEOUT_SEC`/`$ANIX_EXEC_KILL_ON_TIMEOUT`）。
3. **退出码映射**：
   * 捕获输出拼接 `[t<turn_id>] exit=<code>\n` 返回模型；
   * 若 Shell 包装器返回 `75`（`EX_TEMPFAIL`），由包装器策略决定是否透传为工具错误或由外部包装器自身拦截；若子进程返回 `124` 则置 `is_error = true`。

### 6.2 统一侧信道协议（`$ANIX_CTL`）

子进程可向 `$ANIX_CTL` 写入单行 UTF-8 JSONL 控制帧（单帧上限 64 KB）：

* **多模态附加帧（`media`）**：
  ```json
  {"op":"media","mime":"image/png","path":"/tmp/aview_<uuid>.png","source":"screenshot.png"}
  ```
  内核校验 MIME 白名单后原样附加至 `role: "tool"` 消息的 `media[]`（由 Provider 驱动按能力决定编码、转译或返回 `unsupported_media` 错误）。
* **全量压缩帧（`compact`）**：`{"op":"compact"}`
* **尾部折叠帧（`fold`）**：`{"op":"fold","from_turn":3,"summary":"..."}`（可省略 `summary` 触发自动摘要）
* **通用租约帧（`hold` / `release`，`v0.4.0` 取代 `bg_job`）**：
  ```json
  {"op":"hold","id":"j_a1b2c3d4","deadline_sec":600,"label":"cargo test"}
  {"op":"release","id":"j_a1b2c3d4"}
  ```
  `hold` 阻止 `--inbox` 在到期前因 `--idle-timeout` 退出；收到对应 `Anix-In-Reply-To: <id>` 消息或 `release` 帧时释放；超时自动合成 `hold-expired` 事件唤醒模型。

---

## 7. 配置 Schema 与 POSIX `StrictModes` 安全门禁（Normative）

### 7.1 单一显式配置文件（`config.toml`）

默认路径 `${XDG_CONFIG_HOME:-$HOME/.config}/anix/config.toml`（可由 `--config` 或 `$ANIX_CONFIG` 显式覆盖），未知字段 Fail-Closed 退出 `2`：

```toml
[defaults]
provider = "openai-responses"
model = "deepseek-v4.1-flash"
shell = "msh"
tool_name = "shell"
max_turns = 20
max_tokens = 128000
max_output_tokens = 0
compact_threshold = 0.8
output = "reply"
stderr = "auto"

[providers.openai-responses]
bin = "anix-provider-openai-responses"
mode = "coprocess" # coprocess | oneshot
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
timeout_sec = 120
params = {}

[compact]
# provider = "openai-responses"
# model = "deepseek-v4.1-flash"
# prompt = "..."
# prompt_file = "~/.config/anix/compact.prompt.md"
# bin = ""

[sandbox]
enabled = false
strict = false
```

### 7.2 钩子目录（`pre-context.d/`）与 POSIX `StrictModes`

* 内核仅保留 **`pre-context.d/`** 一个生命周期挂载点，目录位置为 `$ANIX_HOOKS_DIR/pre-context.d`（未设时取生效配置文件同级 `hooks/pre-context.d`）。
* 按 ASCII 字典序合并内置 `00-posix-runtime` 与目录内具备 `+x` 权限的外部脚本；同名 0 字节文件或指向 `/dev/null` 的软链用于屏蔽 `00-posix-runtime`。
* **POSIX `StrictModes` 强校验（取代 `anix trust`）**：加载任何外部 `pre-context.d/` 脚本、外部 `[compact].bin` 或自定义 `[providers.<name>].bin` 前，内核必须校验该文件及其所在钩子目录：
  1. 文件与目录属主（`st_uid`）必须为当前有效用户（`geteuid()`）或 `root`（`0`）；
  2. 文件与目录严禁存在 group/world 写权限（`st_mode & 0o022 == 0`）；
  3. 违规时立即向 `fd 2` 报错并以退出码 **`75`** 退出。

---

## 8. 外部线协议规范（Normative）

### 8.1 Canonical IR v1（JSONL）

保持 `ir_version = 1` 不变：
* **第 1 行 `header`**：`ir_version`、`type: "header"`、`tools: ToolDef[]`、`max_output_tokens?`、`params?`、`meta: {stderr_mode, output_mode?, cwd, turn_budget?}`。
* **第 2..N 行消息**：`system` / `user` / `assistant` / `tool`；`MediaAttachment` 仅传 `{mime, path}` 延迟读盘；发送前强校验三项不变量（头行+非空、工具调用严格闭合、`turn_id` 非降序）。

### 8.2 Provider Adapter 线协议（`coprocess` / `oneshot`）

* 保持 `coprocess`（默认）与 `oneshot` 两种模式、公共环境变量（`ANIX_PROVIDER_BASE_URL`/`ANIX_PROVIDER_API_KEY`/`ANIX_PROVIDER_TIMEOUT_SEC`/`ANIX_MODEL`/`ANIX_PROVIDER_MODE`/`ANIX_SESSION_ID`）、包含式 5 字段 `usage`、`opaque` 状态透传与重试归属契约不变。
* 上游不支持多模态输入时，Provider 返回 `{"type":"error","code":"unsupported_media","retryable":false}`（若需非视觉模型转译，由 Provider 自身或包装器在 Layer 2 处理）。

### 8.3 `pre-context.d/` 钩子线协议

`stdin` 接收 Canonical IR JSONL 流，`stdout` 输出修改后的 Canonical IR JSONL 流；退出码 `0` 继续，非零中止 `anix` 并退出 `1`。

### 8.4 结构化 Trace 线协议（`--trace-fd=<N>` / `ANIX_TRACE_FD=<N>` / `journal.sock`）

* 显式激活、非阻塞、遇 `EPIPE` 静默关闭；公共信封含 `v`、`ts`、`pid`、`ppid`、`agent`、`type`。
* 核心事件：`run_start`（含 `session_id`/`provider`/`model`/`shell`/`output`/`fresh_session`/`inbox_mode`/`leaf_id?`）、`cycle_start`（含 `cycle_id`/`source`/`message_ids?`/`start_turn_id`）、`llm_request_start`、`llm_response`、`provider_error`、`tool_start`、`tool_end`、`hold_started`、`hold_released`、`hold_expired`、`compact`（含 `checkpoint_node_id?`/`leaf_id?`）、`fold`（含 `node_id?`/`parent_id?`/`fold_usage?`）、`cycle_end`（严格提交屏障，含 `leaf_id?`/`message_ids?`）、`idle`（含 `active_holds`）、`warning`、`error`、`run_end`（含 `leaf_id?`）。

---

## 附录 A：`0.x` 核心演进总览

| 版本 | 核心里程碑 |
| :--- | :--- |
| **`v0.1.x`** | 显式 `--trace-fd`、标准化退出码 `75`/`124`、长驻 Provider `coprocess`、纯事件驱动等待、`session_id` 亲和。 |
| **`v0.2.0`** | 静态输出契约轴（`output: reply \| explicit \| none`）、`shell: none` 纯文本模式、Core RFC 与 Reference Runtime 文档拆分。 |
| **`v0.3.0`** | 单文件 Tree WAL（`id`/`parent_id` + `--leaf`）、统一 `fold` 为分支摘要节点、去掉 CLI `<args>` 包装。 |
| **`v0.4.0`** | **三原语纯净内核定型**：Maildir 统一唤醒与同步问答（`anix send [--wait]`/`--notify`/`hold` 租约/批量去抖/崩溃去重/`--lock-wait`）、跨文件并行分叉（`anix fork`/`fsck`/`log`）、剥离审批/`skills`/`tips`/`vision`/项目配置发现/`trust`/`post-tool`/`on-exit`/`msh` 硬编码至 Layer 2/3，Core 改用 POSIX `StrictModes`。 |
