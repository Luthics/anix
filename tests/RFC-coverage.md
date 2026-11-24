# v1.0 RFC 测试索引

规范见 [v1.0.0-RFC.md](../docs/v1.0.0-RFC.md)。下表说明每组测试如何观察结果。同一用例可以覆盖多个条款。用例数量不等于覆盖率，规范待决的问题也不算通过。

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

## 复跑

```sh
cargo test -p os-agent-core --test rfc_3_2
cargo xtest --no-fail-fast
python3 tests/test_distribution.py
```

安装链路使用真实安装脚本、临时 HOME 和本机 HTTP 服务，CI 在 Linux 与 macOS 的测试任务中运行它。

## 尚待决定或验证的条款

- 无 frontmatter 的空行及 CRLF 字节语义、Inbox 领取后崩溃的恢复策略、非法 fold 的退出码归属、默认适配器的免鉴权流程，各有独立讨论。[#1](https://github.com/Luthics/os-agent/issues/1) · [#2](https://github.com/Luthics/os-agent/issues/2) · [#3](https://github.com/Luthics/os-agent/issues/3) · [#4](https://github.com/Luthics/os-agent/issues/4)
- §§4–6 尚需覆盖同时投递、钩子和 Provider 的描述符继承、自动压缩与硬上限的等值和相邻边界，以及忽略 TERM 后的 KILL 回收。
- §§7–9 尚需覆盖 PDF 页码、跨 Provider 的 opaque 数据、完整的 1/2/4 秒退避、钩子超时、索引六层优先级和 Trace 消费者断开。
- §10.2 仍需区分可用与不可用 Landlock 内核的行为、父进程异常退出后的子进程回收。§11 在 RFC 中标为 Non-Normative。
