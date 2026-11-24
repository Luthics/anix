# v1.0 RFC 测试索引

按用例查找规范原文请打开 [RFC 逐测试索引](RFC-cases.md)。索引读取每个 Rust 和 Python 测试旁的条款标记。下表说明各组测试的观察点。[v1.0.0-RFC.md](../docs/v1.0.0-RFC.md)

| RFC | 已有断言与观察点 | 测试入口 |
| --- | --- | --- |
| §3.1–3.2 | 验证可选参数分隔符及字面量；逐项改变 LF/CRLF、shebang、YAML 起止符、EOF 和编码。解析失败时检查退出码、stderr 以及 Provider 未启动。 | [脚本语法](../crates/os-agent-core/tests/rfc_3_2/grammar.rs)、[编码](../crates/os-agent-core/tests/rfc_3_2/encoding.rs)、[脚本 CLI](../crates/os-agent-core/tests/rfc_3_2/cli.rs)、[进程 I/O](../crates/os-agent-core/tests/rfc_runtime/io.rs) |
| §§3.3–3.4、8.1–8.2 | 遍历名称的 ASCII 字节、整数边界、比例阈值、五层覆盖的全部 32 种子集；从 Provider 捕获最终 model、输出上限、按键合并的 params。从磁盘核对空 context 的效果。 | [配置](../crates/os-agent-core/tests/rfc_runtime/configuration.rs) |
| §§4.1、4.3、5.1–5.3 | 分别从真实管道、常规文件、控制终端观察输入顺序、二进制输出、stderr 与 Trace；操作独立 PTY 验证 y/n/a/e、编辑权限和清理。剥离控制终端后断言 42 及无副作用。 | [进程 I/O](../crates/os-agent-core/tests/rfc_runtime/io.rs)、[终端](../crates/os-agent-core/tests/rfc_runtime/terminal.rs)、[审批策略](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs) |
| §§4.2、6.3–6.4 | 原子投递后核对消费顺序、WAL 和 done 文件；在锁持续持有或 Provider 正输出半轮时发送信号。覆盖 INT/TERM 的退出码与释放锁、STOP/CONT 的停止状态、USR1 的下一轮压缩和 USR2 的快照内容、路径、权限。 | [Inbox](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs)、[信号](../crates/os-agent-core/tests/rfc_runtime/signals.rs) |
| §§6.1–6.2、9.2 | 检查逐字节截断、逆读跨块边界、独占锁、漂移尾追加、折叠后 ID、压缩与工具配对。从 WAL、IR、Trace、on-exit 四处分别核对五字段用量、压缩计费与恢复后水位。 | [WAL](../crates/os-agent-core/tests/rfc_runtime/wal.rs)、[压缩](../crates/os-agent-core/tests/rfc_runtime/compaction.rs) |
| §7.1 | 在第 8192 字节两侧插入 NUL，改变 UTF-8、CSI、OSC、回车、行数与字节上限，读取落盘文件核对完整内容。另从 msh 入口核对环境覆盖、合流、退出码及超时。 | [msh](../crates/msh/tests/rfc_shell.rs) |
| §7.2 | 从实际 fd 6 读取紧凑 JSONL，验证非法参数、转义后的帧大小；读取 aview 输出的文件并解码 PNG，对照长宽、尺寸与权限。 | [控制帧](../crates/agent-tools/tests/rfc_control.rs) |
| §§4.4、8.3、9.3–9.4、9.6 | 在临时 HOME 中创建全局与项目钩子、掩码、skills 和 tips；核对信任前后及 hash 变化、钩子执行顺序、post-tool 回写、on-exit 故障与首轮冻结索引。 | [生命周期](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs)、[恢复](../crates/os-agent-core/tests/rfc_runtime/wal.rs) |
| §9.1 | 独立构造确定的 JSONL；逐例检查必填字段、序列化、工具配对、空助手消息、普通轮次与 checkpoint 共享锚点的区别。 | [Canonical IR](../crates/os-agent-proto/tests/rfc_ir.rs) |
| §9.2 | 在回环地址接收实际 HTTP 请求，模拟 SSE 分片、重复工具帧、无结束帧、429/5xx 和缺失媒体。另从运行时捕获重试前后的 IR 与半途断开的 Trace。 | [适配器](../crates/provider-openai-responses/tests/rfc_responses.rs)、[运行时重试](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs) |
| §9.5 | 分别打开普通文件、只读 fd 3、Unix socket，核对事件/增量/原始 IR 的去向、凭据占位符未写出，以及 wire 文件权限。 | [Trace](../crates/os-agent-core/tests/rfc_runtime/trace.rs) |
| §§2、10.3 | 在独立 HOME 安装五个程序，以 shebang 启动 agent，驱动真实 Provider、msh、aview 和 aclr；核对上游收到的图像、WAL 里的路径、fd 1 的原始字节和折叠后的下一次请求。自检另覆盖文件缺失、执行位、配置语法、凭据与运行目录。 | [安装链路](test_distribution.py)、[doctor](../crates/os-agent-core/tests/rfc_runtime/doctor.rs) |
| §§4.1–4.3、6.3 | 轮换 fd 4–9 的四种访问模式，在 pre-context、Provider、shell 中核对已消费文件的身份。同一工具仍在执行时，从三个线程原子发布不同的大消息，检查队列状态、完整正文、工具闭合和 WAL 顺序。独立进程处理 TERM 后继续运行，验证三秒宽限和退出。 | [FD](../crates/os-agent-core/tests/rfc_runtime/io.rs)、[Inbox](../crates/os-agent-core/tests/rfc_runtime/inbox.rs)、[信号](../crates/os-agent-core/tests/rfc_runtime/signals.rs) |
| §6.1 | 对相同窗口测量硬上限前一 token、等值、后一 token；按 UTF-8 字节增加输入观察水位，分别在自动阈值两侧观察调用次数和压缩后的第二次硬上限检查。通过不同二进制、模型和提示词文件验证内置压缩器的选择。 | [压缩](../crates/os-agent-core/tests/rfc_runtime/compaction.rs) |
| §7.2 | 用四页宽高比各异的真实 PDF，从文件和 stdin 选择第 1、2、4 页；解码 fd 6 上每条 JSON，读取 PNG 文件的 IHDR 尺寸与权限。 | [PDF 页码](test_pdf_pages.py) |
| §§9.2–9.6 | 核对四次请求、三次逐次退避延时、耗尽后的 stderr 和退出码；从两个不同 Provider 收到的 IR 检查 opaque 状态。覆盖前置和后置钩子失败、on-exit 的五秒超时、skills 六层覆盖、tips 版本刷新和运行目录回退，以及 Trace 对端断开。 | [Provider](../crates/os-agent-core/tests/rfc_runtime/provider.rs)、[钩子](../crates/os-agent-core/tests/rfc_runtime/hooks.rs)、[Trace](../crates/os-agent-core/tests/rfc_runtime/trace.rs) |
| §10.2 | 在支持 Landlock 的内核上操作独立测试文件，观察可写范围和只读文件；让测试子进程的内核探测返回 ENOSYS，分别核对 strict、best-effort 的退出码和 Provider 调用。向测试所有的父进程发送 SIGKILL，检查活跃 Provider 或 shell 已终止。 | [沙箱](../crates/os-agent-core/tests/rfc_runtime/platform.rs)、[不可用内核](test_linux_fallback.py)、[父进程退出](../crates/os-agent-core/tests/rfc_runtime/signals.rs) |
| §§3.4、9.6（178cd0e） | 从 msh 的实际环境核对全部 AGENT_* 键，覆盖观测变量未设、为空和有值、最终审批、侧信道与普通环境变量；两个真实 agent 嵌套执行，验证子模型、WAL、独立预算及 auto/deny。读取 how-to-background.md 的实际索引，再逐项覆盖与屏蔽。 | [环境白名单](../crates/os-agent-core/tests/rfc_runtime/io.rs)、[嵌套进程](test_shell_pipeline.py)、[指南索引](../crates/os-agent-core/tests/rfc_runtime/hooks.rs) |
| §§6.2、7.1、9.1–9.2、9.5 | 对连续三次折叠的所有活跃锚点，用独立正向模型逐步核对逆读窗口和消耗；四种格式的实际图像经用户和工具消息传入，逐字节比对适配器 HTTP 数据；真实 msh 的超时、shell 解析和 wire 原始输出有独立断言。 | [WAL 正向模型](../crates/os-agent-core/tests/rfc_runtime/wal.rs)、[媒体与计费](../crates/provider-openai-responses/tests/rfc_responses.rs)、[shell 链路](test_shell_pipeline.py) |
| §§6.1–6.2、7.2、9.2 | 同一工具连续写两个合法 fold，逐条核对 WAL；对媒体测试合并写入、跨 UTF-8 字节拆写、重复附件和重启。失败尝试上报用量后成功重试，分别核对实际计费与最后上下文水位。 | [控制帧接收](../crates/os-agent-core/tests/rfc_runtime/control.rs)、[预算与计费](../crates/os-agent-core/tests/rfc_runtime/compaction.rs) |
| §§3.4、4.1、5.1–5.2、8.3 | 枚举五层配置的 32 个子集，核对预算、压缩阈值、shell、Provider、审批、stderr、队列和会话路径。在八种终端组合下核对真实字节与静态汇总。分进程验证 Always 的作用域、编辑取消，以及自定义 Provider 和压缩器内容变更后的信任校验。 | [配置](../crates/os-agent-core/tests/rfc_runtime/configuration.rs)、[终端输出](test_shell_pipeline.py)、[审批](../crates/os-agent-core/tests/rfc_runtime/terminal.rs)、[信任](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs) |

## 复跑

```sh
cargo test -p os-agent-core --test rfc_3_2
python3 scripts/rfc-test-index.py --check
cargo xtest --no-fail-fast
python3 tests/test_distribution.py
python3 tests/test_shell_pipeline.py
```

安装链路使用真实安装脚本、临时 HOME 和本机 HTTP 服务，CI 在 Linux 与 macOS 的测试任务中运行它。

默认安装入口为仓库内的 `scripts/install-local.sh`。验收发行版时可以设置 `OS_AGENT_BOOTSTRAP=/absolute/path/to/installer.sh`，向 `sh` 的标准输入提供本地发布脚本。首次执行保留安装后的全局配置和钩子，通过项目配置把推理请求发到回环服务。发布入口与配置是否必须生成实体文件见 [#7](https://github.com/Luthics/os-agent/issues/7)。

Linux 组件测试使用 Poppler 和 strace；macOS 可用 Ghostscript 运行同一 PDF 用例。

```sh
cargo build -p os-agent-core -p agent-tools
python3 tests/test_pdf_pages.py
python3 tests/test_linux_fallback.py  # Linux
```

## 尚待决定或验证的条款

- 无 frontmatter 的空行及 CRLF 字节语义、Inbox 领取后崩溃的恢复策略、非法 fold 的退出码归属、默认适配器的免鉴权流程，各有独立讨论。[#1](https://github.com/Luthics/os-agent/issues/1) · [#2](https://github.com/Luthics/os-agent/issues/2) · [#3](https://github.com/Luthics/os-agent/issues/3) · [#4](https://github.com/Luthics/os-agent/issues/4)
- §6.1 尚需明确估算的字节集合、自动压缩在等值时的行为。§§6.2、9.2 尚需定义旧 WAL 中 opaque 来源未知时的处理，以及切回原 Provider 后如何恢复。[#5](https://github.com/Luthics/os-agent/issues/5) · [#6](https://github.com/Luthics/os-agent/issues/6)
- §10.3 的发布安装入口待确认。§10.1 是容器镜像建议，§11 是 Non-Normative。Linux 的内核契约在真实内核与 musl 构建上验证。WSL2 的宿主集成尚未实机验证。
- 失败尝试已上报用量的 WAL 记账与恢复、多词 safe_commands 的匹配规则和适用模式，各有独立讨论。[#8](https://github.com/Luthics/os-agent/issues/8) · [#9](https://github.com/Luthics/os-agent/issues/9)
- §5.2 的编辑器失败或空文件场景已验证不会执行命令。下一步返还工具错误、重复询问还是退出，待规范确认。[#10](https://github.com/Luthics/os-agent/issues/10)
