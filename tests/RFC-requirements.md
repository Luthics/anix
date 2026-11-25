# v1.1.1 RFC 条款验收表

每行给出一项可观察行为。第一列链接到 RFC 原文，第二列链接到执行断言或待裁决的 issue。通过与失败见对应提交的 CI。

本表按条款逐步补录。[逐测试索引](RFC-cases.md)列出当前套件的全部测试函数。编辑 `tests/rfc-requirements.tsv` 后运行 `python3 scripts/rfc-test-index.py` 更新两份索引。

## §2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [PATH 中的同名 msh 可覆盖私有安装位置，显式路径仍优先](../docs/v1.1.1-RFC.md?plain=1#L56) | [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L473) |

## §3.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [直接传参、可选分隔符及以短横线开头的参数](../docs/v1.1.1-RFC.md?plain=1#L66) | [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/anix-core/tests/rfc_runtime/io.rs#L186) |

## §3.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [最后一轮完成返回 0，仍需推理时返回 124，均不再请求 Provider](../docs/v1.1.1-RFC.md?plain=1#L121) | [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L234) |

## §3.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [嵌套子进程继承实际生效的 auto 或 deny，模型、WAL 和轮数预算独立](../docs/v1.1.1-RFC.md?plain=1#L143) | [`test_nested_anix_uses_own_model_wal_and_budget_and_inherits_cli_auto`](../tests/test_shell_pipeline.py#L537) · [`test_nested_anix_inherits_cli_deny_over_its_own_auto_frontmatter`](../tests/test_shell_pipeline.py#L541) |

## §4.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [有参数的可执行脚本通过 PATH 或相对路径启动，TTY 保持打开即可推理](../docs/v1.1.1-RFC.md?plain=1#L158) | [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L248) |
| [无参数且 stdin 为终端时等待实际 EOF，再将输入交给 Provider](../docs/v1.1.1-RFC.md?plain=1#L158) | [`test_executable_script_on_path_and_by_relative_name_obeys_tty_eof_rules`](../tests/test_provider_streaming.py#L443) · [#34](https://github.com/Luthics/anix/issues/34) |
| [带参数启动，Provider 首帧前是否必须显示等待提示](../docs/v1.1.1-RFC.md?plain=1#L158) | [#36](https://github.com/Luthics/anix/issues/36) |
| [重定向与真实管道只转发显式 Payload，保留原始字节](../docs/v1.1.1-RFC.md?plain=1#L159) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L87) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L272) |
| [末尾无换行的模型文本与使用量行分行显示](../docs/v1.1.1-RFC.md?plain=1#L168) | [`trace_fd_explicit_only_no_implicit_fd3_and_cycle_events`](../crates/anix-core/tests/v1_1_e2e.rs#L161) |
| [流式正文和思考帧在工具状态、诊断侧信道及使用量汇总处的换行；显式模式只输出诊断](../docs/v1.1.1-RFC.md?plain=1#L168) | [`unterminated_deltas_are_separated_from_tool_status_diagnostics_and_usage`](../crates/anix-core/tests/rfc_runtime/io.rs#L393) |
| [在真实 TTY 上精确检查 Dim 使用量行在最后一个换行前重置 ANSI 样式](../docs/v1.1.1-RFC.md?plain=1#L169) | [`test_usage_line_resets_dim_before_the_final_newline_on_a_terminal`](../tests/test_distribution.py#L218) |
| [可读写的显式 Trace FD 不进入静态输入](../docs/v1.1.1-RFC.md?plain=1#L162) | [`each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input`](../crates/anix-core/tests/rfc_runtime/fd.rs#L94) |
| [显式 FD 校验失败时先返回 2；有效时 CLI 优先于环境配置](../docs/v1.1.1-RFC.md?plain=1#L161) | [`invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider`](../crates/anix-core/tests/rfc_runtime/fd.rs#L136) · [`only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment`](../crates/anix-core/tests/rfc_runtime/fd.rs#L61) |
| [真实管道有无脚本参数均等到 EOF，保留跨写入的 UTF-8 及内嵌 NUL](../docs/v1.1.1-RFC.md?plain=1#L158) | [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L238) |
| [逐个通过真实管道连接 fd 4 至 9，读完后保留输入来源和完整正文](../docs/v1.1.1-RFC.md?plain=1#L162) | [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L238) |
| [stdout 与 stderr 各自为终端或普通文件的四种组合，测试 auto 和 explicit](../docs/v1.1.1-RFC.md?plain=1#L159) | [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L408) |

## §4.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [当前工具批次闭合后才取下一条已发布消息](../docs/v1.1.1-RFC.md?plain=1#L187) | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L19) |
| [成功消息先提交，下一条被中断时仍可识别已完成的轮次](../docs/v1.1.1-RFC.md?plain=1#L189) | [`test_inbox_commits_one_message_then_interrupts_the_next_in_the_same_coprocess`](../tests/test_provider_streaming.py#L591) · [#37](https://github.com/Luthics/anix/issues/37) |
| [Linux 空闲时只发一次 idle，新任务结束后重新待命](../docs/v1.1.1-RFC.md?plain=1#L192) | [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L336) |

## §4.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [真实 msh 子进程的标准输入指向空设备，fd 4、5、6 都是只写匿名管道](../docs/v1.1.1-RFC.md?plain=1#L201) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L368) |
| [多次打开输出管道，超过 1 MiB 的二进制 Payload 逐字节完整转发](../docs/v1.1.1-RFC.md?plain=1#L203) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L368) |
| [诊断管道可独立传输超过 1 MiB 的二进制数据](../docs/v1.1.1-RFC.md?plain=1#L204) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L368) |

## §5.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [自动放行和其环境变量别名；外部裁决器不被调用](../docs/v1.1.1-RFC.md?plain=1#L228) | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L571) · [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L319) |
| [deny 保留白名单；拦截时返回工具错误，开启 fail-on-deny 时退出 75](../docs/v1.1.1-RFC.md?plain=1#L229) | [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L319) · [#9](https://github.com/Luthics/anix/issues/9) |
| [Always 对同一可执行程序生效；换进程读取同一 WAL 后需重新审批](../docs/v1.1.1-RFC.md?plain=1#L231) | [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L156) |

## §5.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [外部裁决器读到一行 JSON、真实进程标识及 EOF](../docs/v1.1.1-RFC.md?plain=1#L249) | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L247) |
| [外部返回 0，无控制终端也能执行一次已批准的命令](../docs/v1.1.1-RFC.md?plain=1#L259) | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L247) |
| [外部返回 1，裁剪拒绝理由、为空时给出默认值，继续推理](../docs/v1.1.1-RFC.md?plain=1#L260) | [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L265) |
| [返回 2 后交还终端裁决；无控制终端时退出 75](../docs/v1.1.1-RFC.md?plain=1#L261) | [`test_external_escalation_hands_the_decision_to_the_terminal`](../tests/test_approval.py#L234) · [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L219) |
| [其他非零或超时透出诊断，结束当前轮且不执行工具](../docs/v1.1.1-RFC.md?plain=1#L262) | [`test_external_error_and_timeout_surface_stderr_and_end_the_cycle_with_75`](../tests/test_approval.py#L285) |
| [普通放行、拒绝及被信号中断时记录裁决来源](../docs/v1.1.1-RFC.md?plain=1#L264) | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L247) · [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L265) · [`test_interrupt_during_builtin_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L353) · [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L379) · [#50](https://github.com/Luthics/anix/issues/50) |
| [退出码 2 转交人工、超时或启动失败时如何表示外部退出码](../docs/v1.1.1-RFC.md?plain=1#L264) | [#51](https://github.com/Luthics/anix/issues/51) |

## §5.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [提示与回答走控制终端，管道 stdin 保留为模型输入](../docs/v1.1.1-RFC.md?plain=1#L269) | [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L131) |
| [y 只批准一条，同批下一条可单独拒绝](../docs/v1.1.1-RFC.md?plain=1#L271) | [`test_yes_authorizes_only_one_call_in_a_batch`](../tests/test_approval.py#L206) |
| [n 返回模型错误，支持空理由及 UTF-8 文本](../docs/v1.1.1-RFC.md?plain=1#L272) | [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L131) |
| [a 的授权以可执行程序及 PID 为界](../docs/v1.1.1-RFC.md?plain=1#L273) | [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L156) |
| [e 使用私有临时文件，执行编辑结果并清理；编辑取消或为空时两条命令均未执行](../docs/v1.1.1-RFC.md?plain=1#L274) | [`test_editor_receives_private_original_and_only_the_edited_command_runs`](../tests/test_approval.py#L167) · [`test_failed_or_empty_edit_removes_the_file_and_runs_neither_command`](../tests/test_approval.py#L194) · [#10](https://github.com/Luthics/anix/issues/10) |

## §5.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [无控制终端时打印原命令、记录终止事件并退出 75](../docs/v1.1.1-RFC.md?plain=1#L278) | [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L219) |

## §5.5

| 可验收行为 | 测试与问题 |
| --- | --- |
| [依次覆盖 CLI、环境、脚本、所选 Provider、全局默认和内置默认](../docs/v1.1.1-RFC.md?plain=1#L284) | [`vision_resolves_from_cli_env_script_selected_provider_then_defaults`](../crates/anix-core/tests/rfc_runtime/vision.rs#L10) |
| [无原生视觉且无桥接器时，工具报错且附件不进入恢复的历史](../docs/v1.1.1-RFC.md?plain=1#L289) | [`no_native_vision_without_a_bridge_returns_the_documented_error_through_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L179) |
| [桥接器接收图片信息和 EOF，恢复会话后保留转译描述](../docs/v1.1.1-RFC.md?plain=1#L298) | [`bridge_gets_one_utf8_json_line_then_eof_and_its_description_survives_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L189) |
| [桥接成功的正文边界、空输出及无效 UTF-8 的裁决](../docs/v1.1.1-RFC.md?plain=1#L303) | [#43](https://github.com/Luthics/anix/issues/43) |
| [视觉桥接非零退出或超时成为可恢复的工具错误](../docs/v1.1.1-RFC.md?plain=1#L304) | [`bridge_nonzero_exit_and_timeout_both_return_stderr_as_a_recoverable_tool_error`](../crates/anix-core/tests/rfc_runtime/vision.rs#L216) |
| [上游拒绝媒体时产生专属且不可重试的错误](../docs/v1.1.1-RFC.md?plain=1#L306) | [`test_upstream_media_rejection_is_distinct_from_auth_and_transient_failures`](../tests/test_provider_dialects.py#L215) |

## §6.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [从同批工具的消息锚点折叠时回退到对应 assistant](../docs/v1.1.1-RFC.md?plain=1#L336) | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L321) |

## §6.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [新建会话在压缩、路径别名与恢复后仍使用原 session_id](../docs/v1.1.1-RFC.md?plain=1#L354) | [`persisted_id_survives_builtin_compaction_checkpoint_alias_and_subsequent_resume`](../crates/anix-core/tests/rfc_runtime/session.rs#L122) |
| [无状态运行每进程生成 UUID v4，经过推理和压缩仍一致](../docs/v1.1.1-RFC.md?plain=1#L354) | [`stateless_runs_get_distinct_uuid_v4s_while_all_provider_calls_in_a_run_share_one`](../crates/anix-core/tests/rfc_runtime/session.rs#L100) · [`empty_context_overrides_a_saved_wal_and_keeps_one_ephemeral_id_through_compaction`](../crates/anix-core/tests/rfc_runtime/session.rs#L349) |
| [旧 WAL 的回退值经路径别名与重启稳定，写入 checkpoint 后随文件迁移](../docs/v1.1.1-RFC.md?plain=1#L354) | [`legacy_wal_uses_a_stable_canonical_path_id_across_aliases_and_restarts`](../crates/anix-core/tests/rfc_runtime/session.rs#L161) · [`first_checkpoint_in_a_legacy_wal_persists_the_fallback_id_even_if_the_wal_moves`](../crates/anix-core/tests/rfc_runtime/session.rs#L313) |
| [最新 checkpoint 提供的标识原样到达 Trace、环境和协进程信封](../docs/v1.1.1-RFC.md?plain=1#L379) | [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L195) |
| [从 session_init 和 checkpoint 恢复，stderr、approval、vision 的八种变更组合只在首次变化时产生一条尾部通知](../docs/v1.1.1-RFC.md?plain=1#L351) | [`each_combination_of_stderr_approval_and_vision_drift_creates_one_tail_update`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L635) |
| [旧 runtime_update 只提供部分字段时，缺失的契约配置要从哪里恢复](../docs/v1.1.1-RFC.md?plain=1#L351) | [#17](https://github.com/Luthics/anix/issues/17) |
| [文件增长受限时保留原有字节，只在尾行完整可回放时继续](../docs/v1.1.1-RFC.md?plain=1#L374) | [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L122) |

## §6.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [无截止时间的 Linux 空闲循环持续睡眠，发布消息可以唤醒](../docs/v1.1.1-RFC.md?plain=1#L384) | [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L336) |
| [子进程清理与退出钩子的截止时间在正常和缺少 pidfd 的 Linux 上可检查](../docs/v1.1.1-RFC.md?plain=1#L388) | [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L176) · [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L155) · [#48](https://github.com/Luthics/anix/issues/48) |

## §6.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [Ctrl+C 在等待 Provider 或其关闭 stdout 后回到可继续执行管道的同一 shell](../docs/v1.1.1-RFC.md?plain=1#L394) | [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L493) · [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L550) |
| [在内置审批处只按一次 Ctrl+C，命令不执行，原 shell 恢复并读到退出码 130](../docs/v1.1.1-RFC.md?plain=1#L394) | [`test_ctrl_c_at_the_approval_prompt_returns_to_the_same_shell`](../tests/test_approval.py#L345) · [#49](https://github.com/Luthics/anix/issues/49) |
| [外部审批中收到 SIGTERM，退出 143 并记录中断](../docs/v1.1.1-RFC.md?plain=1#L395) | [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L379) · [#50](https://github.com/Luthics/anix/issues/50) |
| [工具输出一部分后中断，shell 后代收到 TERM、释放 WAL 锁，恢复时没有半截工具结果](../docs/v1.1.1-RFC.md?plain=1#L394) | [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L315) |

## §6.5

| 可验收行为 | 测试与问题 |
| --- | --- |
| [工具超时返回模型错误，长时间运行的命令本身不耗掉下一次 Provider 请求的时限](../docs/v1.1.1-RFC.md?plain=1#L410) | [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L262) · [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L230) |
| [读 stdin 或额外输入管道期间取消，写端仍打开也能返回 shell 可见的 130 或 143](../docs/v1.1.1-RFC.md?plain=1#L411) | [`test_cancelling_input_does_not_need_eof_or_dispatch_the_partial_message`](../tests/test_shell_pipeline.py#L264) |

## §7.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [独立 msh 超时后向整组发 TERM，3 秒后仍存活才强制回收并返回 124](../docs/v1.1.1-RFC.md?plain=1#L428) | [`test_msh_timeout_signals_the_whole_group_and_grants_three_seconds_before_kill`](../tests/test_shell_pipeline.py#L277) · [#53](https://github.com/Luthics/anix/issues/53) |

## §7.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [省略字段时工具发出无 summary 的 fold 帧，运行时生成摘要](../docs/v1.1.1-RFC.md?plain=1#L446) | [`aclr_without_a_summary_omits_the_field_at_each_valid_turn_id_boundary`](../crates/anix-tools/tests/rfc_control.rs#L58) · [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L870) · [#41](https://github.com/Luthics/anix/issues/41) |
| [按含 LF 的实际序列化长度逐个比较 aclr 发送端和内核接收端的判定](../docs/v1.1.1-RFC.md?plain=1#L433) | [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L72) · [#14](https://github.com/Luthics/anix/issues/14) |
| [四页文档的 15 种非空页集合，从文件、显式和隐式 stdin 输入，逐页核对 PNG 尺寸及色块](../docs/v1.1.1-RFC.md?plain=1#L439) | [`aview_selects_pdf_pages_with_their_actual_geometry_and_colors`](../crates/anix-tools/tests/rfc_control.rs#L236) |
| [页码语法错误时以非零状态退出且不发布附件](../docs/v1.1.1-RFC.md?plain=1#L439) | [`aview_rejects_invalid_pdf_page_expressions_without_publishing_attachments`](../crates/anix-tools/tests/rfc_control.rs#L305) |
| [重复或乱序页码的输出次序，以及合法页与越界页混用时能否先发布部分附件](../docs/v1.1.1-RFC.md?plain=1#L439) | [#54](https://github.com/Luthics/anix/issues/54) |

## §9.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [父子会话各自路由，子进程完成后父会话保持原标识](../docs/v1.1.1-RFC.md?plain=1#L576) | [`nested_anix_gets_its_own_id_and_the_parent_resumes_with_its_original_id`](../crates/anix-core/tests/rfc_runtime/session.rs#L224) |
| [每帧路由覆盖只在本轮生效，省略时回到环境默认](../docs/v1.1.1-RFC.md?plain=1#L588) | [`test_session_routing_works_in_oneshot_and_per_envelope_in_coprocess`](../tests/test_provider_dialects.py#L407) · [`test_coprocess_overrides_apply_to_one_frame_then_environment_defaults_resume`](../tests/test_provider_streaming.py#L102) |
| [显式空串和含首尾空白的路由标识如何处理](../docs/v1.1.1-RFC.md?plain=1#L593) | [#47](https://github.com/Luthics/anix/issues/47) |
| [三种方言的默认路由头、改名和逐请求禁用](../docs/v1.1.1-RFC.md?plain=1#L595) | [`test_header_rename_and_disable_apply_only_to_the_current_request`](../tests/test_provider_dialects.py#L433) · [`session_affinity_headers_and_body_mapping_across_all_dialects`](../crates/anix-providers/tests/e2e_mock.rs#L672) |
| [Responses 原生缓存键支持默认、字符串、布尔值及 null](../docs/v1.1.1-RFC.md?plain=1#L596) | [`test_responses_cache_key_has_native_string_boolean_and_null_semantics`](../tests/test_provider_dialects.py#L456) |
| [兼容 Chat 端点的两个 Body 路由字段分别按需启用](../docs/v1.1.1-RFC.md?plain=1#L597) | [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L474) |
| [Anthropic 只发送路由头，Body 保持协议格式](../docs/v1.1.1-RFC.md?plain=1#L598) | [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L474) |
| [慢速流入工具参数与已经开始执行的长命令位于不同的计时阶段](../docs/v1.1.1-RFC.md?plain=1#L624) | [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L291) · [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L230) · [#33](https://github.com/Luthics/anix/issues/33) |

## §9.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [模型从真实 XML 索引读取含特殊字符的技能路径，经 msh 按需获取完整正文](../docs/v1.1.1-RFC.md?plain=1#L643) | [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L165) |

## §9.5

| 可验收行为 | 测试与问题 |
| --- | --- |
| [采集端在第一条记录前或期间断开，Payload、stderr 和退出码保持一致](../docs/v1.1.1-RFC.md?plain=1#L650) | [`disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status`](../crates/anix-core/tests/rfc_runtime/trace.rs#L237) |
| [消费者保持连接但停止读取时的背压及恢复语义](../docs/v1.1.1-RFC.md?plain=1#L650) | [#11](https://github.com/Luthics/anix/issues/11) |
| [初始 Trace 与 WAL、Provider 环境和协进程信封可用同一标识关联](../docs/v1.1.1-RFC.md?plain=1#L657) | [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L195) |
| [模型只看清洗后文本；私有 wire 文件保留命令实际输出的 ANSI 字节](../docs/v1.1.1-RFC.md?plain=1#L667) | [`test_wire_trace_keeps_raw_msh_output_while_the_model_receives_cleaned_text`](../tests/test_provider_streaming.py#L198) · [#52](https://github.com/Luthics/anix/issues/52) |

## §9.6

| 可验收行为 | 测试与问题 |
| --- | --- |
| [MCP 指南按目录优先级披露，路径中的旧品牌待澄清](../docs/v1.1.1-RFC.md?plain=1#L675) | [`mcp_guide_follows_anix_xdg_and_project_precedence_without_disclosing_tool_schemas`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L376) · [#46](https://github.com/Luthics/anix/issues/46) |

## §10.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [全新 HOME 的安装布局、rc 文件、保留用户修改及安装后启动](../docs/v1.1.1-RFC.md?plain=1#L719) | [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L121) · [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L248) |
| [同时缺少 msh 且全局 TOML 损坏时先报告第 1 步](../docs/v1.1.1-RFC.md?plain=1#L721) | [`doctor_reports_missing_msh_before_a_malformed_global_config`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L154) · [#30](https://github.com/Luthics/anix/issues/30) |
| [逐个移除私有二进制或其执行权限，doctor 指出故障路径](../docs/v1.1.1-RFC.md?plain=1#L723) | [`each_missing_or_non_executable_private_binary_is_identified`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L31) |
| [免鉴权配置、密钥回退与项目级默认 Provider 的诊断](../docs/v1.1.1-RFC.md?plain=1#L724) | [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L70) · [`doctor_checks_the_selected_project_provider_and_its_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L122) |
| [doctor 发现无法创建匿名管道并给出修复提示](../docs/v1.1.1-RFC.md?plain=1#L725) | [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L164) |
| [doctor 检测锁操作失败或失去互斥性](../docs/v1.1.1-RFC.md?plain=1#L726) | [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L164) |
