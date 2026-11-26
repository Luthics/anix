# Anix Core（POSIX 原生 Agent 内核）v0.2 技术规范

**版本**：v0.2.0-rfc  
**状态**：Working Specification（`0.x` 演进期草案；`v1.0.0` 正式定稿前保留契约收口空间）  
**适用范围**：`anix-core` 内核与全部外部线协议（参考运行底座组件见 [RUNTIME.md](RUNTIME.md)）  
**协议版本**：`ir_version = 1`  
**目标平台**：Linux (`x86_64` / `arm64`)、macOS (Apple Silicon / Intel)、WSL2  

---

## 1. 设计哲学、四层架构边界与演进约定（Normative）

### 1.1 规范性关键词（RFC 2119）

本文档中的关键词 **必须（MUST）**、**严禁（MUST NOT）**、**应当（SHOULD）**、**可以（MAY）** 均按 RFC 2119 解释。凡标注“必须/严禁”的条款，参考实现与第三方兼容实现均不得偏离。

### 1.2 核心定位

Anix 将传统应用层 Agent Harness 溶解进 POSIX 操作系统：

* **Agent 即标准 POSIX 进程**：每个 `*.agent` 文件通过 Shebang 直接执行，与 `grep`、`awk`、`jq` 享有同等进程、管道、重定向与信号语义。
* **最小内核原则**：核心解释器 `anix` 仅负责 CLI/Frontmatter/配置解析、FD 路由与静态输出契约、推理循环调度、WAL 上下文状态机、纯事件驱动等待、信号处理与外部插件线协议；**严禁**在 `anix` 内置特定 LLM SDK、MCP 客户端、安全分类器/OCR 引擎、模型名能力白名单、交互式 TUI 或文件系统虚拟化层（如 OverlayFS）。
* **机制与策略分离**：Shell 执行器（`shell`）、LLM 驱动（`[providers.<name>]`）、动态审批策略（`[approval].bin`）、非视觉转译策略（`[vision].bin`）、上下文摘要策略（`[compact].bin`）与生命周期钩子（`hooks/*.d/`）均通过语言无关的外部子进程线协议解耦。
* **确定性边界包裹概率性内核**：除模型推理本身外，所有 I/O 通道、FD 契约、线协议 Schema、错误码、超时与权限门禁均必须遵循确定性行为（Fail-Fast / Fail-Closed）。

### 1.3 四层递进架构与规范边界

Anix 生态按**逐级递进、严格单向依赖**划分为四层；**本 RFC 仅规范 Layer 1（`anix-core`）的强制契约与跨进程线协议**：

| 层级 | 名称 | 归属与规范定位 | 职责边界 |
| :--- | :--- | :--- | :--- |
| **Layer 1** | **`anix-core`** | **本文档（Normative）** | 仅包含 `anix` 核心解释器与 `anix-proto` 线协议契约；**零依赖**任何具体 Shell、LLM 驱动、辅助工具或 TUI。 |
| **Layer 2** | **`anix-runtime`** | [RUNTIME.md](RUNTIME.md)（Informative） | 开箱即用的参考发行包：`anix` + `msh` + `a*`（`aclr`/`aview`）+ `anix-provider` + 默认增强钩子（`20-skills`/`30-tips`）+ 安装分发。所有非 Core 组件均可按本 RFC 自由替换。 |
| **Layer 3** | **`anix-agent`** | **独立分仓**（非本 RFC 范围） | 面向人类日常工作流的交互式高阶控制壳（类 Claude Code / Pi）：TUI、多会话/分支管理、可视化审批弹窗、代码 Diff 预览、动态每轮上下文注入。完全通过本 RFC 的外部协议（`--trace-fd`、`[approval].bin`、`-c` WAL、`--inbox`、`params`）驱动底层，不限定实现语言。 |
| **Layer 4** | **`anix-*`** | **独立分仓**（非本 RFC 范围） | 第三方与社区生态项目（自定义 Provider、审批策略引擎、视觉桥接、Trace 采集面板、领域 `*.agent` / Skills 包等），只需遵守本 RFC 线协议即可即插即用。 |

### 1.4 `0.x` 演进与兼容性声明

* **版本编号约定**：本规范当前处于 **`0.x` 演进阶段**（早期草稿与历史 Issue 中曾使用 `v1.0` / `v1.1.x` / `v1.2.0` 编号，现统一归并为 `v0.0` / `v0.1.x` / `v0.2.0`；在 `v1.0.0` 正式定稿前保留小版本契约收口空间，线协议版本字段维持 `ir_version = 1` 不变）。
* **保持兼容（Forward-Compatible）**：Canonical IR 维持 `ir_version = 1`；WAL 保持 Append-Only 格式前向兼容（`v0.1` / `v0.1.1` / `v0.1.2` / `v0.2.0` 新增字段均为可选）；`*.agent` 脚本格式、`$ANIX_OUT`/`$ANIX_ERR`/`$ANIX_CTL` 侧信道与 Lifecycle Hooks 线协议保持不变；所有新增可选插件（`[approval].bin`、`[vision].bin`）未配置时保持原有默认行为。
* **`v0.1.0 → v0.1.1` 增量扩展（向后兼容）**：新增会话级路由亲和标识 `session_id`（WAL `session_init`/`checkpoint` 可选字段、Provider 公共环境 `ANIX_SESSION_ID` 与 `coprocess` 信封可选 `session_id`、Trace `run_start.session_id`）；无状态运行自动生成进程级 UUID v4，历史无 `session_id` 的旧 WAL 自动派生确定性回退值。
* **`v0.1.1 → v0.1.2` 增量扩展**：Shell 工具调用新增可选 `timeout_sec`（受 `[defaults].max_msh_timeout_sec` 硬上限裁剪）与 `kill_on_timeout`（默认 `false`），新增 `$ANIX_CTL` `bg_job` 帧与后台任务完成 `<runtime_update>` 自动通知及 `bg_job_started`/`bg_job_finished` Trace 事件（工具线格式向后兼容；参考 Shell 默认超时行为由强杀改为转后台，见 [RUNTIME.md](RUNTIME.md)）。
* **`v0.1.2 → v0.2.0` 架构收口（三轴正交静态输出契约）**：将**能力轴（`shell: <bin> | none`）**、**策略轴（`approval: ask | auto | deny`）**与**输出契约轴（`output: reply | explicit | none`，默认 `reply`）**彻底正交解耦：
  * 任何配置下 `fd 1`（`stdout`）均有**唯一静态确定的写者**，绝不由 LLM 运行时采样决定；
  * `shell: none` 支持零 Shell 攻击面的纯函数文本过滤器 Agent（`tools: []`，单轮实时流式直出 `fd 1`）；
  * 默认 `output: reply`（改变 `v0.1` 默认仅靠 `$ANIX_OUT` 写 `fd 1` 的行为；需要精确工具直通字节流或大文件零拷贝的脚本显式声明 `output: explicit`；纯磁盘副作用型脚本声明 `output: none`）；
  * 旧 WAL 未记录 `output` 字段时，恢复漂移对比按 `explicit` 视之并按需注入 `<runtime_update>`；若最终 `output != explicit` 但脚本正文出现 `ANIX_OUT`，向 `fd 2` 输出非阻断迁移警告。
* **`v0.0 → v0.1` 显式契约收口（4 项）**：
  1. 废除硬编码 `fd 3` Trace，改为显式 `--trace-fd=<N>` / `ANIX_TRACE_FD=<N>`（§4.1、§9.4）；
  2. 标准化退出码 `75`（`EX_TEMPFAIL`）取代 `42`（§6.5）；
  3. Provider 默认升级为长驻 `coprocess` 模式，保留 `oneshot` 回退（§9.2）；
  4. 品牌与命名统一收口为 Anix（`anix`、`anix-provider*`、`.anix/`、`libexec/anix`、`ANIX_*`，保留 `*.agent` 脚本后缀与 `#!/usr/bin/env anix`）。

---

## 2. 外部组件解析与执行边界（Normative）

`anix` 内核不内置具体 Shell、LLM SDK 或增强工具，所有外部可执行组件均遵循统一的确定性解析契约（参考发行包的具体 FHS 布局见 [RUNTIME.md](RUNTIME.md#2-fhs-安装布局与组件总览)）：

1. **组件可执行路径解析顺序**：`anix` 解析 `shell`（当 `shell != "none"`）以及配置中的外部可执行文件（`[providers.<name>].bin`、`[approval].bin`、`[vision].bin`、`[compact].bin`）时：
   * 若配置值包含 `/`：展开前导 `~/` 后按路径解析（绝对路径直接使用；相对路径按项目根或当前工作目录解析）；
   * 若配置值不含 `/`：先按用户 `$PATH` 依次查找具备 `+x` 权限的文件，未命中时再回退私有组件目录 `$HOME/.local/libexec/anix/<name>`（开发未安装态可回退同目录兄弟可执行文件）。
2. **工具目录前置注入**：当 `shell != "none"` 时，`anix` 拉起 Shell 子进程前必须将私有工具目录（默认 `$HOME/.local/libexec/anix/tools`）前置（prepend）到子进程 `PATH` 中，避免内部辅助工具污染人类全局 `$PATH`。
3. **无反向依赖铁律**：任何满足本 RFC 第 4、5、7、8、9 章线协议的 Shell、Provider、工具或插件均可无修改替换参考发行包中的对应组件。

---

## 3. CLI 接口与 `*.agent` 文件规范（Normative）

### 3.1 CLI 调用语法

```sh
anix [OPTIONS] <SCRIPT.agent> [--] [<SCRIPT_ARGS>...]
anix trust <PATH>          # 信任项目级 hooks/ 及外部执行配置并记录 SHA-256
anix --doctor              # 校验内核运行环境、配置与凭据就绪
anix --version             # 输出版本号与支持的 ir_version
```

`--` 可选，仅在传递以 `-` 开头的裸参数且不希望被 `anix` 解析为自身选项时用于分隔。  
核心 CLI 选项（优先级最高）：

* `-c, --context <FILE>`：指定持久化 WAL JSONL 文件路径；传空串 `""` 强制无状态。
* `--provider <NAME>` / `--model <ID>` / `--shell <PATH|none>`（`none` 禁用全部 Shell 工具，进入零注入纯文本模式）
* `-o, --output <reply|explicit|none>`：声明 `fd 1`（stdout）的唯一静态输出契约（默认 `reply`，见 §4.1）
* `--vision` / `--no-vision`：显式声明当前模型是否支持原生视觉输入（默认 `true`，见 §5.5）。
* `--max-turns <N>` / `--max-tokens <N>` / `--max-output-tokens <N>` / `--compact-threshold <VAL>` / `--no-compact`
* `--approval <ask|auto|deny>` / `-y, --yes`（等价于 `--approval auto`）/ `--fail-on-deny`
* `--param <KEY=VALUE>`：透传给 Provider 的额外模型参数（可重复指定；值优先按 JSON 字面量解析，否则作为字符串）。
* `--stderr <auto|explicit>` / `-q, --quiet`（等价于 `--stderr explicit`）
* `--trace-fd <N>`：显式指定结构化 Trace（JSONL）输出的目标文件描述符编号（见 §4.1、§9.4）。
* `--inbox <DIR>`：进入常驻 Inbox 队列消费模式（Maildir 风格原子目录队列，异步非阻塞接收外部或上层壳投递的消息）。
* `--idle-timeout <SEC>`：常驻模式下的闲置自动退出秒数（默认 `300`；设为 `0` 表示永不因闲置退出，直至收到 `SIGINT`/`SIGTERM`）。

### 3.2 `*.agent` 文件格式与解析规则

1. **编码与行尾**：文件必须为合法 UTF-8；解析器必须同时兼容 `\n` 与 `\r\n`。
2. **Shebang**：若首行以 `#!` 开头，解析器必须忽略该行。
3. **YAML Frontmatter**：Shebang 之后（或文件首行）若出现独占一行的 `---`，则至下一个独占一行的 `---` 之间的内容作为 YAML 1.2 Frontmatter 解析；其余文本（去除首尾空行）作为 `agent_prompt`。若无 `---` 块，则全文作为 `agent_prompt`。
4. **未知字段处理（Fail-Closed）**：Frontmatter 中出现未定义字段（除 `x-*` 扩展命名空间外）时，`anix` 必须向 `fd 2` 报错并以退出码 `2` 退出。

示例：

```markdown
#!/usr/bin/env anix
---
name: security-auditor
provider: openai-responses
model: deepseek-v4.1-flash
vision: false
shell: msh
output: reply
max_turns: 15
max_tokens: 128000
compact_threshold: 0.8
approval: ask
stderr: auto
---
你是一个安全审计专家。读取传入的代码变更，分析潜在的并发竞态、注入与越权漏洞，输出高危漏洞的 JSON 数组。
```

### 3.3 Frontmatter 字段定义

| 字段 | 类型 | 默认值 | 确定性约束 |
| :--- | :--- | :--- | :--- |
| `name` | string | 文件名（去后缀） | 仅允许 `[a-zA-Z0-9._-]`；用于 Trace 与日志标识。 |
| `provider` | string | 全局 `defaults.provider` | 必须匹配 `config.toml` 中已定义的 `[providers.<name>]`。 |
| `model` | string | 全局 `defaults.model` | 非空字符串，传给 Provider 驱动。 |
| `vision` | bool | `true`（回退 Provider / `defaults`） | 显式声明当前模型是否支持原生视觉输入（见 §5.5）。 |
| `shell` | string | `"msh"` | 能力轴：可执行文件路径、`$PATH` 命令名，或字面量 `"none"`（禁用 Shell 工具，`tools: []`）。 |
| `output` | enum | `"reply"` | 契约轴（`v0.2.0`）：仅允许 `reply` \| `explicit` \| `none`，静态声明 `fd 1`（stdout）唯一写者（见 §4.1）。 |
| `max_turns` | uint32 | `20` | 必须 `>= 1`；达到上限且任务未完成时以退出码 `124` 退出。 |
| `max_tokens` | uint32 | `128000` | 必须 `>= 1024`；上下文硬上限，超出且无法压缩时以退出码 `124` 退出。 |
| `max_output_tokens` | uint32 | `0`（由上游决定） | 为 `0` 或省略时不写入 IR，沿用上游模型默认上限；若显式设为 `>= 1`，则必须 `< max_tokens`，且写入 IR `header.max_output_tokens`，Provider 必须将其映射至上游输出 Token 上限参数。 |
| `compact_threshold` | float \| uint32 | `0.8` | 若为 `(0.0, 1.0)` 浮点数，表示 `floor(max_tokens * val)`；若为 `>= 1` 整数，表示绝对 Token 数；若为 `0` 或 `>= max_tokens` 或传入 `--no-compact`，则禁用自动压缩。 |
| `approval` | enum | `"ask"` | 仅允许 `ask` \| `auto` \| `deny`。 |
| `stderr` | enum | `"auto"` | 仅允许 `auto` \| `explicit`。 |
| `context` | string | `""`（无状态） | 相对路径按当前工作目录（`cwd`）解析；为空时严格无状态。 |
| `params` | object | `{}` | 开放键值对透传字典（内部键不受未知字段 Fail-Closed 限制）；内核不解析其业务语义，按优先级浅合并后原样透传至 IR `header.params` 交由 Provider 处理。 |

### 3.4 配置优先级与环境变量继承

配置合并严格遵循以下优先级（高覆盖低）：  
`CLI 参数 > 环境变量 > *.agent Frontmatter > 项目 .anix/config.toml > 全局 config.toml > 内置默认值`

其中 `vision` 在 `config.toml` 内部遵循 `[providers.<name>].vision > [defaults].vision`（Provider 级优先于全局默认）。

标准环境变量映射：

* `ANIX_PROVIDER`、`ANIX_MODEL`、`ANIX_VISION`（`1`/`true` 为真，`0`/`false` 为假）、`ANIX_SHELL`、`ANIX_OUTPUT`（`reply`|`explicit`|`none`）、`ANIX_MAX_TURNS`、`ANIX_MAX_TOKENS`、`ANIX_MAX_OUTPUT_TOKENS`、`ANIX_COMPACT_THRESHOLD`、`ANIX_APPROVAL`、`ANIX_YES`（`1`/`true` 等价于 `--approval auto`）、`ANIX_STDERR`、`ANIX_LOG`（`human`|`info`|`trace`）、`ANIX_TRACE_FD`、`ANIX_TRACE_LEVEL`、`ANIX_CONTEXT`、`ANIX_INBOX`、`ANIX_IDLE_TIMEOUT`。
* **空串覆盖规则**：`ANIX_CONTEXT=""`（已设置但为空）必须覆盖 Frontmatter 中的 `context`，强制本次运行无状态。
* **启动期静态校验（Fail-Fast，`v0.2.0`）**：在全部 6 层配置合并完成之后、写入 WAL `session_init` 之前，必须执行以下确定性校验；凡违规一律向 `fd 2` 报错并以退出码 **`2`** 立即退出（严禁留下无效 WAL 或发起 Provider 请求）：
  1. `output`、`approval`、`stderr` 枚举值非法 → 退出 `2`；
  2. `output == explicit` 且（`shell == none` 或 `approval == deny`）→ 退出 `2`（`explicit` 契约要求存在可执行的 Shell 工具向 `$ANIX_OUT` 写入）；
  3. `shell != none` 但无法解析到具备 `+x` 权限的 Shell 可执行文件 → 退出 `2`（`shell == none` 时跳过检查，且忽略 `approval` 策略）；
  4. **非阻断警告（输出至 `fd 2`）**：`output == reply && shell != none && approval == deny`、`output == none && shell == none`、以及 `output != explicit` 但脚本正文出现 `ANIX_OUT`（迁移提示）。
* **子进程隔离规则（默认清空 + 观测白名单 + 主动注入生效审批）**：`anix` 拉起 Shell 子进程前必须**清空（`unset`）所有 `ANIX_*` 环境变量**（防止父进程的 `ANIX_CONTEXT`、`ANIX_INBOX`、`ANIX_MODEL`、`ANIX_VISION`、`ANIX_TRACE_FD`、`ANIX_MAX_TURNS` 等单实例状态、FD 编号或预算配置隐式污染子 Agent 并覆盖其 Frontmatter），仅按以下规则向下传递：
  * **可观测性白名单透传**：若父进程环境中存在 `ANIX_LOG` 或 `ANIX_TRACE_LEVEL`，原样透传（注意 `ANIX_TRACE_FD` 绝不透传，父进程的 Trace FD 在 `exec` 前已因 `FD_CLOEXEC` 关闭）；
  * **审批策略主动注入**：必须将当前父进程已解析生效的审批策略主动注入为 `ANIX_APPROVAL=<ask|auto|deny>`（不再单独透传 `ANIX_YES`，统一归一至 `ANIX_APPROVAL`）；
  * **侧信道主动注入**：仅当 `output == explicit` 时注入 `ANIX_OUT=/dev/fd/4` 并挂载 `fd 4`；当 `output == reply | none` 时严禁注入 `ANIX_OUT` 且在子进程 `pre_exec` 显式关闭 `fd 4`。`ANIX_ERR=/dev/fd/5` 与 `ANIX_CTL=/dev/fd/6` 始终注入。
  * 不带 `ANIX_` 前缀的外部变量（如 `OPENAI_API_KEY` 等）保持常规继承。
* **`params` 浅合并规则**：`params` 字典按 `CLI --param > *.agent Frontmatter params > 项目/全局 config.toml [providers.<name>.params]` 的优先级执行键级浅合并（Shallow Merge），高优先级仅覆盖同名键，不整体清空低优先级字典。

---

## 4. I/O 协议与全局文件描述符（FD）分配表（Normative）

为彻底消除多路输入、观测 Trace、系统继承句柄（如 `systemd LISTEN_FDS`）与内部侧信道之间的 FD 冲突，本规范将**外部 `anix` 进程**与**内部 Shell 子进程**的 FD 空间严格隔离，并废除硬编码 `fd 3`。

### 4.1 外部 `anix` 进程 FD 契约

| FD | 方向 | 角色 | 确定性行为规范 |
| :--- | :--- | :--- | :--- |
| **`fd 0`** | R | 主输入（stdin） | 阻塞读取全部字节直至 EOF（默认不支持流式边读边推）。若 CLI 传入了 `<SCRIPT_ARGS>`，将其按空格拼接并包装为 `<args>...</args>` 置于 `fd 0` 内容之前作为首条 `turn_user` 文本；当 `isatty(0)` 为真且已传入 `<SCRIPT_ARGS>` 时，**严禁阻塞等待 `stdin` EOF**，直接以 `<SCRIPT_ARGS>` 作为用户输入启动推理；仅当 `isatty(0)` 为真且未传入任何 `<SCRIPT_ARGS>` 时，阻塞读取 `stdin` 至 Ctrl+D（EOF）。 |
| **`fd 1`** | W | 纯净 Payload（stdout） | 遵循静态单写者 **`output`** 契约（`v0.2.0`，严禁由 LLM 运行时概率决定）：<br>• **`output = reply`（默认）**：仅输出本轮周期成功结束（无 `tool_calls` 的正常 `done`）时最终轮的 assistant 正文（不再重复写入 `fd 2`）。当 `shell = none` 时零延迟实时流式直出 `fd 1`；当 `shell != none` 时按轮缓冲 `text_delta`，确认无 `tool_calls` 后整块写出 `fd 1`（若该轮含 `tool_calls` 或发生错误/中断/预算耗尽，则已缓冲文本按 `stderr` 模式刷至 `fd 2`，`fd 1` 保持 0 字节）。仅当 `isatty(1)` 为真、内容非空且末尾非 `\n` 时补 1 个 `\n`；非 TTY（管道/重定向）时字节级原样输出。<br>• **`output = explicit`**：仅转发子进程显式写入 `$ANIX_OUT` 的原始字节流，无论 `isatty(1)` 真假均严禁写入模型普通文本、日志或自动补换行。<br>• **`output = none`**：严禁向 `fd 1` 写入任何字节（严格 0 字节，适用于纯磁盘副作用型 Agent）。 |
| **`fd 2`** | W | 人类诊断流（stderr） | `auto` 模式：接收模型普通文本（非 `output=reply` 终轮）、思考摘要、工具执行状态、`$ANIX_ERR` 写入内容及致命错误；`isatty(2)` 为真时以 ANSI Dim（`\x1b[2m`）降噪渲染。每次推理循环结束（单次模式在进程退出前；`--inbox` 模式在每条消息 `cycle_end` 时）向 `fd 2` 输出一行静态 ANSI Dim `[usage]` 汇总（严禁光标回退或动态刷新），包含当前上下文水位 `ctx <cur>/<max> (<pct>%)`、本次运行/本轮循环累计消耗（含内置压缩器与自动折叠开销，非零细项如 `cached`/`write`/`reasoning` 按需展示），以及 `-c` 持久化会话下的跨压缩总消耗 `total`。<br>`explicit` 模式（含 `-q`）：仅输出 `$ANIX_ERR` 写入内容及运行时致命错误；普通文本与思考仅落 Trace；严禁向 `fd 2` 输出 `[usage]` 汇总行（仍落 Trace 与 `on-exit.d/`）。<br>**确定性换行契约**：见下方专项条款。 |
| **`--trace-fd=<N>`** | W（可选） | 可配置结构化 Trace（JSONL） | 运行时 **严禁（MUST NOT）** 假定 `fd 3` 为空闲或默认观测通道。仅当通过 `--trace-fd=<N>` 或 `ANIX_TRACE_FD=<N>` 显式指定（CLI 优先级高于环境变量，且 `<N> >= 3`）时，通过 `fcntl(N, F_GETFL)` 校验其已打开且可写（`O_WRONLY`/`O_RDWR`），以单行 UTF-8 JSONL 写入事件；若指定了无效/不可写 FD，必须向 `fd 2` 报错并以退出码 `2` 退出。未指定时，FD 观测流默认关闭（不打开、不探测、不写入任何额外 FD；若 `$XDG_RUNTIME_DIR/anix/journal.sock` 显式存在，可按 §9.4 连接采集器）。 |
| **`fd 4..=9`** | R（可选） | 静态多路输入 | 启动时按序探测 `fd 4` 至 `fd 9`（**跳过已被 `--trace-fd=<N>` 占用的编号**）：凡处于打开且可读状态（`O_RDONLY`/`O_RDWR`）的 FD，阻塞读取至 EOF，按编号顺序包装为 `<input fd="N">...</input>` 附加在 `fd 0` 内容之后。 |

#### 人类诊断流（`fd 2`）与 `[usage]` 汇总行确定性换行契约

1. **阶段边界补齐**：在 `stderr = auto` 模式下，运行时必须跟踪最近写入 `fd 2` 的末字节；当从流式文本/思考输出切换至工具执行状态行、`$ANIX_ERR` 内容或任务结束前的 `[usage]` 汇总行时，若末字节非 `\n`，**必须**先向 `fd 2` 补发一个 `\n`。
2. **退出收尾保证**：`[usage]` 汇总行以及运行时自身向 `fd 2` 输出的所有诊断/错误行**必须**以 `\n` 结尾，确保 `anix` 进程退出时终端光标稳定停留在新行行首；当 `isatty(2)` 为真且使用 ANSI Dim 渲染时，**必须**按 `\x1b[2m[usage] ...\x1b[0m\n` 的顺序先重置 ANSI 样式再输出 `\n`，防止样式泄漏至宿主 Shell 提示符。
3. **`fd 1` 严格零侵入**：上述换行补齐规则**仅限 `fd 2`（stderr）**，**严禁**对 `fd 1` 生效。

**FD 清理铁律**：`anix` 在读取完静态输入 `fd 4..=9` 后必须立即关闭这些描述符，并在 `spawn` 任何子进程（Shell、Provider、Hooks、外部裁决器/转译器/压缩器）前确保除明确约定通道外的所有内部 FD（含 Trace FD 与 WAL 文件句柄）均设置 `FD_CLOEXEC`。

静态多路输入与显式 Trace 示例：

```sh
git diff HEAD~1 | ./audit.agent --trace-fd=3 3> audit.trace.jsonl 4< error.log | jq '.[] | select(.severity=="high")'
```

### 4.2 常驻队列与 Inbox 投递模式（`--inbox` / `--idle-timeout`）

普通调用（未传 `--inbox`）完成单次任务后必须立即退出 `0` 并关闭 `fd 1` EOF，以保证标准 Shell 管道不阻塞。当指定 `--inbox <DIR>` 时，`anix` 进入常驻纯事件驱动循环模式，采用 POSIX Maildir 风格目录队列实现异步、无阻塞、不交错的消息投递：

1. **目录结构初始化**：启动时若 `<DIR>` 或其子目录 `tmp/`、`new/`、`cur/`、`done/` 不存在，必须以权限 `0700` 自动创建。
2. **非阻塞原子投递契约（发送方）**：发送方（其他 Agent、外部脚本或上层 `anix-agent`）向 `<DIR>` 发送消息时，必须先将完整内容写入 `<DIR>/tmp/<ts_ns>-<pid>-<rand>`，关闭文件后通过原子 `rename()` 移入 `<DIR>/new/`。该机制保证发送方永不阻塞且彻底杜绝多写者大消息字节交错。
3. **串行安全消费（接收方）**：`anix` 通过 `inotify`（Linux）/ `kqueue`（macOS/BSD）事件机制监听 `<DIR>/new/`（遵循 §6.3 纯事件驱动规范，严禁固定间隔轮询），按文件名 ASCII 字典序逐条串行消费：
   * 仅在当前推理任务结束、所有 `tool_calls` 闭合的安全边界取出下一条消息，严禁并发修改上下文。
   * 消费前先将 `new/<name>` 原子 `rename()` 至 `cur/<name>`，读取全部字节并包装为 `<input source="<name>">...</input>` 作为新的 `role: "user"` 消息触发推理。
   * 该次任务完成、提交 WAL 并将 `cur/<name>` 移至 `done/<name>` **之后**，向 Trace 发出 `cycle_end` 事件（作为面向外层壳与编排器的严格提交屏障，见 §9.4）；当 `stderr = auto` 时同步输出该轮 `[usage]` 汇总行。
4. **保持模式与闲置退出（`--idle-timeout <SEC>`）**：
   * 启用 `--inbox <DIR>` 后，`anix` 在完成初始输入（如有）或某条 Inbox 消息后不退出，保持进程在线并持续持有 `-c <file>` 的 `flock` 独占锁与长驻 Provider 协进程，以最大化复用服务端 Prompt Cache 与 HTTP/2 连接池。
   * 当 `new/` 队列排空、即将进入事件阻塞等待时，向 Trace 发出一次 `idle` 事件（`{"type":"idle","idle_timeout_sec":<SEC>}`；在下一条消息到达前不重复发送）。
   * 事件循环阻塞超时必须精确设为距离最后一次任务完成 `<SEC>` 秒（默认 `300` 秒）到期的剩余毫秒数；若超时仍无新消息到达，则优雅关闭 Provider 协进程、释放 `flock` 并以退出码 `0` 退出；若设为 `0`，则阻塞超时为 `None`（无限阻塞，纯待机 `0 次/秒` 唤醒），直至收到 `SIGINT` 或 `SIGTERM`。

### 4.3 内部 Shell 子进程 FD 契约与防截断机制

`anix` 每次执行 Shell 工具调用拉起 `shell` 可执行文件时，必须按以下固定映射构造子进程 FD：

| 子进程 FD | 环境变量 | 底层实现与确定性语义 |
| :--- | :--- | :--- |
| **`fd 0`** | — | 绑定 `/dev/null`（严禁继承外部 stdin，彻底杜绝交互挂起）。 |
| **`fd 1, fd 2`** | — | 绑定至主进程捕获管道，作为工具结果返回模型。 |
| **`fd 4`** | `ANIX_OUT=/dev/fd/4`（仅 `output=explicit`） | **仅当 `output == explicit` 时挂载匿名 `pipe()` 写端**，主进程后台中转将读端原样字节流转发至外部 `anix` 的 `fd 1`。当 `output == reply \| none` 时**必须（MUST）**在 `pre_exec` 显式关闭 `fd 4` 且从子进程环境移除 `ANIX_OUT`，确保 `fd 1` 严格单写者。 |
| **`fd 5`** | `ANIX_ERR=/dev/fd/5` | **匿名 `pipe()` 写端**。主进程后台中转将读端原样字节流转发至外部 `anix` 的 `fd 2`（`auto` 与 `explicit` 模式均始终可用）。 |
| **`fd 6`** | `ANIX_CTL=/dev/fd/6` | **统一侧信道控制管道（匿名 `pipe()` 写端）**。供 Shell 及其子工具向主进程发送单行 JSONL 控制帧（格式见 §7.2）。 |

**规避 `O_TRUNC` 截断陷阱**：严禁通过 `dup2` 将外部 `fd 1`/`fd 2` 直接暴露给子进程。经由匿名 `pipe()` 中转后，POSIX 内核对管道文件自动忽略 `O_TRUNC`，确保即使外部重定向至普通文件（`./audit.agent > out.json`）且模型多次执行 `> $ANIX_OUT`，所有写入均安全顺序追加，永不互相覆盖。

### 4.4 核心契约注入钩子（`00-posix-runtime`）

`00-posix-runtime` 的默认逻辑必须内置于 `anix` 二进制中（无需依赖磁盘预置文件，确保全新环境开箱即用）；仅当全局或已信任项目 `hooks/pre-context.d/` 中存在同名文件 `00-posix-runtime`（或 0 字节文件 / 指向 `/dev/null` 的软链）时，按 §8.3 规则被外部覆盖或屏蔽。`00-posix-runtime` 必须在首轮组装冻结 `system_context` 时按当前生效的 `(output, shell, stderr, vision)` 四元组确定性注入 `<posix_runtime>` 契约：

* **`output = reply` + `shell = none`**：声明当前为无工具纯文本过滤器模式，最终正文回复直接流式写入 `fd 1`（stdout）；不附加多余寒暄或包装。
* **`output = reply` + `shell != none`**：声明可使用 Shell 工具执行命令；`$ANIX_OUT` 未挂载；仅**最后一轮（不再调用工具）的正文回复**输出至 `fd 1`（stdout），中间轮次文本与 `$ANIX_ERR` 走 `fd 2`（受 `stderr` 控制）。
* **`output = explicit`**：声明 `$ANIX_OUT` 已挂载为 `fd 1` 唯一交付通道；`stderr = auto` 时普通回复展示于 `stderr`，`stderr = explicit` 时普通回复对人类不可见（人类提示须显式写入 `$ANIX_ERR`）；最终交付数据必须通过 Shell 重定向至 `$ANIX_OUT`。
* **`output = none`**：声明 `fd 1` 严格静默（`$ANIX_OUT` 未挂载），任务交付通过 Shell 副作用（如修改文件）完成，人类可读说明走 `fd 2`（`stderr = auto` 为普通文本，`stderr = explicit` 为 `$ANIX_ERR`）。
* **`vision` 能力动态适配**（仅 `shell != none` 时）：按 `vision = true` / `vision = false` 无桥接 / `vision = false` 有 `[vision].bin` 明确告知模型视觉输入能力边界。

---

## 5. 审批门禁、外部裁决器与视觉防护（Normative）

所有内置人工交互审批严禁读写 `fd 0`，必须直接以 `O_RDWR` 打开控制终端 `/dev/tty`。

### 5.1 审批触发规则

* **`approval = auto`**（或 `-y` / `ANIX_YES=1`）：直接放行所有 Shell 工具调用，不调用外部裁决器。
* **`approval = deny`**：严禁执行任何不在 `config.toml[approval].safe_commands` 白名单中的命令，不调用外部裁决器；命中拦截时向模型返回 `"Error: Command denied by approval=deny policy"`（或按 `--fail-on-deny` 以退出码 **`75`** 退出）。
* **`approval = ask`**：每次调用 Shell 前依次检查：
  1. 命令是否命中 `[approval].safe_commands` 或本次进程内存中的 `Always` 放行表 → 命中直接放行；
  2. 若配置了 `[approval].bin` → 调用外部审批裁决器（§5.2）；
  3. 否则（或裁决器返回退出码 `2`）→ 进入 `/dev/tty` 人工交互审批（§5.3）；无法打开 `/dev/tty` 时触发 Fail-Closed（§5.4）。

### 5.2 外部审批裁决器（`[approval].bin`）

为在不破坏最小内核的前提下支持动态细粒度审批策略（如规则引擎、安全分类器、或上层 `anix-agent` 的统一审批 UI 代理），`config.toml` 的 `[approval]` 段支持配置外部审批裁决可执行文件：

```toml
[approval]
safe_commands = ["ls", "cat", "head", "tail", "rg", "grep", "find", "pwd", "git status", "git diff", "git log"]
# 可选：外部审批裁决可执行文件（默认空，沿用内置审批逻辑）
# bin = "~/.config/anix/hooks/approval-decider"
# 可选：单次裁决超时秒数（默认 30）
# timeout_sec = 30
```

* **防 RCE 强校验**：若项目级 `.anix/config.toml` 配置了 `[approval].bin`，必须纳入 `anix trust` 的 SHA-256 校验范围（§8.3）；未信任或哈希变更时必须立即向 `fd 2` 报错并以退出码 **`75`** 退出。
* **线协议（`stdin` / `stdout` / 退出码）**：执行 Shell 工具前，主进程将待审批上下文作为单行 UTF-8 JSON 写入 `[approval].bin` 的 `stdin` 并关闭写端（EOF）：

```json
{"v":1,"cmd":"rm -rf build/","cwd":"/abs/path/to/repo","agent":"dev-helper","pid":48210,"ppid":48102,"approval_mode":"ask"}
```

内核严格按 `[approval].bin` 的进程退出码执行确定性裁决：

| 退出码 | 裁决语义 | 内核确定性行为 |
| :--- | :--- | :--- |
| **`0`** | Allow（放行） | 直接放行本次工具调用，跳过 `/dev/tty` 交互。 |
| **`1`** | Deny（拒绝） | 拒绝执行，不终止 `anix` 进程；读取裁决器 `stdout`（去除首尾空白；为空时使用默认拒绝文案）作为工具执行结果（`is_error: true`）返回模型。 |
| **`2`** | Escalate（回退人工审批） | 交还内核，回退至默认 `/dev/tty` 人工交互审批 `[y/n/a/e]`；若无法打开 `/dev/tty`，则触发 §5.4 Fail-Closed 以退出码 **`75`** 退出。 |
| **其他非零 / 超时** | Fail-Closed 拦截 | **严禁放行**；向 `fd 2` 输出裁决器 `stderr` 诊断信息、记录 Trace 后以退出码 **`75`** 终止进程。 |

* **Trace 审计字段**：Trace 中的 `tool_approval` 事件必须包含 `decider`（`"builtin"` | `"external"`）字段，并在 `external` 时附加 `decider_exit_code`。

### 5.3 `/dev/tty` 交互协议：`[y/n/a/e]`

向 `/dev/tty` 打印待执行完整命令并读取单字符指令：

* **`y` (Yes)**：放行本次执行。
* **`n` (No)**：不终止进程；允许用户输入可选拒绝理由，并将 `"Error: User denied this action. Reason: <reason>"` 作为工具执行结果（`is_error: true`）返回模型。
* **`a` (Always)**：将当前命令的可执行程序名（`argv[0]`）加入当前 PID 生命周期的内存白名单（不持久化落盘），本次运行内同类命令自动放行。
* **`e` (Edit)**：将待执行命令写入 `/tmp/anix_edit_<uuid>.sh`（权限 `0600`），调用 `$EDITOR`（回退 `vi`）编辑；退出码为 `0` 且内容非空时执行修改后的命令，随后删除临时文件。

### 5.4 非交互环境降级（Fail-Closed）

当 `approval = ask`、未开启 `-y`/`ANIX_YES=1` 且无法打开 `/dev/tty`（如 cron、CI/CD、无 TTY 管道）时，一旦需要人工交互审批，**严禁阻塞等待**，必须立即向 `fd 2` 打印被拦截命令、写入结构化 Trace（如已激活）并以退出码 **`75`**（`EX_TEMPFAIL`）终止进程。

### 5.5 视觉能力声明与非视觉模型降级（`vision` / `[vision].bin`）

内核**严禁**按模型名称字符串硬编码猜测视觉能力，必须通过显式配置与确定性防护处理 `fd 6` 传入的 `media` 控制帧：

1. **显式能力声明**：布尔配置 `vision`（默认 `true`），优先级遵循 §3.4：`CLI --vision / --no-vision > ANIX_VISION > *.agent Frontmatter vision > config.toml [providers.<name>].vision > config.toml [defaults].vision > true`。
2. **`vision = false` 且无 `[vision].bin` 时的内核拦截（防 WAL 污染）**：
   * 收到 `{"op":"media", ...}` 控制帧时：
     * 主进程**严禁**将该附件写入 `role: "tool"` 消息的 `media[]` 字段，也**严禁**将其持久化写入 WAL；
     * 主进程必须将该次工具结果标记为 `is_error: true`，并附加错误说明：`"Error: current model does not support native vision (vision=false). Use text tools (e.g. OCR/pdftotext) or delegate to a vision-capable sub-agent."`。
3. **可选外部视觉转译器（`[vision].bin`）**：
   * 配置 Schema：`[vision]` 下可选 `bin` 与 `timeout_sec`（默认 `60`）。项目级 `.anix/config.toml` 配置的 `[vision].bin` **必须**纳入 `anix trust` 的 SHA-256 校验范围，未信任或哈希变更时以退出码 **`75`** 退出。
   * **线协议**：当 `vision = false` 且 `[vision].bin` 非空时，主进程在收到 `media` 控制帧后，将单行 UTF-8 JSON 写入 `[vision].bin` 的 `stdin` 并关闭写端（EOF）：
     ```json
     {"v":1,"mime":"image/png","path":"/tmp/aview_xxx.png","source":"screenshot.png","cwd":"/abs/path/to/repo"}
     ```
   * **输出与应用**：退出码 `0` 时，读取 `stdout` 的合法 UTF-8 纯文本描述，包装为 `<vision_description source="screenshot.png">...</vision_description>` 拼入本次 `role: "tool"` 消息的 `text` 中，**严禁**保留 `media[]`；非零退出或超时时不终止主进程，丢弃 `media` 帧并将该次工具结果标记为 `is_error: true`（附带转译器 `stderr` 摘要）返回模型。
4. **Provider 兜底错误码**：当上游 API 拒收多模态附件时，Provider **必须**识别并输出标准非重试错误事件：
   ```json
   {"type":"error","code":"unsupported_media","message":"Upstream model does not support image inputs","retryable":false}
   ```
   确保主进程立即报错退出（退出码 `1`），不执行无意义的指数退避重试。

---

## 6. 上下文管理、WAL 持久化、事件驱动、信号与退出码（Normative）

### 6.1 自动上下文压缩、硬上限熔断与折叠原语

1. **Turn ID 锚点规则**：每个会话（或单次无状态运行）维护严格单调递增的整数 `turn_id`（从 `1` 开始，每次用户消息入队或工具执行结果返回均递增 `1`，即使发生 `fold` 也不回退编号）。Shell 执行器不感知 `turn_id`；`[t<turn_id>] exit=<code>\n` 头部锚点统一由主进程 `anix` 在构造 `role: "tool"` 的 `text` 时拼接。
2. **每轮推理前预算检查（有效上下文水位 `T_cur`）**：
   * 定位当前活跃窗口内**最后一条存活的 `role: "assistant"` 消息**：若存在且携带 `usage`，则以该条 `usage.input_tokens + usage.output_tokens` 为基准，加上其后所有活跃消息的保守字节估算 `ceil(bytes / 3)`。
   * 若活跃窗口内无带 `usage` 的存活 `assistant`（如首轮或刚完成大压缩 `checkpoint`），则对整个活跃窗口统一执行 `ceil(bytes / 3)` 估算；**严禁**将大压缩或尾部自动折叠的压缩器 `usage` 计入活跃窗口 `T_cur`。
   * 发生尾部折叠（`fold`）后，尾部轮次被弹栈，最后一条存活 `assistant` 自动回退至折叠边界之前，`T_cur` 同步回落并叠加新 `<folded>` 消息的字节估算。
   * 若压缩后（或禁用自动压缩时）仍有 `T_cur > max_tokens`，必须立即向 `fd 2` 输出超限错误并以退出码 **`124`** 熔断退出。
3. **显式压缩与尾部折叠控制帧语义（通过 `$ANIX_CTL` 触发）**：
   * **`{"op":"compact"}`**（全量压缩）：按第 4 条 `[compact]` 策略生成结构化摘要、写入 WAL `checkpoint` 分节，并重新执行 `pre-context.d/` 刷新冻结 `system_context`。
   * **`{"op":"fold","from_turn":N,"summary":"..."}`**（手工摘要尾部折叠，零额外 LLM 调用）：校验 `N > last_checkpoint_turn && N <= current_turn`；非法时不修改上下文并向模型报错。合法时经工具对闭合对齐后将 `from_turn..=current_turn`（连同触发本次折叠的当轮 `assistant(tool_call)`）原子弹栈，原位压入 `<folded turns="from_turn..now">{summary}</folded>`，**严禁修改头部 `system_context` 与 `1..(from_turn - 1)`**，最大化保留 Prefix-Cache。
   * **`{"op":"fold","from_turn":N}`**（省略 `summary`，自动摘要尾部折叠）：边界校验与工具对闭合对齐规则与手工版一致；主进程按 `[compact]` 优先级解析压缩器，仅将对齐后的待折叠子窗口消息（`from_turn..=current_turn`）连同轻量摘要指令构造为无工具（`tools: []`）的 Canonical IR 喂给压缩器；取合法 UTF-8 纯文本作为 `summary` 写入 WAL `fold` 事件并原位压入 `<folded>`；**严禁**修改头部冻结 `role: "system"` 与 `1..(from_turn - 1)` 的任何字节；失败或空输出时中止折叠并将错误作为 `is_error: true` 工具结果返回模型。内置压缩器产生的 5 字段 `usage` **必须**计入 `run_usage`、`session_usage` 与 Trace `fold.fold_usage`，但**严禁**计入折叠后的 `T_cur`。
   * **工具对闭合对齐（Tool-Pair Closure）**：若 `N` 落在某条 `role: "tool"` 消息上，运行时必须自动将 `from_turn` 向上对齐至对应 `assistant(tool_call)` 的 `turn_id`，将该 `assistant` 及其同批所有 `tool` 消息一并纳入折叠范围，严禁产生违反 §9.1 不变量 (2) 的孤立 `tool_calls`。
4. **可覆写的压缩策略（机制与策略分离）**：自动压缩、`compact` 控制帧、`fold` 自动摘要与 `SIGUSR1` 均按以下确定性优先级解析 `[compact]` 配置：
   * **外部自定义压缩器（`[compact].bin` 非空，最高优先）**：`anix` 将待压缩的 Canonical IR JSONL 写入该可执行文件的 `stdin` 并关闭写端，读取 `stdout` 的合法 UTF-8 纯文本作为摘要；全量压缩失败或空输出时以退出码 `1` 退出，自动尾部折叠失败时返回工具错误。项目级 `[compact].bin` 受 `anix trust` 门禁校验。
   * **内置默认压缩器（默认）**：使用 `[compact].provider` 与 `[compact].model`（未配置时回退当前会话），结合 `[compact].prompt` 或 `[compact].prompt_file`（二选一，未配置时回退内置极简摘要提示词）调用 Provider 生成摘要。
   * 压缩器（无论内置或外部）均不具备 Shell 工具权限（`tools: []`）。

### 6.2 无状态持久化上下文（`-c <file>`）与 WAL Schema

`anix` 本体不维护中心化数据库；`-c <file>` 是唯一持久化原语，采用 **Append-Only JSONL（WAL）** 格式。

1. **并发防线**：打开 `<file>` 时必须调用 `flock(LOCK_EX | LOCK_NB)` 获取独占非阻塞锁。若返回 `EWOULDBLOCK`，必须立即向 `fd 2` 报错并以退出码 `1` 退出。
2. **KV-Cache 工程让步（不可变 Prefix + 尾部追加）**：
   * **首轮冻结**：新建 `<file>` 时，执行 `pre-context.d/` 并将初始配置写入 `session_init` 行，紧随其后将冻结的系统提示词写入 `turn_id: 0` 的 `role: "system"` 消息行；后续恢复时原样复用该冻结前缀。
   * **Config Drift 尾部通知**：恢复运行若检测到契约类配置（如 `output`、`shell` 启用状态、`stderr`、`approval`、`vision`）与 WAL 中最新生效状态不一致，当前调用参数胜出，严禁改动头部 `role: "system"`，仅在尾部追加一条 `runtime_update` 事件（映射至 IR 时作为尾部 `role: "system"` 增量消息，若上游 API 不支持中间 `system` 则由 Provider 降级为 `user`）。**`v0.2.0` 向后兼容**：若逆读的旧版 WAL 缺失 `output` 字段，漂移检测必须将其视作 `explicit`。
   * **大压缩分节（唯一刷新时机）**：仅当自动压缩、`compact` 控制帧或 `SIGUSR1` 触发大压缩时，在文件末尾追加一个新的 **`checkpoint` 分节头**（携带 `ir_version: 1`、最新 `config` 与新 `turn_id`），紧接着追加重新执行 `pre-context.d/` 生成的新冻结 `role: "system"` 行与摘要行；尾部折叠 `fold` 严禁刷新头部 Prefix。
3. **WAL JSONL 事件 Schema**（每行必须为合法 JSON + `\n`，每轮结束原子 flush）：
   * **会话路由标识（`session_id`，`v0.1.1`）**：`session_init` 与 `checkpoint` 分节头均携带可选字段 `session_id`（UTF-8 字符串，默认 UUID v4）。新建 WAL 时生成一次并写入 `session_init`；写入 `checkpoint` 时**必须（MUST）**原样延续当前会话的 `session_id`，确保 `O(当前活跃窗口)` 逆读停在 `checkpoint` 时仍能恢复同一会话标识；无状态运行生成进程级临时 UUID v4；逆读缺失 `session_id` 的旧 WAL 时严禁报错，基于该 WAL 规范路径派生确定性回退标识。
   * `role: "assistant"` 消息行可携带可选字段 `provider_opaque`（原样保存 Provider 返回的 `opaque.data`）以及可选字段 `usage`（同构 5 字段对象，见 §9.2；构造 Canonical IR 发给 Provider 时必须剥离，严禁写入 IR）；
   * `fold` 事件在自动摘要折叠模式下可携带可选字段 `fold_usage`（5 字段对象，计入本段 `cumulative_usage`）；
   * `checkpoint` 分节头可携带可选字段 `compact_usage` 与 `cumulative_usage`（5 字段前缀和，等于上一 `checkpoint` 的 `cumulative_usage` 加上本段窗口内所有 `assistant.usage`（含被 `fold` 剪枝的行）、所有 `fold.fold_usage` 与本次 `compact_usage`）。

```json
{"ir_version":1,"type":"session_init","ts":"2026-11-24T12:00:00Z","session_id":"550e8400-e29b-41d4-a716-446655440000","config":{"output":"reply","shell":"msh","stderr":"auto","approval":"ask","vision":true}}
{"type":"message","turn_id":0,"role":"system","ts":"2026-11-24T12:00:00Z","text":"<posix_runtime>...</posix_runtime>\n\n你是一个安全审计专家..."}
{"type":"message","turn_id":1,"role":"user","ts":"...","text":"分析这个报错截图"}
{"type":"message","turn_id":2,"role":"assistant","ts":"...","text":"","reasoning":"...","tool_calls":[{"id":"call_1","name":"msh","arguments":{"cmd":"aview err.png"}}],"usage":{"input_tokens":4200,"output_tokens":380,"cached_tokens":3600,"cache_write_tokens":400,"reasoning_tokens":200}}
{"type":"message","turn_id":3,"role":"tool","ts":"...","tool_call_id":"call_1","text":"[t3] exit=0\n","media":[{"mime":"image/png","path":"/tmp/aview_xxx.png"}]}
{"type":"runtime_update","turn_id":4,"ts":"...","content":"stderr mode switched to explicit...","config":{"output":"reply","shell":"msh","stderr":"explicit"}}
{"type":"fold","ts":"...","from_turn":3,"to_turn":6,"summary":"使用xx是错误的","fold_usage":{"input_tokens":1800,"output_tokens":95,"cached_tokens":0,"cache_write_tokens":0,"reasoning_tokens":0}}
{"ir_version":1,"type":"checkpoint","turn_id":7,"ts":"...","session_id":"550e8400-e29b-41d4-a716-446655440000","config":{"output":"reply","shell":"msh","stderr":"explicit","approval":"ask","vision":true},"compact_usage":{"input_tokens":98000,"output_tokens":1200,"cached_tokens":80000,"cache_write_tokens":0,"reasoning_tokens":400},"cumulative_usage":{"input_tokens":246800,"output_tokens":8695,"cached_tokens":192000,"cache_write_tokens":6400,"reasoning_tokens":3100}}
{"type":"message","turn_id":7,"role":"system","ts":"...","text":"<posix_runtime>...</posix_runtime>\n\n你是一个安全审计专家..."}
{"type":"message","turn_id":8,"role":"user","ts":"...","text":"<checkpoint>\n...\n</checkpoint>"}
```

4. **逆读回放算法与尾行损坏处理（Fail-Closed）**：
   * **逆读到最新大压缩点**：启动加载 `-c <file>` 时，**必须从文件末尾（EOF）向前按行逆读**，使加载复杂度保持 `O(当前活跃窗口)`。
   * **尾行损坏处理（Fail-Closed）**：逆读首行（文件最后一行）若非合法 UTF-8 JSON，必须立即向 `fd 2` 报错并以退出码 `1` 退出，严禁静默丢弃数据。
   * **主动折叠自动剪枝（Single-Pass Reverse Fix）**：维护剪枝水位 `skip_ge = +∞`：
     1. 遇到 `fold` 事件：累加可选 `fold_usage`；压入 `<folded turns="{from_turn}..{to_turn}">{summary}</folded>`，更新 `skip_ge = min(skip_ge, from_turn)`；
     2. 遇到普通 `message` 或 `runtime_update`：凡 `role: "assistant"` 携带 `usage` 一律累加计入窗口历史消耗；若 `turn_id >= skip_ge` 则跳过丢弃，否则压入结果栈；
     3. 若保留窗口末尾（或 `skip_ge` 边界前一条）落在带有未闭合 `tool_calls` 的 `assistant` 消息上，继续上调 `skip_ge` 并丢弃之，确保满足 §9.1 不变量 (2)；
     4. 遇到 `checkpoint` 或 `session_init` 分节头：校验 `ir_version == 1`（不兼容则退出 `2`），提取最新配置与 `session_id`，叠加 `cumulative_usage` 后**立即停止逆读**；
     5. 将结果栈反转即得按时间正序、无孤立 `tool_call` 的活跃上下文。

### 6.3 纯事件驱动等待规范

为保证高密度常驻（如单机百实例 `--inbox`）下的零空闲唤醒与低延迟短命令执行，内核实现 **严禁（MUST NOT）** 使用固定间隔睡眠轮询：

* **子进程退出监听**：Linux 平台 **应当（SHOULD）** 使用 `pidfd_open` + `epoll`；macOS/BSD 平台 **应当（SHOULD）** 使用 `kqueue`（`EVFILT_PROC` / `NOTE_EXIT`）。
* **`--inbox` 与信号集成**：信号处理 **必须（MUST）** 通过 `signalfd`（Linux）、`EVFILT_SIGNAL`（macOS/BSD）或跨平台 `self-pipe` 统一接入主事件循环；`--inbox` 目录变更通过 `inotify`（Linux）或 `kqueue`（macOS/BSD `EVFILT_VNODE`）接入同一事件循环。
* **精确超时**：事件循环阻塞超时 **必须（MUST）** 设为距离下一次到期事件（`--idle-timeout`、子进程超时或退避定时器）的精确剩余毫秒数；无待决超时时无限阻塞（`None`），确保纯待机状态下唤醒次数为 **`0 次/秒`**。

### 6.4 POSIX 信号处理规范

| 信号 | 确定性行为 |
| :--- | :--- |
| **`SIGINT`** | 向当前活跃子进程组（Provider / Shell / 外部插件）发送 `SIGTERM`（事件驱动等待最多 3 秒后仍未退出则 `SIGKILL`），丢弃未提交的半成品 Turn，释放 `flock`，以退出码 **`130`**（`128 + 2`）退出。 |
| **`SIGTERM`** | 执行与 `SIGINT` 相同的优雅清理流程，以退出码 **`143`**（`128 + 15`）退出。 |
| **`SIGSTOP`** / **`SIGCONT`** | 遵循内核默认行为，挂起与恢复进程。 |
| **`SIGUSR1`** | 置位原子标志；在下一轮 Provider 调用前强制触发一次全量压缩（等价于 `{"op":"compact"}`）。 |
| **`SIGUSR2`** | 将当前内存中的完整 Canonical IR JSONL 快照原子写入 `/tmp/anix_<pid>_<epoch_ms>.ctx.jsonl`（权限 `0600`），并在 `fd 2` 输出文件路径（以 `\n` 结尾）。 |

### 6.5 标准退出码表

退出码对齐 POSIX、BSD `sysexits.h` 与 GNU `timeout(1)` 惯例：

| 退出码 | 符号语义 | 触发条件 |
| :--- | :--- | :--- |
| **`0`** | `EX_OK` | 任务正常完成，Payload 已完整刷入 `fd 1`（或 `--inbox` 闲置优雅退出）。 |
| **`1`** | `GENERAL_ERR` | 通用运行时错误：Provider 网络/协议失败（含 `unsupported_media` / `media_not_found`）、I/O 异常、WAL 损坏、`flock` 锁冲突、未知内部错误。 |
| **`2`** | `EX_USAGE` | 命令行参数非法、无效 `--trace-fd`、启动期静态校验失败、Frontmatter / `config.toml` 语法或未知字段错误、`ir_version` 不兼容。 |
| **`75`** | `EX_TEMPFAIL` | **挂起 / 待外部介入（取代 `v0.0` `42`）**：无 TTY 环境下触发 `approval=ask` 拦截、`approval=deny` 叠加 `--fail-on-deny`、`[approval].bin` 异常或超时 Fail-Closed、项目级外部执行（`hooks/`、`[approval].bin`、`[vision].bin`、`[compact].bin`、`providers.<name>.bin`）未经 `anix trust` 校验或 SHA-256 变更。具体原因**必须（MUST）**通过 `fd 2` 诊断与结构化 Trace 输出。 |
| **`124`** | `EX_TIMEOUT` | 预算或超时熔断：达到 `max_turns`、超出 `max_tokens` 硬上限，或 Provider 调用超时。注：单条 Shell 子命令返回 `124` 时不终止 `anix` 主进程（作为 `is_error: true` 工具结果返回模型）。 |
| **`128+N`** | `SIGNAL` | 被信号 `N` 终止（如 `130` 对应 `SIGINT`，`143` 对应 `SIGTERM`）。 |

---

## 7. Shell 执行器协议与 `$ANIX_CTL` 侧信道规范（Normative）

本节定义 `anix-core` 与外部 Shell 执行器及侧信道工具之间的**强制线协议**；参考 Shell（`msh`）与参考工具集（`aview`、`aclr`）的具体实现约定见 [RUNTIME.md](RUNTIME.md)。

### 7.1 Shell 执行器子进程调用协议

当 `shell != "none"` 时，内核向模型暴露单一 Shell 工具（默认名称 `msh`，参数 `cmd`（必填）、`timeout_sec`（可选整数 `>= 0`）、`kill_on_timeout`（可选布尔，默认 `false`）），并按以下契约拉起解析后的 Shell 可执行文件：

1. **调用形式**：`<shell_bin> -c "<cmd>"`，工作目录为当前 `cwd`，置于独立进程组（`setpgid(0, 0)`）；Linux 平台配合 `PR_SET_PDEATHSIG(SIGKILL)` 防孤儿进程。
2. **环境变量注入**：在执行 §3.4 的 `ANIX_*` 清理与白名单/审批/侧信道注入之外，内核注入本次执行策略环境变量：
   * `MSH_TIMEOUT_SEC`：由工具调用入参 `timeout_sec` 裁剪至 `[defaults].max_msh_timeout_sec`（默认 `600`）之内，省略时回退 `[defaults].msh_timeout_sec`（默认 `120`）；若 `kill_on_timeout == true` 且结果为 `0` 则裁剪为 `1`；
   * `MSH_KILL_ON_TIMEOUT`：`"1"` 或 `"0"`；
   * `MSH_JOBS_DIR`：后台任务目录路径（默认 `$XDG_RUNTIME_DIR/anix/jobs`，权限 `0700`）。
3. **FD 映射**：严格遵循 §4.3（`fd 0` → `/dev/null`、`fd 1/2` → 捕获管道、`fd 4` 仅 `output == explicit` 挂载、`fd 5` → `$ANIX_ERR`、`fd 6` → `$ANIX_CTL`）。
4. **退出码映射**：捕获的 `stdout`/`stderr` 经 `post-tool.d/` 钩子后拼接 `[t<turn_id>] exit=<code>\n` 返回模型；退出码为 `124`（或存在视觉/折叠校验错误）时置 `is_error = true`。

### 7.2 统一侧信道协议（`fd 6` / `$ANIX_CTL`）

Shell 及其子进程可通过环境变量 `ANIX_CTL`（`/dev/fd/6`）向主进程 `anix` 写入单行 UTF-8 JSONL 控制帧（单帧上限 64 KB）。单次工具调用结束后，主进程按写入顺序解析并应用所有合法帧（格式错误或超长帧记入 `warning` 而不崩溃）：

* **多模态附加帧（`media`）**：
  ```json
  {"op":"media","mime":"image/png","path":"/tmp/aview_<uuid>.png","source":"screenshot.png"}
  ```
  受 §5.5 视觉策略管辖（`vision = true` 写入 `media[]`；`vision = false` 且有 `[vision].bin` 转译为文本；`vision = false` 且无转译器拦截报 `is_error: true` 且不写 WAL）。
* **全量压缩帧（`compact`）**：
  ```json
  {"op":"compact"}
  ```
  置位压缩标志，在下一轮推理前触发全量压缩与 `checkpoint`。
* **尾部折叠帧（`fold`）**：
  ```json
  {"op":"fold","from_turn":3,"summary":"使用xx是错误的"}
  {"op":"fold","from_turn":3}
  ```
  按 §6.1 执行边界校验、工具对闭合对齐与手工/自动摘要折叠。
* **后台任务移交帧（`bg_job`，`v0.1.2`）**：
  ```json
  {"op":"bg_job","job_id":"j_a1b2c3d4","pid":12345,"pgid":12345,"cmd":"cargo build --release","log_path":"/tmp/anix-1000/jobs/j_a1b2c3d4.log","exit_file":"/tmp/anix-1000/jobs/j_a1b2c3d4.exit"}
  ```
  `anix` 收到该帧后在进程内登记该后台任务并发出 `bg_job_started` Trace 事件；在后续每轮发起推理前自动检查 `exit_file` 与进程组存活状态（`kill(-pgid, 0)`），对新近结束的任务读取经清洗的尾部日志摘要，追加一条 `runtime_update`（`<runtime_update>Background job ...</runtime_update>`，保持冻结前缀不变）并发出 `bg_job_finished` Trace 事件。

---

## 8. 配置体系、目录解析与 `anix trust` 门禁（Normative）

### 8.1 XDG 路径与向上查找规则

* **全局配置目录**：`$XDG_CONFIG_HOME/anix/`（未设置时回退 `~/.config/anix/`）。
* **状态目录**：`$XDG_STATE_HOME/anix/`（未设置时回退 `~/.local/state/anix/`）。
* **运行时目录**：`$XDG_RUNTIME_DIR/anix/`（未设置时回退 `/tmp/anix-<uid>/`，权限必须为 `0700`）。
* **项目配置查找**：从 `cwd` 逐级向上查找 `.anix/` 目录，**遇到第一个 `.anix/` 即停止**；若向上到达 `.git` 所在目录（含该目录本身）或 `$HOME` 仍未找到则停止。

### 8.2 `config.toml` 规范 Schema

未知顶层段或未知键必须按 Fail-Closed 报错退出 `2`。

```toml
[defaults]
provider = "openai-responses"
model = "deepseek-v4.1-flash"
# vision = true              # 可选：全局默认模型原生视觉能力声明（默认 true）
shell = "msh"
max_turns = 20
max_tokens = 128000
# max_output_tokens = 16384  # 可选：全局默认单轮输出上限（省略或为 0 时使用上游默认）
compact_threshold = 0.8
# output = "reply"           # 可选：全局默认 fd 1 输出契约（reply | explicit | none，默认 reply）
# msh_timeout_sec = 120      # 可选：单条 Shell 命令默认前台等待秒数（默认 120）
# max_msh_timeout_sec = 600  # 可选：模型按次指定 timeout_sec 的硬上限（默认 600）
approval = "ask"
stderr = "auto"

[approval]
safe_commands = ["ls", "cat", "head", "tail", "rg", "grep", "find", "pwd", "git status", "git diff", "git log"]
# bin = "~/.config/anix/hooks/approval-decider"   # 可选；项目级配置受 anix trust 校验
# timeout_sec = 30

[vision]
# bin = "~/.config/anix/hooks/vision-bridge"      # 可选；项目级配置受 anix trust 校验
# timeout_sec = 60

[providers.openai-responses]
bin = "anix-provider-openai-responses"
mode = "coprocess"           # 可选："coprocess"（默认）| "oneshot"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
# api_key = "sk-..."         # 可选：静态密钥（api_key_env 非空时优先；两者皆空视作免鉴权端点）
# vision = true              # 可选：Provider 级视觉能力声明（优先于 [defaults].vision）
timeout_sec = 120

# [providers.openai-responses.params]
# reasoning_effort = "medium"

[compact]
# provider = "openai-responses"
# model = "deepseek-v4.1-flash"
# prompt = "Summarize the conversation preserving decisions, file paths, constraints, and open tasks."
# prompt_file = "~/.config/anix/compact.prompt.md"
# bin = ""                   # 可选外部压缩器（最高优先；项目级配置受 anix trust 校验）

[sandbox]
# enabled = false            # 可选：Linux Landlock 沙盒开关（见 RUNTIME.md）
# strict = false
```

### 8.3 钩子覆盖与 `anix trust` 安全门禁

* **systemd 风格覆盖**：合并全局与项目 `hooks/<stage>.d/` 目录，按文件名 ASCII 字典序执行具备 `+x` 权限的文件；项目级同名文件覆盖全局；项目级同名空文件（0 字节）或指向 `/dev/null` 的软链用于屏蔽全局或内置钩子。核心内置钩子 `00-posix-runtime`（以及参考发行包附带的 `20-skills` / `30-tips`，见 [RUNTIME.md](RUNTIME.md#6-内置增强上下文钩子20-skills-与-30-tips)）均按此规则接受覆写与屏蔽。
* **防 RCE 强校验**：项目级 `config.toml` 的声明式参数默认生效；但项目级 `hooks/` 下的任何可执行脚本，以及项目级 `.anix/config.toml` 中配置的以下外部可执行路径默认**严禁执行**：
  * `[approval].bin`
  * `[vision].bin`
  * `[compact].bin`
  * `[providers.<name>].bin`
  用户必须显式运行 `anix trust <project_dir>`，将每个钩子脚本及自定义可执行文件的规范绝对路径（`realpath`）与内容 SHA-256 摘要写入 `$XDG_STATE_HOME/anix/trusted.json`：

```json
{"version":1,"hooks":{"/abs/path/.anix/hooks/pre-context.d/10-custom":{"sha256":"<hex>","trusted_at":"2026-11-24T12:00:00Z"}}}
```

运行时若检测到未信任的项目级可执行文件或文件内容 SHA-256 发生变更，必须立即中止执行、向 `fd 2` 输出未信任路径并以退出码 **`75`**（`EX_TEMPFAIL`）退出。

---

## 9. Canonical IR 与外部插件线协议（`ir_version = 1`）（Normative）

所有外部插件（Provider、Hooks、外部审批/视觉/压缩插件）均为语言无关的可执行进程，通过 `stdin`/`stdout` 交换 UTF-8 JSON 数据。

### 9.1 Canonical IR v1 规范（`ir_version = 1`）

Canonical IR 是 `anix` 核心与外部 Provider、`pre-context.d/` 钩子及外部压缩器（`[compact].bin`）之间交换的唯一标准推理请求数据模型。在面向 `pre-context.d/`、`[compact].bin` 及 `oneshot` Provider 时采用**严格 UTF-8 JSONL（每行一条独立 JSON 对象 + `\n`）**；在面向 `coprocess` Provider 时封装为**单行请求信封**（见 §9.2）。

* **不绑定模型（JSONL 流形态）**：IR JSONL 中严禁包含 `model` 字段；`oneshot` 模式下由环境变量 `ANIX_MODEL` 注入，`coprocess` 模式下由每轮请求信封显式携带 `model`，确保同一份 IR 上下文可零修改重放至任意模型或压缩器。
* **所有角色扁平化**：统一采用扁平 `text` 字段 + 可选 `media[]` 附件数组，严禁嵌套 `ContentPart[]`；系统提示词统一表示为 `role: "system"` 消息。
* **流头字段（`header`）**：
  * `ir_version` (`uint32`, 必填)：必须为 `1`；Provider 遇到不支持版本必须向 `stderr` 报错并以退出码 `2` 退出。
  * `type` (`"header"`, JSONL 流第 1 行必填)：固定字面量。
  * `tools` (`ToolDef[]`, 必填)：工具声明数组（`shell = none`、全量压缩与自动尾部折叠时为 `[]`，为空时 Provider 必须省略上游工具字段）。
  * `max_output_tokens` (`uint32`, 可选)：单轮输出 Token 上限（未设置或为 `0` 时省略）。存在时 Provider **必须**将其映射至上游 API 的对应输出上限参数，严禁静默忽略。
  * `params` (`object`, 可选)：经运行时浅合并后的模型透传参数字典（为空时省略），由 Provider 自行解释与映射；Provider 遇到不支持的键时**应当**忽略或向 `stderr` 输出非致命警告，严禁因此失败退出。
  * `meta` (`Meta`, 必填)：`{"stderr_mode": "auto"|"explicit", "output_mode": "reply"|"explicit"|"none", "cwd": "<abs_path>"}`，可选含 `turn_budget`（`output_mode` 为 `v0.2.0` 新增可选字段，保持 `ir_version = 1` 前向兼容）。
  * *前向兼容*：`ir_version = 1` 下遇到未识别扩展字段（如 `x_*`）必须忽略而非报错；钩子应当原样透传未修改字段。
* **消息项（按 `role` 区分，顺序即时间顺序）**：
  * **`role = "system"`**：`turn_id` (`uint64`, 必填)、`text` (`string`, 必填)。首条 `role: "system"` 为冻结系统前缀，Provider 必须将其映射至上游可缓存的系统前缀位置且严禁改动其字节内容以保 KV-Cache（如 Anthropic / OpenAI Chat Completions 映射至 `system`；OpenAI Responses 映射至 `input[0]` 的 `role: "developer"` 消息而非顶层 `instructions`，因 GPT-6.2+ 将顶层 `instructions` 追加在 `input` 之后且不计入前缀缓存，顶层 `instructions` 保留给 `params.instructions` 承载动态单轮指令）；尾部因 `runtime_update` 追加的 `role: "system"` 若上游 API 不支持中间 `system` 角色，由 Provider 降级映射为 `user` 消息。
  * **`role = "user"`**：`turn_id` (`uint64`, 必填)、`text` (`string`, 必填)、`media` (`MediaAttachment[]`, 可选，无附件时省略)。
  * **`role = "assistant"`**：`turn_id` (`uint64`, 必填)、`text` (`string`, 可选)、`reasoning` (`string`, 可选)、`tool_calls` (`ToolCall[]`, 可选，每项含 `id`、`name`、`arguments` JSON 对象；`text` 与 `tool_calls` 不可同时为空)、`provider_opaque` (`object`, 可选，跨 Provider 切换时必须清空)。
  * **`role = "tool"`**：`turn_id` (`uint64`, 必填)、`tool_call_id` (`string`, 必填，精确匹配紧邻前序 `assistant.tool_calls[].id`)、`is_error` (`bool`, 可选，默认 `false`)、`text` (`string`, 必填)、`media` (`MediaAttachment[]`, 可选)。
* **多模态附件（`MediaAttachment`）**：
  * 结构：`{"mime": "<mime>", "path": "<abs_path>"}`。`mime` 仅允许 `image/png`、`image/jpeg`、`image/webp`、`image/gif`。
  * **延迟读盘契约**：`anix` 严禁将 Base64 写入 IR 或 WAL；由 Provider 在发起 HTTP 请求前按 `path` 读盘编码。文件不存在或不可读时，Provider 必须输出 `{"type":"error","code":"media_not_found","retryable":false}`（`oneshot` 模式同时以退出码 `1` 退出）。
* **WAL 特殊事件归一化映射**：
  * `checkpoint`：以该节紧随其后的新冻结 `role: "system"` 消息作为首条系统前缀，并将摘要行作为随后的 `role: "user"` 消息注入：`<checkpoint>\n{summary}\n</checkpoint>`。
  * `fold`：弹出 `turn_id >= from_turn`（经工具对闭合对齐后）的全部消息（含触发折叠的当轮 `assistant`），原位压入一条 `role: "user"` 文本：`<folded turns="{from_turn}..{to_turn}">{summary}</folded>`。
  * `runtime_update`：严禁改动首条冻结 `role: "system"` 消息；在漂移处注入尾部 `role: "system"` 增量消息（文本为 `<runtime_update>{content}</runtime_update>`）。
* **发送前结构不变量**：(1) 首行必为 `header` 且后续至少包含一条非 `header` 消息；(2) `assistant.tool_calls[].id` 必须在紧随其后的连续 `tool` 消息中一一配对，严禁孤立或穿插其他角色；(3) 除同一 `checkpoint` 分节内冻结 `system` 与首条摘要可共享起始锚点外，`turn_id` 整体非降序且对话轮次严格单调递增。

JSONL 形态示例：

```json
{"ir_version":1,"type":"header","tools":[{"name":"msh","description":"Execute a non-interactive shell command. If it exceeds timeout_sec, it automatically yields to background (unless kill_on_timeout=true) and returns pid/pgid/log path; set timeout_sec=0 to start in background immediately. Follow the <posix_runtime> contract for output delivery and $ANIX_ERR.","parameters":{"type":"object","properties":{"cmd":{"type":"string"},"timeout_sec":{"type":"integer","minimum":0},"kill_on_timeout":{"type":"boolean"}},"required":["cmd"]}}],"max_output_tokens":16384,"params":{"reasoning_effort":"high"},"meta":{"output_mode":"reply","stderr_mode":"auto","cwd":"/abs/path/to/repo"}}
{"turn_id":0,"role":"system","text":"<posix_runtime>...</posix_runtime>\n\n你是一个安全审计专家..."}
{"turn_id":1,"role":"user","text":"分析这个报错截图"}
{"turn_id":2,"role":"assistant","reasoning":"...","text":"","tool_calls":[{"id":"call_1","name":"msh","arguments":{"cmd":"aview err.png"}}]}
{"turn_id":3,"role":"tool","tool_call_id":"call_1","text":"[t3] exit=0\n","media":[{"mime":"image/png","path":"/tmp/aview_xxx.png"}]}
```

### 9.2 Provider Adapter 线协议（默认 `coprocess`，支持 `oneshot` 回退）

Provider 驱动通过标准 `stdin`/`stdout` JSONL 与 `anix` 通信。默认采用长驻协进程模式（`mode = "coprocess"`）以复用 HTTP/2 连接池与 TLS 会话，同时保留 `mode = "oneshot"` 兼容无状态单次脚本驱动。

#### 公共环境注入

拉起 Provider 进程时注入环境变量：`ANIX_PROVIDER_BASE_URL`、`ANIX_PROVIDER_API_KEY`（优先从 `api_key_env` 对应非空环境变量解析，否则回退 `api_key`；两者皆空时传空串以支持本地/内网免鉴权端点，Provider 应省略 `Authorization` 请求头）、`ANIX_PROVIDER_TIMEOUT_SEC`、`ANIX_MODEL`、`ANIX_PROVIDER_MODE`（`"coprocess"` | `"oneshot"`）、`ANIX_SESSION_ID`（`v0.1.1`：当前会话的稳定路由亲和标识，内置压缩器调用复用主会话 `session_id`）。

#### 模式 A：长驻协进程模式（`mode = "coprocess"`，默认）

* **生命周期**：
  * `anix` 在首次发起推理请求时按 Provider 实例拉起单个子进程，并在整个会话（及 `--inbox` 常驻期）内保持存活。
  * 会话结束或闲置超时退出时，`anix` 关闭 Provider 的 `stdin`（发送 `EOF`）；Provider 收到 `EOF` 后 **必须（MUST）** 优雅清理并退出（`exit 0`）。
  * 若 Provider 在请求间隙异常崩溃或意外关闭 `stdout`，`anix` **应当（SHOULD）** 在下一轮按需重新拉起，不得破坏主状态机与 WAL。
* **请求帧协议（`stdin`）**：每轮请求写入**单行完整 UTF-8 JSON 请求信封**（以 `\n` 结尾）：
  ```json
  {"ir_version":1,"type":"request","request_id":"req_1","session_id":"550e8400-e29b-41d4-a716-446655440000","model":"deepseek-v4.1-flash","timeout_sec":120,"header":{"tools":[...],"max_output_tokens":16384,"params":{"reasoning_effort":"high"},"meta":{"output_mode":"reply","stderr_mode":"auto","cwd":"/abs/path/to/repo"}},"messages":[{"turn_id":0,"role":"system","text":"..."},{"turn_id":1,"role":"user","text":"..."}]}
  ```
  其中 `model`（省略回退 `ANIX_MODEL`）、`timeout_sec`（省略回退 `ANIX_PROVIDER_TIMEOUT_SEC`）与 `session_id`（`v0.1.1`，省略回退 `ANIX_SESSION_ID`）允许同一协进程按轮切换。
* **响应帧协议（`stdout`）**：流式输出每行一条 JSONL 事件帧，每轮请求**必须（MUST）**以唯一终结帧 `{"type":"done",...}` 或 `{"type":"error",...}` 收尾，随后继续阻塞等待 `stdin` 下一行请求（收到 `error` 后协进程**不退出**）。

#### 模式 B：单次调用模式（`mode = "oneshot"`，兼容回退）

* 每轮推理新起一次 Provider 子进程，`stdin` 写入 §9.1 多行 Canonical IR JSONL 流后立即关闭写端（`EOF`），`stdout` 流式输出同样的 JSONL 事件帧，完成后进程退出 `0`（失败时输出 `error` 帧并以非零退出码退出）。

#### 标准输出事件帧（两种模式共用）

```json
{"type":"reasoning_delta","text":"..."}
{"type":"text_delta","text":"..."}
{"type":"tool_call","id":"call_1","name":"msh","arguments":{"cmd":"..."}}
{"type":"usage","input_tokens":4200,"output_tokens":380,"cached_tokens":3600,"cache_write_tokens":400,"reasoning_tokens":200}
{"type":"opaque","data":{...}}
{"type":"error","code":"rate_limit","message":"429 Too Many Requests","retryable":true}
{"type":"error","code":"unsupported_media","message":"Upstream model does not support image inputs","retryable":false}
{"type":"done","finish_reason":"tool_calls"}
```

* **`usage` 包含式口径契约**：
  1. 统一 5 字段结构贯穿 Provider 事件、WAL（`assistant.usage` / `fold.fold_usage` / `checkpoint.compact_usage` / `checkpoint.cumulative_usage`）、Trace 与 `on-exit.d/`，严禁写入 Canonical IR 请求。
  2. `input_tokens`（`uint64`, 必填）必须采用**包含式（Inclusive）**总输入口径，包含 `cached_tokens` 与 `cache_write_tokens`；`output_tokens`（`uint64`, 必填）必须采用包含式总输出口径，包含 `reasoning_tokens`。
  3. `cached_tokens`、`cache_write_tokens`、`reasoning_tokens`（`uint64`, 可选）未提供时按 `0` 处理。
  4. 单次进程内的运行累计消耗（`run_usage`）必须包含本次运行中所有 Provider 调用（含内置压缩器调用）。
* **`opaque` 状态透传**：若本轮收到 `opaque` 事件，`anix` 必须将其 `data` 持久化写入该轮 `role: "assistant"` 的 `provider_opaque` 字段，并在后续调用同一 Provider 时原样回传（跨 Provider 切换时清空）。
* **重试归属契约**：Provider 驱动严禁在内部自行隐式重试；遇到上游限流（`429`）或临时服务端错误（`5xx`）时输出 `retryable: true`；遇到媒体不支持（`unsupported_media`）、媒体缺失（`media_not_found`）或客户端参数错误（`400`/`401`/`403`）时输出 `retryable: false`。由主进程 `anix` 统一执行指数退避重试（默认最多重试 3 次，退避间隔 `1s` / `2s` / `4s`，每次失败均落盘 `provider_error` Trace 事件）；不可重试错误或重试耗尽时向 `fd 2` 报错并以退出码 `1` 退出。
* **异常与超时退出码**：单轮推理超时以退出码 `124` 退出；Provider 未输出 `done`/`error` 即意外关闭 `stdout` 或异常退出且无法恢复时，向 `fd 2` 输出 Provider 的 `stderr` 并以退出码 `1` 退出。

### 9.3 Lifecycle Hooks 线协议（`run-parts` 模式）

* **`pre-context.d/`**：`stdin` 接收 Canonical IR JSONL 流，`stdout` 输出修改后的 Canonical IR JSONL 流（钩子可直接追加或改写 `role: "system"` / 其他消息行）。退出码 `0` 表示继续，非零退出则中止整个 `anix` 进程（透传报错至 `fd 2` 并退出 `1`）。
* **`post-tool.d/`**：`stdin` 接收 `{"turn_id":3,"tool_call_id":"call_1","cmd":"...","exit_code":0,"output":"..."}`，`stdout` 输出修改/脱敏后的同构 JSON；非零退出中止执行。
* **`on-exit.d/`**：进程退出前触发，`stdin` 接收 `{"exit_code":0,"turns":4,"usage":{...},"session_usage":{...},"context":{"tokens":42100,"max_tokens":128000},"context_file":"..."}`，单钩子默认超时 `5s`，失败仅记录至 Trace 不覆盖主进程退出码。

### 9.4 结构化 Trace 线协议（`--trace-fd=<N>` / `ANIX_TRACE_FD=<N>` / `journal.sock`）

1. **显式按需激活与非阻塞铁律**：
   * Trace 默认完全关闭（不写入任何磁盘或额外 FD，**严禁**默认探测或写入 `fd 3`）。
   * 仅当显式指定 `--trace-fd=<N>` 或 `ANIX_TRACE_FD=<N>` 时向该 FD 输出 JSONL；未指定时，仅在 `$XDG_RUNTIME_DIR/anix/journal.sock`（Unix Domain `SOCK_STREAM`）显式存在时尝试连接采集器，不存在则静默保持关闭。
   * 写入遇到 `EPIPE` 或对端关闭时必须静默关闭 Trace 写端，**严禁阻塞推理循环或影响主进程退出码**；严禁将 `ANIX_PROVIDER_API_KEY` 等密钥写入 Trace。
2. **公共信封字段**（单行 UTF-8 JSONL，支持父子 Agent 嵌套溯源）：
   ```json
   {"v":1,"ts":"2026-11-24T13:30:00.123Z","pid":48210,"ppid":48102,"agent":"security-auditor","type":"..."}
   ```
3. **三级存储粒度（`ANIX_TRACE_LEVEL`）**：
   * `events`（`journal.sock` 推荐默认）：记录完整嵌套生命周期事件（`run_start`、`cycle_start`、`llm_request_start`、`llm_response`、`provider_error`、`tool_approval`、`tool_start`、`tool_end`、`bg_job_started`、`bg_job_finished`、`compact`、`fold`、`cycle_end`、`idle`、`warning`、`error`、`run_end`）：
     * `run_start`：携带 `session_id`（`v0.1.1`）、`provider`、`model`、`shell`（`v0.2.0`）、`output`（`v0.2.0`）、`vision`、`fresh_session` 与 `inbox_mode`；
     * `cycle_start`：每次进入推理循环前发出（单次模式与 `--inbox` 模式同构），携带 `cycle_id`（从 `1` 单调递增）、`source`（初始输入为 `"initial"`，Inbox 消息为文件名 `<name>`）与 `start_turn_id`；
     * `cycle_end`：作为**严格提交屏障**，在该次推理循环结束、WAL 已原子落盘且 `--inbox` 消息已移入 `done/` **之后**发出，携带 `cycle_id`、`source`、`reason`（`"completed"` | `"max_turns"` | `"max_tokens"` | `"approval_blocked"` | `"provider_error"` | `"error"` | `"interrupted"`）、`exit_code`、`turns`、`cycle_usage`、`usage`、`session_usage` 与 `context` 水位对象；
     * `idle`：仅 `--inbox` 模式在 `new/` 队列排空、进入事件阻塞待命时发出一次（携带 `idle_timeout_sec`），直至新消息到达前不重复发出；
     * `llm_response` 携带当轮 5 字段 `usage`；
     * `tool_approval` 携带 `decider`（`"builtin"` | `"external"`）与可选 `decider_exit_code`（仅 `external` 时存在）；
     * `bg_job_started` / `bg_job_finished`（`v0.1.2`）：`bg_job_started` 携带 `job_id`、`pid`、`pgid`、`cmd`、`log_path`、`exit_file`；`bg_job_finished` 携带 `job_id`、`pid`、`pgid`、`exit_code`（外部强杀缺失 `.exit` 时为 `null`）、`log_path` 与 `elapsed_ms`；
     * `fold` 在自动摘要折叠时携带可选 `fold_usage`（5 字段对象）；
     * `run_end` 携带 `exit_code`、`turns`、`usage`、`session_usage` 与 `context` 水位对象；
     * 发生网络断连或重试时，`provider_error` 必须立刻落盘并附带 `code`、`attempt`、`elapsed_ms`、`provider_stderr` 及断连前已收到的 `partial_text`/`partial_reasoning` 与 `last_delta_ts`。
   * `deltas`（显式 `--trace-fd` 推荐默认）：在 `events` 基础上追加每一帧流式 `reasoning_delta` / `text_delta`。
   * `wire`（显式排障开启）：额外落盘完整 Canonical IR 输入、Provider 原始输出帧与 Shell 原始输出；**严禁发往共享的 `journal.sock`**，仅允许写入权限为 `0600` 的独立排障文件。

---

## 附录 A：`0.x` 核心演进变更与兼容矩阵

| 特性 | 归属章节 | 配置 / 协议变更 | 默认值 | `anix trust` 要求 | 兼容性 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 外部审批裁决器 | §5.2, §8.2, §9.4 | `config.toml` `[approval].bin` / `timeout_sec`；Trace `decider` / `decider_exit_code` | 空（内置审批） | 项目级必填校验 | 向后兼容增量 |
| 视觉能力声明 | §3.1, §3.3, §4.4, §5.5 | CLI `--vision`/`--no-vision`、`ANIX_VISION`、Frontmatter / `config.toml` `vision` | `true` | 不适用 | 向后兼容增量 |
| 外部视觉转译器 | §5.5, §8.2 | `config.toml` `[vision].bin` / `timeout_sec` | 空（拦截报 `is_error`） | 项目级必填校验 | 向后兼容增量 |
| Provider 视觉兜底 | §5.5, §9.2 | 错误事件 `code: "unsupported_media"` | `retryable: false` | 不适用 | 向后兼容增量 |
| 自动尾部折叠 | §6.1, §6.2, §7.2, §9.4 | `fold` 帧省略 `summary`；WAL/Trace 可选 `fold_usage` | 复用 `[compact]` | 复用 `[compact].bin` | 向后兼容增量 |
| `fd 2` 尾部换行契约 | §4.1 | `auto` 模式阶段切换与 `[usage]` 汇总行强制 `\n` 收尾（不侵入 `fd 1`） | 默认启用 | 不适用 | 向后兼容增量 |
| 循环级观测信号 | §4.1, §4.2, §9.4 | Trace 新增 `cycle_start` / `cycle_end`（提交屏障）与 `idle`；`--inbox` 每轮输出 `[usage]` | 激活 Trace 时发出 | 不适用 | 向后兼容增量 |
| 可配置观测 FD | §3.1, §4.1, §9.4 | CLI `--trace-fd=<N>` / `ANIX_TRACE_FD=<N>`（废除硬编码 `fd 3`） | 未指定时关闭 | 不适用 | **破坏性收口（`v0.1`）** |
| Provider 协进程模式 | §8.2, §9.2 | Provider `mode = "coprocess" \| "oneshot"`，单行 JSON 请求信封 + 连接池 | `coprocess` | 不适用 | **破坏性收口（`v0.1`）** |
| 标准化退出码表 | §5, §6.5, §8.3 | 对齐 `sysexits.h`，用 `75`（`EX_TEMPFAIL`）取代 `42`；保留 `124` | 默认启用 | 不适用 | **破坏性收口（`v0.1`）** |
| 纯事件驱动等待 | §4.2, §6.3 | 禁止固定间隔轮询，使用 `pidfd`/`kqueue` + `signalfd`/`self-pipe` + 精确超时 | 默认启用 | 不适用 | 内部实现升级 |
| 品牌与命名统一（Anix） | 全局 | `anix`、`anix-provider*`、`.anix/`、`libexec/anix`、`ANIX_*`（保留 `*.agent`） | `anix` | 不适用 | **破坏性收口（`v0.1`）** |
| 会话路由亲和（`v0.1.1`） | §6.2, §9.2, §9.4 | WAL `session_id`、Provider `ANIX_SESSION_ID` / 信封 `session_id`、Trace `run_start.session_id` | 自动 UUID v4 | 不适用 | 向后兼容增量 |
| 按需超时与后台任务帧（`v0.1.2`） | §7.1, §7.2, §8.2, §9.4 | `timeout_sec`/`kill_on_timeout`、`max_msh_timeout_sec`、`$ANIX_CTL` `bg_job` 帧、`<runtime_update>` 完成通知 | `120s` / 上限 `600s` | 不适用 | 线格式兼容（参考 Shell 默认超时改为转后台） |
| 三轴正交输出契约（`v0.2.0`） | §3.1, §3.3, §3.4, §4.1, §4.3, §4.4, §6.2, §8.2, §9.1, §9.4 | `shell: <bin> \| none`、`output: reply \| explicit \| none`（`-o`/`ANIX_OUTPUT`）、启动 Fail-Fast（exit 2）、显式关闭 `fd 4` | `output = reply` | 不适用 | **默认输出契约升级（旧 WAL 回退按 `explicit`）** |
| 规范与运行底座分层（`v0.2.0`） | §1.3, §2 | `RFC.md` 收口为纯 Core 规范，参考组件（`msh`/`a*`/`anix-provider`/`skills`/`tips`/打包）拆出至 [RUNTIME.md](RUNTIME.md)，上层壳与生态独立分仓 | 四层架构 | 不适用 | 文档结构收口 |
