# v1.2.0 RFC 逐测试索引

按测试查看它所验证的行为。编号链接到 RFC 原句，断言链接到执行比较的代码。同一条款有多个测试时，每行只描述该测试实际检查的部分。

反向查找见[条款验收表](RFC-requirements.md)。修改源码注释和验收表后，运行 `python3 scripts/rfc-test-index.py` 更新两个索引。

历史快照：[v1.1.2](https://github.com/Luthics/anix/blob/e4fb614f314625ae282bd2fcb9b7e80312514946/tests/RFC-cases.md)、[v1.1.1](https://github.com/Luthics/anix/blob/0b11a3c60218b9313f9e81a806c9a471906766eb/tests/RFC-cases.md)、[v1.0](https://github.com/Luthics/anix/blob/45e55a9f16f6bead6a933c67051b94b0acb753a0/tests/RFC-cases.md)。

## `crates/anix-core/src/cli.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`parses_script_and_args_with_double_dash`](../crates/anix-core/src/cli.rs#L220) | [§3.1-01](../docs/RFC.md?plain=1#L72) | -- 后的 -q 和 -n 保留为脚本参数，解释器只消费分隔符之前的选项。 |
| [`parses_direct_and_quoted_script_args_without_double_dash`](../crates/anix-core/src/cli.rs#L238) | [§3.1-01](../docs/RFC.md?plain=1#L72) | 省略 -- 时保留带空格的参数和空字符串，混排的 -y、-q、-c 仍由解释器处理。 |
| [`trust_subcommand_does_not_conflict_with_script_args`](../crates/anix-core/src/cli.rs#L261) | [§3.1-02](../docs/RFC.md?plain=1#L73) | trust 子命令与普通脚本参数分别解析。 |

## `crates/anix-core/src/platform/mod.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`probes_platform_capabilities`](../crates/anix-core/src/platform/mod.rs#L119) | [§10.2-03](../docs/RFC.md?plain=1#L752) | 检查 /dev/fd 可用；Linux 测试主机还须报告 pidfd 支持及 Landlock ABI ≥ 1。 |

## `crates/anix-core/src/wal.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage`](../crates/anix-core/src/wal.rs#L748) | [§6.2-16](../docs/RFC.md?plain=1#L393) | 回放丢弃未闭合工具调用，同时保留已经提交的 usage。 |
| [`replay_restores_session_id_across_checkpoint_and_derives_stable_legacy_fallback`](../crates/anix-core/src/wal.rs#L859) | [§6.2-03](../docs/RFC.md?plain=1#L369) | 从分节恢复已有 session_id，同一路径的旧 WAL 在重复读取时得到同一回退值。 |

## `crates/anix-core/tests/rfc_3_2/cli.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`crlf_script_reaches_the_provider_as_the_intended_system_message`](../crates/anix-core/tests/rfc_3_2/cli.rs#L44) | [§3.2-01](../docs/RFC.md?plain=1#L95) | CRLF 脚本的正文和 params 按预期进入真实 Provider 请求。 |
| [`unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position`](../crates/anix-core/tests/rfc_3_2/cli.rs#L67) | [§3.2-04](../docs/RFC.md?plain=1#L98) | 改变拼写或位置仍拒绝未声明的顶层键，且不调用 Provider。 |
| [`invalid_file_bytes_and_yaml_report_usage_errors_to_stderr`](../crates/anix-core/tests/rfc_3_2/cli.rs#L90) | [§6.5-05](../docs/RFC.md?plain=1#L423) | 脚本含非法 UTF-8 或无效 YAML 时以 2 退出并写 stderr。 |
| [`yaml_block_scalar_chomping_survives_the_frontmatter_boundary`](../crates/anix-core/tests/rfc_3_2/cli.rs#L107) | [§3.2-03](../docs/RFC.md?plain=1#L97) | 分别验证 \|、> 的 strip、clip 和 keep 尾换行。 |
| [`duplicate_yaml_keys_are_invalid_even_inside_open_maps`](../crates/anix-core/tests/rfc_3_2/cli.rs#L150) | [§3.2-03](../docs/RFC.md?plain=1#L97) | 在顶层字段与开放 map 中使用重复 YAML 键均应拒绝。 |
| [`yaml_aliases_can_supply_typed_provider_params_from_an_extension_field`](../crates/anix-core/tests/rfc_3_2/cli.rs#L163) | [§3.2-04](../docs/RFC.md?plain=1#L98) | x-* 中定义的 YAML 锚点可在 params 中复用并保留类型。 |
| [`vision_requires_a_yaml_boolean_and_leaves_provider_params_open`](../crates/anix-core/tests/rfc_3_2/cli.rs#L181) | [§3.3-13](../docs/RFC.md?plain=1#L126) | vision 只接受 YAML 布尔值，params.vision 仍可作为普通字符串透传。 |

## `crates/anix-core/tests/rfc_3_2/encoding.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`all_256_single_bytes_inside_a_plain_prompt`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L11) | [§3.2-01](../docs/RFC.md?plain=1#L95) | 遍历全部 256 个单字节，区分合法 ASCII 与非法 UTF-8。 |
| [`valid_multibyte_boundaries_survive_file_decoding`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L37) | [§3.2-01](../docs/RFC.md?plain=1#L95) | 合法多字节字符在 UTF-8 边界上逐字节保留。 |
| [`malformed_sequences_are_rejected_in_every_script_region`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L64) | [§3.2-01](../docs/RFC.md?plain=1#L95) | 在 shebang、头部及正文中分别插入畸形 UTF-8，解析须失败。 |
| [`eof_at_each_interior_byte_of_a_utf8_scalar`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L104) | [§3.2-01](../docs/RFC.md?plain=1#L95) | 多字节字符在任一内部位置遇 EOF 均应拒绝。 |

## `crates/anix-core/tests/rfc_3_2/grammar.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`layout_matrix_for_line_ending_shebang_header_body_and_eof`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L9) | [§3.2-01](../docs/RFC.md?plain=1#L95) | 组合 LF/CRLF、shebang、头部、正文与末尾 EOF，比较解析结果。 |
| [`only_a_first_line_beginning_with_hash_bang_is_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L46) | [§3.2-02](../docs/RFC.md?plain=1#L96) | 仅去掉首行 shebang。 |
| [`opening_delimiter_requires_the_designated_line_and_exact_marker`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L69) | [§3.2-03](../docs/RFC.md?plain=1#L97) | 仅指定位置的独占 --- 行能打开 YAML 头部。 |
| [`first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L92) | [§3.2-03](../docs/RFC.md?plain=1#L97) | 首个独占 --- 行关闭头部，只裁掉正文边界空行。 |
| [`yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L115) | [§3.2-03](../docs/RFC.md?plain=1#L97) | YAML 1.2 的标量类型与 x-* 扩展和开放 params 可共存。 |
| [`incomplete_delimiters_and_malformed_yaml_fail_at_the_header`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L145) | [§3.2-03](../docs/RFC.md?plain=1#L97) | 缺关闭分隔行或 YAML 损坏时在解析头部阶段失败。 |

## `crates/anix-core/tests/rfc_runtime/compaction.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L31) | [§9.2-10](../docs/RFC.md?plain=1#L654) | 逐一组合可选计数，核对 WAL、Trace、on-exit 与不含 usage 的 IR。 |
| [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L31) | [§9.2-11](../docs/RFC.md?plain=1#L655) | 枚举可选用量字段的每个子集，核对最终累计总量。 |
| [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L101) | [§6.1-03](../docs/RFC.md?plain=1#L336) | 对照显式报告 0 和未报告 usage 的水位与预算判定。 |
| [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L101) | [§9.2-12](../docs/RFC.md?plain=1#L656) | 显式零 usage 与未上报 usage 在预算恢复中产生可区分结果。 |
| [`retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L141) | [§9.2-13](../docs/RFC.md?plain=1#L657) | 失败重试已产生的费用各计一次，成功轮次水位不被抬高。 |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) | [§6.1-04](../docs/RFC.md?plain=1#L337) | 全量压缩后恢复会话，确认压缩器费用不抬高活跃窗口水位。 |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) | [§6.1-07](../docs/RFC.md?plain=1#L342) | 显式全量压缩后出现新分节，新进程从压缩上下文继续。 |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) | [§6.1-11](../docs/RFC.md?plain=1#L355) | 会话与内置压缩请求的实际使用量可分别核对。 |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) | [§9.2-10](../docs/RFC.md?plain=1#L654) | 全量压缩、恢复后的各类用量记录与实际各轮费用相符。 |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) | [§9.3-03](../docs/RFC.md?plain=1#L666) | on-exit 得到主运行及压缩器的累计费用，与 WAL/Trace 对齐。 |
| [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) | [§6.1-01](../docs/RFC.md?plain=1#L351) | 从同批工具的消息锚点折叠时回退到对应 assistant。 |
| [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) | [§6.1-08](../docs/RFC.md?plain=1#L343) | 锚点回退到闭合工具调用的 assistant，剪去触发折叠的工具轮次。 |
| [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) | [§6.2-11](../docs/RFC.md?plain=1#L367) | 局部 fold 后保留原始系统前缀。 |
| [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) | [§9.1-17](../docs/RFC.md?plain=1#L592) | 从工具锚点发起 fold，回退到对应 assistant 并插入 user 摘要。 |
| [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L385) | [§6.1-10](../docs/RFC.md?plain=1#L354) | 外部压缩器只收 IR、输出须为非空 UTF-8、故障退出。 |
| [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L385) | [§6.1-12](../docs/RFC.md?plain=1#L356) | 检查外部压缩器 stdin 中的 IR 工具列表为空。 |
| [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L385) | [§8.2-02](../docs/RFC.md?plain=1#L535) | 配置外部压缩器时从其 stdin/stdout 交付 IR 和摘要。 |
| [`fresh_window_counts_utf8_bytes_in_groups_of_three`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L454) | [§6.1-04](../docs/RFC.md?plain=1#L337) | 按 UTF-8 字节长度检查新窗口的三字节估算边界。 |
| [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L472) | [§3.3-06](../docs/RFC.md?plain=1#L130) | 等于有效预算时可调用 Provider，超过时在首次请求之前退出。 |
| [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L472) | [§6.1-06](../docs/RFC.md?plain=1#L339) | 水位等于上限时可推理，超过上限时首次 Provider 请求被阻止。 |
| [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L472) | [§6.5-07](../docs/RFC.md?plain=1#L425) | 首轮就超过硬上限时返回 124，Provider 未被调用。 |
| [`auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L502) | [§6.1-06](../docs/RFC.md?plain=1#L339) | 先压缩再按新窗口复查硬上限，仍超限时停止推理。 |
| [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L563) | [§6.1-11](../docs/RFC.md?plain=1#L355) | 内置压缩器读取配置的 Provider、模型及字面或文件提示词。 |
| [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L563) | [§6.1-12](../docs/RFC.md?plain=1#L356) | 检查内置压缩器发给 Provider 的请求无工具。 |
| [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L563) | [§8.2-02](../docs/RFC.md?plain=1#L535) | 内置压缩器根据配置选择 Provider、模型与提示词。 |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) | [§6.1-07](../docs/RFC.md?plain=1#L342) | 连续两个 checkpoint 只累计一次旧用量，并在新分节刷新系统前缀。 |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) | [§6.2-11](../docs/RFC.md?plain=1#L367) | 完整压缩重新运行钩子并刷新系统前缀。 |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) | [§6.2-12](../docs/RFC.md?plain=1#L368) | 跨两次 checkpoint 恢复会话时逐项核对累计用量。 |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) | [§6.2-15](../docs/RFC.md?plain=1#L392) | 跨两个 checkpoint 核对被剪枝轮次费用只累加一次。 |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) | [§6.2-17](../docs/RFC.md?plain=1#L394) | 回放到最新 checkpoint 后核对跨分节累计用量。 |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) | [§9.1-16](../docs/RFC.md?plain=1#L591) | 两次完整压缩后的 IR 都以新冻结 system 与 user 摘要开头。 |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) | [§9.2-13](../docs/RFC.md?plain=1#L657) | 跨两个 checkpoint 的旧用量不重复累加。 |
| [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L894) | [§6.1-14](../docs/RFC.md?plain=1#L346) | 外部自动摘要器收到对齐的无工具子窗口，恢复后的 IR 只保留返回的摘要。 |
| [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L894) | [§7.2-01](../docs/RFC.md?plain=1#L474) | 运行时接收无摘要 fold 帧，调用摘要器并将其结果用于折叠。 |
| [`empty_or_failed_auto_summary_keeps_the_subwindow_and_reports_a_tool_error`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L924) | [§6.1-15](../docs/RFC.md?plain=1#L348) | 自动摘要失败或为空时保留旧窗口，向模型返回工具错误。 |

## `crates/anix-core/tests/rfc_runtime/configuration.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`agent_name_has_exactly_the_documented_ascii_alphabet`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L11) | [§3.3-02](../docs/RFC.md?plain=1#L123) | 遍历允许与禁止的字符，检查显式 name 的 ASCII 字母、数字及 . _ - 边界。 |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) | [§3.3-03](../docs/RFC.md?plain=1#L124) | 错误类型的 provider 字段在解析阶段拒绝。 |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) | [§3.3-04](../docs/RFC.md?plain=1#L125) | model 输入必须满足字符串约束。 |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) | [§3.3-06](../docs/RFC.md?plain=1#L130) | 验证 max_tokens 的整数类型、下限及 uint32 上界。 |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) | [§3.3-09](../docs/RFC.md?plain=1#L133) | 未知 approval 枚举值不能解析为有效 Frontmatter。 |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) | [§3.3-10](../docs/RFC.md?plain=1#L134) | 拒绝 stderr 的未知枚举值或错误类型。 |
| [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L96) | [§3.1-07](../docs/RFC.md?plain=1#L85) | 按比例取整或使用绝对阈值，0 与达到上限的配置关闭自动压缩。 |
| [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L96) | [§3.3-08](../docs/RFC.md?plain=1#L132) | 小数阈值向下取整，整数表示绝对值，0 或超过上下文时禁用。 |
| [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L122) | [§3.3-04](../docs/RFC.md?plain=1#L125) | 在五层配置的 32 种子集中检查最高优先级的 model。 |
| [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L122) | [§3.4-02](../docs/RFC.md?plain=1#L141) | 枚举五层配置开关的 32 种子集，读取 Provider 接到的模型与输出上限。 |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) | [§3.1-06](../docs/RFC.md?plain=1#L82) | 逐一启用五层覆盖的组合，检查最后生效的 Provider 与 shell。 |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) | [§3.1-07](../docs/RFC.md?plain=1#L85) | 从五层值的组合读取生效的轮数、Token 预算和压缩配置。 |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) | [§3.1-11](../docs/RFC.md?plain=1#L91) | 在配置层组合中读取解析后的 idle_timeout 值。 |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) | [§3.3-09](../docs/RFC.md?plain=1#L133) | 逐层指定 approval，检查解析出的有效策略及无覆盖的默认值。 |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) | [§3.3-10](../docs/RFC.md?plain=1#L134) | 逐层指定 stderr，检查生效模式及无覆盖的默认值。 |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) | [§3.4-02](../docs/RFC.md?plain=1#L141) | 对全部 32 种配置组合，核对可观测的标量和布尔选项。 |
| [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L364) | [§3.1-07](../docs/RFC.md?plain=1#L85) | 单轮输出等于上下文预算时拒绝，0 不进入 IR。 |
| [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L364) | [§3.3-07](../docs/RFC.md?plain=1#L131) | 0 不写入 header，非零上限必须严格小于有效上下文预算。 |
| [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L364) | [§9.1-06](../docs/RFC.md?plain=1#L578) | 单轮限制为 0 时 header 省略该字段，合法非零值按原值写入。 |
| [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L404) | [§3.3-12](../docs/RFC.md?plain=1#L136) | params 的扩展键与浅合并。 |
| [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L404) | [§3.4-07](../docs/RFC.md?plain=1#L159) | CLI 中的 JSON 参数按键覆盖，未冲突的键及值类型保留。 |
| [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L451) | [§3.1-05](../docs/RFC.md?plain=1#L81) | ANIX_CONTEXT 为空串时即使脚本声明 context 也不创建 WAL。 |
| [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L451) | [§3.3-11](../docs/RFC.md?plain=1#L135) | 环境将 context 显式置空时关闭文件持久化。 |
| [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L451) | [§3.4-04](../docs/RFC.md?plain=1#L148) | 显式空串覆盖会话路径。 |
| [`project_search_stops_at_nearest_configuration_git_or_home_boundary`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L485) | [§8.1-04](../docs/RFC.md?plain=1#L490) | 按最近配置、Git 仓库及 HOME 边界分别检查向上查找结果。 |
| [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L538) | [§3.3-03](../docs/RFC.md?plain=1#L124) | 模拟已注册的 Provider 读取所选配置及注入值。 |
| [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L538) | [§9.1-01](../docs/RFC.md?plain=1#L572) | oneshot Provider 从进程环境读取所选模型。 |
| [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L538) | [§9.2-09](../docs/RFC.md?plain=1#L612) | 有效 model、base URL、timeout 与密钥来源。 |
| [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L595) | [§3.1-08](../docs/RFC.md?plain=1#L86) | 对照 ANIX_YES、ANIX_APPROVAL 与 CLI，检查命令的磁盘副作用及拒绝结果。 |
| [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L595) | [§3.4-03](../docs/RFC.md?plain=1#L147) | 识别 ANIX_YES 的 1 与 true，并检查与 CLI 审批覆盖的组合。 |
| [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L595) | [§5.1-01](../docs/RFC.md?plain=1#L243) | 用环境别名与 CLI 切换审批策略，检查是否产生预期的命令副作用。 |
| [`invalid_resolved_environment_values_exit_two_before_contacting_the_provider`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L643) | [§3.4-03](../docs/RFC.md?plain=1#L147) | 非法环境配置以 2 退出且不启动 Provider。 |
| [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L671) | [§6.5-05](../docs/RFC.md?plain=1#L423) | 全局或项目 TOML 出现未知键时以 2 退出且未推理。 |
| [`absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L718) | [§3.3-08](../docs/RFC.md?plain=1#L132) | 在 uint32 最大值两侧探测绝对阈值，溢出应先被拒绝。 |
| [`cli_params_preserve_json_literals_and_the_exact_fallback_string`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L752) | [§3.1-09](../docs/RFC.md?plain=1#L87) | --param 的 JSON 转为对应类型，无法解析时保留原字符串。 |
| [`malformed_cli_param_stops_before_provider_or_wal_creation`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L789) | [§3.1-09](../docs/RFC.md?plain=1#L87) | 缺少有效 KEY=VALUE 时使用错误码 2，WAL 与 Provider 请求均不存在。 |
| [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) | [§3.1-05](../docs/RFC.md?plain=1#L81) | 不同工作目录下运行同一脚本，使用各自的相对 WAL 路径。 |
| [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) | [§3.3-11](../docs/RFC.md?plain=1#L135) | 同一脚本从两个 cwd 启动时，各自生成对应的相对 WAL。 |
| [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) | [§8.1-04](../docs/RFC.md?plain=1#L490) | 从两个 cwd 调用同一脚本时使用各自项目配置。 |
| [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) | [§9.1-08](../docs/RFC.md?plain=1#L580) | 从不同 cwd 启动时，实际 Provider header.meta.cwd 记录各自目录。 |
| [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L884) | [§3.3-12](../docs/RFC.md?plain=1#L136) | params 的扩展键与浅合并。 |
| [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L884) | [§3.4-07](../docs/RFC.md?plain=1#L159) | 只合并被选中 Provider 的 params，同名对象整体替换。 |
| [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L884) | [§8.2-01](../docs/RFC.md?plain=1#L532) | 所选 Provider 的 params 浅合并。 |
| [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L963) | [§2-02](../docs/RFC.md?plain=1#L62) | Provider 的命令名依次查找 PATH 和私有目录，显式路径直接执行。 |
| [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L963) | [§3.1-06](../docs/RFC.md?plain=1#L82) | 配置为名称时按 PATH、libexec 查找 Provider，带斜杠时直接使用该路径。 |
| [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1031) | [§8.1-01](../docs/RFC.md?plain=1#L487) | XDG_CONFIG_HOME 与 HOME 回退。 |
| [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1031) | [§8.1-02](../docs/RFC.md?plain=1#L488) | XDG_STATE_HOME 与 HOME 回退。 |
| [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1031) | [§8.1-03](../docs/RFC.md?plain=1#L489) | 分别设置 XDG 变量，检查 runtime 目录及其他目录独立回退。 |
| [`selected_model_changes_the_provider_environment_without_entering_the_ir`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1090) | [§9.1-01](../docs/RFC.md?plain=1#L572) | 更换模型时只改变 Provider 环境，Canonical IR 不含 model。 |

## `crates/anix-core/tests/rfc_runtime/control.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L28) | [§4.3-04](../docs/RFC.md?plain=1#L218) | 同次及分次写入控制管道的多条媒体帧按顺序绑定到对应工具。 |
| [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L28) | [§7.2-07](../docs/RFC.md?plain=1#L461) | 并列及跨写入的媒体帧均应按完整 JSONL 及到达顺序处理。 |
| [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L28) | [§9.1-13](../docs/RFC.md?plain=1#L586) | 媒体帧保留工具调用身份和到达顺序，恢复时附件保持一致。 |
| [`two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order`](../crates/anix-core/tests/rfc_runtime/control.rs#L97) | [§7.2-07](../docs/RFC.md?plain=1#L461) | 同一工具提交两条合法 fold 帧，WAL 应按到达顺序各写一条。 |

## `crates/anix-core/tests/rfc_runtime/doctor.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`each_missing_or_non_executable_private_binary_is_identified`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L31) | [§10.3-03](../docs/RFC.md?plain=1#L760) | 逐个移除私有二进制或其执行权限，doctor 指出故障路径。 |
| [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L71) | [§3.1-03](../docs/RFC.md?plain=1#L74) | 自检与凭据诊断。 |
| [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L71) | [§10.3-04](../docs/RFC.md?plain=1#L761) | 对比密钥缺失、空值及回退来源的 doctor 诊断。 |
| [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L100) | [§8.1-03](../docs/RFC.md?plain=1#L489) | runtime 目录不可用时 doctor 报错。 |
| [`doctor_checks_the_selected_project_provider_and_its_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L123) | [§10.3-04](../docs/RFC.md?plain=1#L761) | doctor 检查项目实际选中的默认 Provider 及其凭据。 |
| [`doctor_reports_missing_msh_before_a_malformed_global_config`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L155) | [§10.3-02](../docs/RFC.md?plain=1#L758) | 同时缺少 msh 且全局 TOML 损坏时先报告第 1 步。 |

## `crates/anix-core/tests/rfc_runtime/fd.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment`](../crates/anix-core/tests/rfc_runtime/fd.rs#L61) | [§4.1-09](../docs/RFC.md?plain=1#L174) | 有效 CLI trace FD 覆盖环境设定，只有被选中的 FD 收到 JSONL。 |
| [`each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input`](../crates/anix-core/tests/rfc_runtime/fd.rs#L94) | [§4.1-08](../docs/RFC.md?plain=1#L175) | 可读写的显式 Trace FD 不进入静态输入。 |
| [`invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider`](../crates/anix-core/tests/rfc_runtime/fd.rs#L136) | [§4.1-09](../docs/RFC.md?plain=1#L174) | 传入不可写或未打开的 trace FD 时返回 2，Provider 未被调用。 |

## `crates/anix-core/tests/rfc_runtime/hooks.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L11) | [§9.1-09](../docs/RFC.md?plain=1#L581) | pre-context 输出中的改写及追加记录可进入被冻结的上下文。 |
| [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L11) | [§9.3-01](../docs/RFC.md?plain=1#L664) | 串联 pre-context 过滤器后只冻结最终改写的消息序列。 |
| [`pre_context_failure_forwards_stderr_and_stops_before_inference`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L82) | [§9.3-01](../docs/RFC.md?plain=1#L664) | pre-context 失败时 stderr 可见，后续推理不启动。 |
| [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L108) | [§9.3-02](../docs/RFC.md?plain=1#L665) | 首个失败的 post-tool 阻止后续过滤器、工具和推理。 |
| [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L158) | [§6.3-02](../docs/RFC.md?plain=1#L403) | 一个 on-exit 钩子超时后其他钩子仍执行，保留主进程退出码。 |
| [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L158) | [§9.3-03](../docs/RFC.md?plain=1#L666) | 一个 on-exit 钩子超时只影响该钩子，主退出码保持不变。 |
| [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L158) | [§10.2-02](../docs/RFC.md?plain=1#L751) | 在无 pidfd 的 Linux 配置里，退出钩子仍有独立超时。 |
| [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L199) | [§9.4-03](../docs/RFC.md?plain=1#L672) | 六层搜索顺序和同名覆盖。 |
| [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L199) | [§9.4-04](../docs/RFC.md?plain=1#L673) | 逐层比较同名技能的获胜路径，首轮索引中不出现正文。 |
| [`tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L262) | [§9.6-02](../docs/RFC.md?plain=1#L712) | 版本漂移后恢复内置指南；状态目录只读时使用运行时目录。 |
| [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L302) | [§9.6-02](../docs/RFC.md?plain=1#L712) | 屏蔽 30-tips 时首次运行不会释放内置指南文件。 |
| [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L302) | [§9.6-03](../docs/RFC.md?plain=1#L727) | 空文件或 /dev/null 屏蔽 30-tips 后索引和磁盘副作用均消失。 |
| [`an_empty_merged_tips_index_costs_no_prompt_bytes`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L350) | [§9.6-04](../docs/RFC.md?plain=1#L715) | 屏蔽所有指南后不向系统提示词注入 tips 索引。 |
| [`mcp_guide_follows_anix_xdg_and_project_precedence_without_disclosing_tool_schemas`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L381) | [§9.6-01](../docs/RFC.md?plain=1#L712) | MCP 指南按 anix 状态目录、全局和项目路径披露。 |
| [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L474) | [§3.1-10](../docs/RFC.md?plain=1#L88) | 启动与恢复时对照 stderr=auto/explicit 的提示词、输出与 WAL 通知。 |
| [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L474) | [§4.4-02](../docs/RFC.md?plain=1#L228) | 首次生成 stderr=auto 的运行提示，诊断流可见模型文本。 |
| [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L474) | [§4.4-03](../docs/RFC.md?plain=1#L228) | 在旧 WAL 上切换至 explicit，只在末尾追加通知并保留冻结首条提示。 |
| [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) | [§4.4-01](../docs/RFC.md?plain=1#L224) | 同名全局或已信任项目钩子可覆写和屏蔽。 |
| [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) | [§8.3-01](../docs/RFC.md?plain=1#L548) | 三个内置 pre-context 钩子依次由全局及项目同名文件覆写，也可被空文件或空设备软链屏蔽。 |
| [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) | [§9.4-02](../docs/RFC.md?plain=1#L670) | 20-skills 内置、同名覆写与禁用。 |
| [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) | [§9.6-03](../docs/RFC.md?plain=1#L727) | 用全局或已信任项目中的同名钩子覆写或屏蔽 30-tips。 |
| [`post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L602) | [§6.1-13](../docs/RFC.md?plain=1#L333) | 工具退出 0、7、124 时，当前 IR、落盘 WAL 和重新加载的 IR 都含正确的轮次、退出码与过滤后文本。 |
| [`post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L602) | [§9.3-02](../docs/RFC.md?plain=1#L665) | 两个 post-tool 过滤器依次改写文本，只有最终结果到达 WAL 和 Provider。 |

## `crates/anix-core/tests/rfc_runtime/inbox.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L21) | [§4.2-01](../docs/RFC.md?plain=1#L200) | 当前工具批次闭合后才取下一条已发布消息。 |
| [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L21) | [§4.2-04](../docs/RFC.md?plain=1#L197) | 并发发布时核对 Inbox 目录权限，并确认每个消息体完整交付。 |
| [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L21) | [§4.2-05](../docs/RFC.md?plain=1#L198) | 多发送方只发布完整文件。 |
| [`a_publisher_can_queue_work_while_the_initial_cli_task_is_running`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L122) | [§4.2-04](../docs/RFC.md?plain=1#L197) | 在初始 CLI 任务仍运行时即可向已就绪的队列发布下一任务。 |
| [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L182) | [§3.1-11](../docs/RFC.md?plain=1#L91) | Provider 仍忙时不计闲置时间，最后一个任务完成后才退出并释放 WAL。 |
| [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L182) | [§4.2-06](../docs/RFC.md?plain=1#L206) | 上一任务结束后才开始计算闲置超时并释放 WAL 锁。 |
| [`oneshot_nonzero_after_done_discards_uncommitted_calls_and_preserves_later_jobs`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L268) | [§9.2-16](../docs/RFC.md?plain=1#L638) | oneshot 虽已输出 done 却非零退出时返回 1，不执行未提交的命令，后一条 Inbox 消息保留在 new/。 |
| [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L340) | [§4.2-03](../docs/RFC.md?plain=1#L205) | Linux 空闲时只发一次 idle，新任务结束后重新待命。 |
| [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L340) | [§6.3-01](../docs/RFC.md?plain=1#L399) | 无截止时间的 Linux 空闲循环持续睡眠，发布消息可以唤醒。 |

## `crates/anix-core/tests/rfc_runtime/io.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/anix-core/tests/rfc_runtime/io.rs#L8) | [§3.1-04](../docs/RFC.md?plain=1#L75) | --version 成功退出，在 stdout 报告包版本与 ir_version。 |
| [`args_stdin_and_sparse_extra_descriptors_are_ordered_and_trace_is_separate`](../crates/anix-core/tests/rfc_runtime/io.rs#L35) | [§4.1-13](../docs/RFC.md?plain=1#L175) | 先拼 args 与 stdin，再按 4、6、9 的顺序拼入可读 FD，跳过只写 FD 和 Trace。 |
| [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) | [§3.1-10](../docs/RFC.md?plain=1#L88) | 两种 stderr 模式都不改变重定向到文件的工具原始字节。 |
| [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) | [§4.1-04](../docs/RFC.md?plain=1#L172) | 在两个工具调用中写出含 NUL 和 0xff 的片段，目标文件保留顺序和原始字节。 |
| [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) | [§4.3-05](../docs/RFC.md?plain=1#L220) | 连续工具重定向到普通文件时顺序追加。 |
| [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) | [§6.5-03](../docs/RFC.md?plain=1#L421) | 正常结束返回 0，普通文件中保留完整的显式输出。 |
| [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L154) | [§2-03](../docs/RFC.md?plain=1#L63) | 检查工具进程 PATH 的第一项目录为私有 tools。 |
| [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L154) | [§3.4-06](../docs/RFC.md?plain=1#L157) | 在 output=explicit 的工具进程中读到 ANIX_OUT=/dev/fd/4、ANIX_ERR=/dev/fd/5、ANIX_CTL=/dev/fd/6。 |
| [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L154) | [§7.1-02](../docs/RFC.md?plain=1#L436) | 实际子 shell 所见 PATH 起始项为私有工具目录。 |
| [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/anix-core/tests/rfc_runtime/io.rs#L200) | [§3.1-01](../docs/RFC.md?plain=1#L72) | -- 分隔后的类选项字符串按原顺序送入 Provider；普通参数允许省略分隔符。 |
| [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L222) | [§4.1-13](../docs/RFC.md?plain=1#L175) | 逐个设置 fd 4–9 的访问模式，核对输入顺序及调用子进程前关闭文件。 |
| [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L222) | [§4.1-14](../docs/RFC.md?plain=1#L185) | Provider、钩子、shell 不能继承已消费的文件。 |
| [`shell_environment_contains_only_observability_effective_approval_and_routed_fds`](../crates/anix-core/tests/rfc_runtime/io.rs#L295) | [§3.4-05](../docs/RFC.md?plain=1#L154) | 清除父实例状态、保留观测变量与普通环境变量。 |
| [`unterminated_deltas_are_separated_from_tool_status_diagnostics_and_usage`](../crates/anix-core/tests/rfc_runtime/io.rs#L407) | [§4.1-06](../docs/RFC.md?plain=1#L181) | 流式正文和思考帧在工具状态、诊断侧信道及使用量汇总处的换行，显式模式只输出诊断。 |

## `crates/anix-core/tests/rfc_runtime/provider.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr`](../crates/anix-core/tests/rfc_runtime/provider.rs#L20) | [§9.2-15](../docs/RFC.md?plain=1#L659) | 同一请求至多重试三次，按 1/2/4 秒退避并保留最终 stderr。 |
| [`nonretryable_error_preserves_provider_stderr_without_another_attempt`](../crates/anix-core/tests/rfc_runtime/provider.rs#L136) | [§9.2-15](../docs/RFC.md?plain=1#L659) | 永久错误只调用一次 Provider，退出时向 stderr 转发其诊断。 |
| [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/anix-core/tests/rfc_runtime/provider.rs#L147) | [§6.5-04](../docs/RFC.md?plain=1#L422) | Provider 未发终结帧就 EOF，主进程退出 1 并报告诊断。 |
| [`provider_timeout_maps_to_124_without_a_retry`](../crates/anix-core/tests/rfc_runtime/provider.rs#L158) | [§6.5-07](../docs/RFC.md?plain=1#L425) | Provider 请求超时后返回 124，且不重试。 |
| [`opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch`](../crates/anix-core/tests/rfc_runtime/provider.rs#L164) | [§9.2-14](../docs/RFC.md?plain=1#L658) | 同一 Provider 恢复时回放 opaque，切换 Provider 后应去掉私有字段。 |
| [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L235) | [§3.3-01](../docs/RFC.md?plain=1#L129) | 最后一轮完成返回 0，仍需推理时返回 124，均不再请求 Provider。 |
| [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L235) | [§6.5-07](../docs/RFC.md?plain=1#L425) | 最后一轮已用尽但仍需推理时返回 124。 |

## `crates/anix-core/tests/rfc_runtime/session.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`stateless_runs_get_distinct_uuid_v4s_while_all_provider_calls_in_a_run_share_one`](../crates/anix-core/tests/rfc_runtime/session.rs#L100) | [§6.2-02](../docs/RFC.md?plain=1#L369) | 无状态进程内普通推理和压缩共用 UUID v4，不同进程标识不同。 |
| [`persisted_id_survives_builtin_compaction_checkpoint_alias_and_subsequent_resume`](../crates/anix-core/tests/rfc_runtime/session.rs#L122) | [§6.2-01](../docs/RFC.md?plain=1#L369) | 新建会话在压缩、路径别名与恢复后仍使用原 session_id。 |
| [`legacy_wal_uses_a_stable_canonical_path_id_across_aliases_and_restarts`](../crates/anix-core/tests/rfc_runtime/session.rs#L161) | [§6.2-03](../docs/RFC.md?plain=1#L369) | 旧 WAL 的规范路径及其别名在重启时得到相同回退标识。 |
| [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L196) | [§6.2-04](../docs/RFC.md?plain=1#L394) | 最新 checkpoint 提供的标识原样到达 Trace、环境和协进程信封。 |
| [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L196) | [§9.5-03](../docs/RFC.md?plain=1#L693) | 初始 Trace 与 WAL、Provider 环境和协进程信封可用同一标识关联。 |
| [`nested_anix_gets_its_own_id_and_the_parent_resumes_with_its_original_id`](../crates/anix-core/tests/rfc_runtime/session.rs#L225) | [§9.2-01](../docs/RFC.md?plain=1#L612) | 父子会话各自路由，子进程完成后父会话保持原标识。 |
| [`inbox_keeps_one_session_id_for_both_messages_in_a_single_coprocess`](../crates/anix-core/tests/rfc_runtime/session.rs#L277) | [§6.2-04](../docs/RFC.md?plain=1#L394) | 两条 Inbox 消息使用同一 Provider 进程，两个请求的 session_id 与 WAL 及 run_start 相同。 |
| [`first_checkpoint_in_a_legacy_wal_persists_the_fallback_id_even_if_the_wal_moves`](../crates/anix-core/tests/rfc_runtime/session.rs#L314) | [§6.2-03](../docs/RFC.md?plain=1#L369) | 第一次 checkpoint 持久化回退标识，移动 WAL 后仍可恢复。 |
| [`empty_context_overrides_a_saved_wal_and_keeps_one_ephemeral_id_through_compaction`](../crates/anix-core/tests/rfc_runtime/session.rs#L350) | [§6.2-02](../docs/RFC.md?plain=1#L369) | 清空 context 后不会沿用已有 WAL 标识，当次压缩仍复用本次标识。 |
| [`restarting_a_coprocess_between_turns_preserves_session_and_tool_history`](../crates/anix-core/tests/rfc_runtime/session.rs#L387) | [§9.2-17](../docs/RFC.md?plain=1#L624) | 协进程重新启动后，替代进程仍收到相同 session_id 和已提交的工具历史。 |

## `crates/anix-core/tests/rfc_runtime/signals.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/anix-core/tests/rfc_runtime/signals.rs#L12) | [§6.4-01](../docs/RFC.md?plain=1#L409) | 中断后收回 Provider、不提交半截回答，下一进程可以重新获取 WAL 锁。 |
| [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L51) | [§6.4-05](../docs/RFC.md?plain=1#L411) | 保持模式进程收到 STOP 后暂停，CONT 恢复后继续响应快照请求。 |
| [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L51) | [§6.4-07](../docs/RFC.md?plain=1#L413) | USR2 输出快照路径，文件权限为 0600，内容与活跃 IR 相符。 |
| [`usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference`](../crates/anix-core/tests/rfc_runtime/signals.rs#L118) | [§6.4-06](../docs/RFC.md?plain=1#L412) | USR1 在下次推理前执行一次无工具的上下文压缩。 |
| [`sigterm_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L171) | [§10.2-02](../docs/RFC.md?plain=1#L751) | 在无 pidfd 的 Linux 配置里，SIGTERM 的子进程清理仍须按时结束。 |
| [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L178) | [§6.3-02](../docs/RFC.md?plain=1#L403) | 向响应 TERM 的 Provider 发 SIGINT，检查回收仍受截止时间限制。 |
| [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L178) | [§10.2-02](../docs/RFC.md?plain=1#L751) | 在无 pidfd 的 Linux 配置里，SIGINT 也必须按期限回收已收到 TERM 的 Provider。 |
| [`abrupt_parent_exit_stops_the_active_provider_or_shell_child`](../crates/anix-core/tests/rfc_runtime/signals.rs#L222) | [§10.2-01](../docs/RFC.md?plain=1#L751) | Linux 上主进程异常终止后，活跃的 Provider 或 shell 子进程也终止。 |

## `crates/anix-core/tests/rfc_runtime/trace.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`events_distinguish_tool_approval_from_execution_and_record_an_applied_fold`](../crates/anix-core/tests/rfc_runtime/trace.rs#L16) | [§9.5-09](../docs/RFC.md?plain=1#L692) | 审批和实际执行分别发出事件，已应用的 fold 也写入 Trace。 |
| [`explicit_fd_separates_events_deltas_and_wire_and_omits_provider_key`](../crates/anix-core/tests/rfc_runtime/trace.rs#L147) | [§9.5-10](../docs/RFC.md?plain=1#L691) | 对独立 FD 比较 events、deltas、wire 记录内容，三种级别均排除 Provider 密钥。 |
| [`journal_without_an_explicit_fd_receives_lifecycle_events_and_no_wire_records`](../crates/anix-core/tests/rfc_runtime/trace.rs#L201) | [§9.5-11](../docs/RFC.md?plain=1#L704) | 未指定显式 FD 时 journal 收到生命周期事件，wire 内容不会发往共享 socket。 |
| [`disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status`](../crates/anix-core/tests/rfc_runtime/trace.rs#L237) | [§9.5-01](../docs/RFC.md?plain=1#L686) | 采集端在第一条记录前或期间断开，Payload、stderr 和退出码保持一致。 |
| [`explicit_trace_fd_takes_precedence_over_an_available_journal`](../crates/anix-core/tests/rfc_runtime/trace.rs#L289) | [§9.5-12](../docs/RFC.md?plain=1#L685) | 同时存在显式 Trace FD 和 journal 时，记录写入指定 FD。 |

## `crates/anix-core/tests/rfc_runtime/vision.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`vision_resolves_from_cli_env_script_selected_provider_then_defaults`](../crates/anix-core/tests/rfc_runtime/vision.rs#L10) | [§5.5-01](../docs/RFC.md?plain=1#L299) | 依次覆盖 CLI、环境、脚本、所选 Provider、全局默认和内置默认。 |
| [`native_vision_keeps_the_attachment_through_resume_and_never_calls_a_bridge`](../crates/anix-core/tests/rfc_runtime/vision.rs#L163) | [§9.1-21](../docs/RFC.md?plain=1#L586) | 启用原生视觉时附件随 tool 消息写入 WAL，恢复后保留附件且不调用桥接器。 |
| [`no_native_vision_without_a_bridge_returns_the_documented_error_through_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L181) | [§5.5-02](../docs/RFC.md?plain=1#L304) | 无原生视觉且无桥接器时，工具报错且附件不进入恢复的历史。 |
| [`bridge_gets_one_utf8_json_line_then_eof_and_its_description_survives_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L191) | [§5.5-03](../docs/RFC.md?plain=1#L313) | 桥接器接收图片信息和 EOF，恢复会话后保留转译描述。 |
| [`bridge_nonzero_exit_and_timeout_both_return_stderr_as_a_recoverable_tool_error`](../crates/anix-core/tests/rfc_runtime/vision.rs#L218) | [§5.5-05](../docs/RFC.md?plain=1#L319) | 视觉桥接非零退出或超时成为可恢复的工具错误。 |

## `crates/anix-core/tests/rfc_runtime/wal.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`legacy_wal_without_vision_or_fold_usage_resumes_without_rewriting_the_prefix`](../crates/anix-core/tests/rfc_runtime/wal.rs#L26) | [§6.2-10](../docs/RFC.md?plain=1#L365) | 恢复缺 vision 和 fold_usage 的旧 WAL，已有前缀不被改写。 |
| [`repeated_resume_appends_one_vision_update_per_change_and_reuses_frozen_text`](../crates/anix-core/tests/rfc_runtime/wal.rs#L62) | [§6.2-05](../docs/RFC.md?plain=1#L366) | 两次相同 vision 配置不会重复通知；每次切换仅追加一次并复用冻结提示词。 |

## `crates/anix-core/tests/rfc_runtime/wal_replay.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`every_partial_byte_write_of_a_final_record_fails_without_modifying_history`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L35) | [§6.2-13](../docs/RFC.md?plain=1#L389) | 对最后一条记录的每个内部字节截断，失败时原 WAL 不变。 |
| [`resuming_a_complete_last_object_without_lf_never_joins_two_wal_records`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L59) | [§6.2-13](../docs/RFC.md?plain=1#L389) | 最后记录 JSON 完整但缺 LF 时，恢复追加不能粘连两个 JSON 对象。 |
| [`checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L109) | [§6.2-08](../docs/RFC.md?plain=1#L388) | 构造倒读缓冲区边界上的 checkpoint，检查回放不会触碰更早的损坏记录。 |
| [`overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L137) | [§6.2-14](../docs/RFC.md?plain=1#L391) | 重叠 fold 保留存活前缀，剪枝后也不回收已用 turn_id。 |
| [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L174) | [§6.2-17](../docs/RFC.md?plain=1#L394) | 末尾损坏或分节版本不受支持时，不进入正常推理。 |
| [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L174) | [§6.5-05](../docs/RFC.md?plain=1#L423) | 不兼容的 WAL 分节版本在推理前以 2 退出。 |
| [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L198) | [§6.2-09](../docs/RFC.md?plain=1#L362) | 独占非阻塞锁、冲突时退出并诊断。 |
| [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L198) | [§6.5-04](../docs/RFC.md?plain=1#L422) | 第二进程争用 WAL 的非阻塞锁时以 1 退出。 |
| [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L218) | [§6.2-10](../docs/RFC.md?plain=1#L365) | 恢复时复用首次冻结的 system，仅在有配置变更时追加通知。 |
| [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L218) | [§9.1-18](../docs/RFC.md?plain=1#L593) | 检测到配置漂移时保留首次冻结 system，尾部追加 runtime_update。 |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) | [§6.1-02](../docs/RFC.md?plain=1#L333) | 连续三次折叠后将每个存活消息锚点与独立回放结果比较。 |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) | [§6.1-08](../docs/RFC.md?plain=1#L343) | 对每个存活锚点执行折叠，与独立正向回放的前缀及摘要比较。 |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) | [§6.2-14](../docs/RFC.md?plain=1#L391) | 对连续折叠后的所有有效锚点，用正向模型核对活跃消息与水位。 |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) | [§6.2-15](../docs/RFC.md?plain=1#L392) | 每轮剪枝后将会话累计用量与正向模型比较。 |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) | [§6.2-18](../docs/RFC.md?plain=1#L395) | 正序回放与前向参考模型逐轮对照。 |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) | [§9.1-17](../docs/RFC.md?plain=1#L592) | 把不同存活锚点的折叠结果与独立模型比较，检查摘要位置。 |
| [`running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L404) | [§6.1-02](../docs/RFC.md?plain=1#L333) | uint64 标识耗尽时检查已经提交的锚点不会被复用。 |
| [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L447) | [§6.2-11](../docs/RFC.md?plain=1#L367) | 从不完整的 checkpoint 恢复时不得交付缺失冻结上下文的请求。 |
| [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L447) | [§9.1-16](../docs/RFC.md?plain=1#L591) | 逐条截断 checkpoint，检查只在已具备完整前缀时才派发。 |
| [`initial_section_interruption_never_dispatches_without_a_frozen_system_message`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L524) | [§6.2-10](../docs/RFC.md?plain=1#L365) | 在首次分节各记录边界模拟中断，检查未冻结 system 时不向 Provider 派发。 |
| [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L579) | [§6.1-03](../docs/RFC.md?plain=1#L336) | 恢复时使用最后存活 assistant 的 usage 对下一请求执行硬上限判断。 |
| [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L579) | [§6.1-05](../docs/RFC.md?plain=1#L338) | fold 后的水位回落。 |
| [`each_combination_of_stderr_approval_and_vision_drift_creates_one_tail_update`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L645) | [§6.2-05](../docs/RFC.md?plain=1#L366) | 从 session_init 和 checkpoint 恢复，stderr、approval、vision 的八种变更组合只在首次变化时产生一条尾部通知。 |

## `crates/anix-core/tests/v1_1_e2e.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`trace_fd_explicit_only_no_implicit_fd3_and_cycle_events`](../crates/anix-core/tests/v1_1_e2e.rs#L164) | [§4.1-05](../docs/RFC.md?plain=1#L181) | 末尾无换行的模型文本与使用量行分行显示。 |
| [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L297) | [§6.5-06](../docs/RFC.md?plain=1#L424) | 未信任项目配置的外部裁决器时返回 75，不执行该裁决器。 |
| [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L297) | [§8.3-02](../docs/RFC.md?plain=1#L550) | 项目级裁决器首次运行需取得信任，再次启动按裁决处理工具。 |
| [`vision_false_blocks_wal_pollution_and_vision_bridge_translates_and_mcp_tip_present`](../crates/anix-core/tests/v1_1_e2e.rs#L466) | [§5.5-02](../docs/RFC.md?plain=1#L304) | vision=false 时 WAL 无图片，有桥接器时保存文字描述，提示词含 MCP 指南。 |
| [`aclr_auto_summary_tail_fold_and_coprocess_inbox_lifecycle`](../crates/anix-core/tests/v1_1_e2e.rs#L600) | [§4.2-02](../docs/RFC.md?plain=1#L202) | 两条 Inbox 消息进入 done/，各产生一个 completed cycle_end，WAL 与 fold 事件记录自动摘要的用量。 |
| [`test_v1_1_1_session_id_persists_across_wal_resume_checkpoint_and_isolates_msh`](../crates/anix-core/tests/v1_1_e2e.rs#L809) | [§6.2-01](../docs/RFC.md?plain=1#L369) | 压缩及恢复后 Provider 与 WAL 使用同一会话标识，msh 环境不继承父会话标识。 |
| [`test_v1_1_2_msh_per_call_timeout_yield_bg_notification_and_kill_on_timeout`](../crates/anix-core/tests/v1_1_e2e.rs#L1033) | [§6.5-08](../docs/RFC.md?plain=1#L425) | 默认超时返回后台句柄并继续任务，显式强杀返回工具错误并继续推理。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§3.4-08](../docs/RFC.md?plain=1#L153) | 脚本正文引用未挂载的 ANIX_OUT 时给出迁移提示。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§3.4-09](../docs/RFC.md?plain=1#L151) | 无 shell 或禁止执行时请求工具直通，启动失败且不写 WAL。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§4.1-16](../docs/RFC.md?plain=1#L172) | 默认只向管道交付最终回复，禁用工具时可串联两级脚本。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§4.1-17](../docs/RFC.md?plain=1#L172) | explicit 逐字节交付工具输出，none 保持零字节并允许磁盘副作用。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§4.3-06](../docs/RFC.md?plain=1#L216) | reply 模式下真实 shell 观察到 fd 4 关闭且 ANIX_OUT 缺失。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§6.2-19](../docs/RFC.md?plain=1#L366) | 恢复旧 WAL 时默认 reply 在尾部通知原 explicit 契约发生变化。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§3.4-11](../docs/RFC.md?plain=1#L152) | 配置无法解析的 shell 时返回 2，目标 WAL 不被创建。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§4.1-18](../docs/RFC.md?plain=1#L172) | 带工具的轮次及失败轮次的部分文本进入诊断流，stdout 只交付成功终轮的回复。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§4.4-04](../docs/RFC.md?plain=1#L226) | shell=none 的系统提示声明无工具，省略 runtime_tips 与 available_skills。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§9.1-22](../docs/RFC.md?plain=1#L580) | 纯文本请求中 header.tools 为空，meta.output_mode 为 reply。 |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) | [§9.5-14](../docs/RFC.md?plain=1#L693) | run_start 记录本次真实生效的 shell=none 与 output=reply。 |

## `crates/anix-proto/src/ctl.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`ctl_frames_round_trip`](../crates/anix-proto/src/ctl.rs#L46) | [§7.2-07](../docs/RFC.md?plain=1#L461) | 控制帧编码与解码往返保持操作和参数。 |

## `crates/anix-proto/src/ir.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`ir_jsonl_header_and_messages_round_trip`](../crates/anix-proto/src/ir.rs#L397) | [§9.1-03](../docs/RFC.md?plain=1#L576) | header 在首行、消息在随后行，编码后可完整解码。 |

## `crates/anix-proto/src/provider.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`provider_events_round_trip`](../crates/anix-proto/src/provider.rs#L242) | [§9.2-11](../docs/RFC.md?plain=1#L655) | 把包含五项计数的 usage 帧序列化再解码，所有字段保持相等。 |

## `crates/anix-proto/src/wal.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`wal_events_round_trip`](../crates/anix-proto/src/wal.rs#L105) | [§6.2-12](../docs/RFC.md?plain=1#L368) | WAL 事件序列化再解码后字段保持相等。 |

## `crates/anix-proto/tests/rfc_ir.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) | [§9.1-01](../docs/RFC.md?plain=1#L572) | header 必填字段不得移入消息或其他对象。 |
| [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) | [§9.1-03](../docs/RFC.md?plain=1#L576) | 删除或移位必填 header 字段应拒绝。 |
| [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) | [§9.1-05](../docs/RFC.md?plain=1#L577) | 从 header 移除 tools 应导致 IR 无效。 |
| [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) | [§9.1-19](../docs/RFC.md?plain=1#L594) | 要求首行 header 及后续消息的位置和字段类型合法。 |
| [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L72) | [§9.1-07](../docs/RFC.md?plain=1#L579) | 对 header.params 和 meta 逐项验证 wire 类型。 |
| [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L72) | [§9.1-08](../docs/RFC.md?plain=1#L580) | 验证 meta 的 cwd、stderr_mode 与预算字段的类型。 |
| [`validated_ir_only_serializes_objects_for_arguments_and_provider_opaque`](../crates/anix-proto/tests/rfc_ir.rs#L107) | [§9.1-12](../docs/RFC.md?plain=1#L585) | 非对象的 arguments 或 provider_opaque 不应通过已校验 IR 的序列化。 |
| [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L161) | [§9.1-02](../docs/RFC.md?plain=1#L573) | 逐行解码真实样例，保留冻结正文、扩展字段和附件引用。 |
| [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L161) | [§9.1-09](../docs/RFC.md?plain=1#L581) | IR 样例中的未知扩展字段在解码后保留。 |
| [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L191) | [§9.1-13](../docs/RFC.md?plain=1#L586) | tool 结果必须连续且与对应调用一一配对。 |
| [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L191) | [§9.1-19](../docs/RFC.md?plain=1#L594) | 连续 tool 结果与同批调用一一对应。 |
| [`a_single_tool_result_cannot_close_two_calls_with_the_same_id`](../crates/anix-proto/tests/rfc_ir.rs#L219) | [§9.1-19](../docs/RFC.md?plain=1#L594) | 重复调用 ID 不能由同一个工具结果同时闭合。 |
| [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L231) | [§9.1-02](../docs/RFC.md?plain=1#L573) | 分别删除或改错四种角色的必填字段，解码应失败。 |
| [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L231) | [§9.1-11](../docs/RFC.md?plain=1#L584) | user 缺必填字段或 turn_id 型别错误时拒绝。 |
| [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L231) | [§9.1-13](../docs/RFC.md?plain=1#L586) | tool 消息缺少必需的调用标识时拒绝。 |
| [`assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state`](../crates/anix-proto/tests/rfc_ir.rs#L266) | [§9.1-12](../docs/RFC.md?plain=1#L585) | reasoning 或 opaque 单独存在时不能替代 text 或 tool_calls。 |
| [`ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared`](../crates/anix-proto/tests/rfc_ir.rs#L285) | [§9.1-19](../docs/RFC.md?plain=1#L594) | 普通轮次严格递增，仅 checkpoint 特例可共享锚点。 |
| [`coprocess_envelope_wraps_the_same_ir_without_moving_model_into_history`](../crates/anix-proto/tests/rfc_ir.rs#L305) | [§9.1-01](../docs/RFC.md?plain=1#L572) | 协进程在请求信封传入 model，内嵌消息不变。 |

## `crates/anix-proto/tests/rfc_provider.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values`](../crates/anix-proto/tests/rfc_provider.rs#L7) | [§9.2-12](../docs/RFC.md?plain=1#L656) | 逐个覆盖五项计数器的 uint64 边界并拒绝非整数。 |
| [`usage_requires_both_totals_even_when_all_three_details_are_present`](../crates/anix-proto/tests/rfc_provider.rs#L56) | [§9.2-12](../docs/RFC.md?plain=1#L656) | 即使三个明细齐全也不能缺少 input_tokens 或 output_tokens。 |

## `crates/anix-providers/src/anthropic.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`resolves_anthropic_endpoint_urls`](../crates/anix-providers/src/anthropic.rs#L942) | [§2-04](../docs/RFC.md?plain=1#L59) | Anthropic 端点接受基础 URL 及已含 /v1/messages 的路径，避免重复添加后缀。 |
| [`coalesces_adjacent_user_and_tool_results_and_replays_signed_thinking`](../crates/anix-providers/src/anthropic.rs#L959) | [§9.2-14](../docs/RFC.md?plain=1#L658) | Anthropic 合并相邻 user/tool 记录，并在请求中重放带签名的 thinking。 |
| [`sse_normalizes_inclusive_usage_captures_thinking_signature_and_tool_use`](../crates/anix-providers/src/anthropic.rs#L1063) | [§9.2-11](../docs/RFC.md?plain=1#L655) | Anthropic 的 SSE 输入用量加上缓存计数，产生包含式总数，同时保留思考签名与工具调用。 |
| [`sse_overloaded_error_is_retryable`](../crates/anix-providers/src/anthropic.rs#L1142) | [§9.2-15](../docs/RFC.md?plain=1#L659) | Anthropic overloaded SSE 错误被转换为 retryable=true。 |

## `crates/anix-providers/src/lib.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`resolves_dialect_from_argv0_symlink_and_version`](../crates/anix-providers/src/lib.rs#L232) | [§2-04](../docs/RFC.md?plain=1#L59) | 通过 argv[0] 的 BusyBox 名称选方言，并识别版本请求。 |
| [`resolves_dialect_from_subcommand_flag_and_env`](../crates/anix-providers/src/lib.rs#L252) | [§2-04](../docs/RFC.md?plain=1#L59) | 普通入口分别从子命令、显式选项和环境选择方言。 |

## `crates/anix-providers/src/openai_compat.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`endpoint_url_appends_chat_completions_once`](../crates/anix-providers/src/openai_compat.rs#L926) | [§2-04](../docs/RFC.md?plain=1#L59) | Chat 兼容端点只追加一次 chat/completions 路径。 |
| [`translates_frozen_system_degraded_runtime_update_tools_and_params`](../crates/anix-providers/src/openai_compat.rs#L939) | [§9.1-18](../docs/RFC.md?plain=1#L593) | Chat 请求保留首条 system，后续 runtime_update 按方言降级，同时传递工具及参数。 |
| [`omits_tools_when_empty_and_supports_disabling_stream_options`](../crates/anix-providers/src/openai_compat.rs#L984) | [§9.1-05](../docs/RFC.md?plain=1#L577) | IR 中空 tools 经转换后，Chat 请求不携带 tools 参数。 |
| [`replays_opaque_reasoning_and_flushes_tool_media_after_batch`](../crates/anix-providers/src/openai_compat.rs#L1006) | [§9.2-14](../docs/RFC.md?plain=1#L658) | Chat 适配器重放不透明推理状态，工具附件放在批量工具结果后。 |
| [`sse_streams_reasoning_text_and_trailing_usage_after_finish_reason`](../crates/anix-providers/src/openai_compat.rs#L1080) | [§9.2-18](../docs/RFC.md?plain=1#L625) | Chat SSE 输出 reasoning/text 增量，并读取 finish_reason 后的 usage。 |
| [`sse_accumulates_multi_chunk_tool_calls_and_extra_content`](../crates/anix-providers/src/openai_compat.rs#L1128) | [§9.2-18](../docs/RFC.md?plain=1#L625) | 把分块工具调用参数拼回完整对象，并保留 extra_content。 |
| [`sse_clean_eof_after_finish_reason_without_done_and_truncated_error`](../crates/anix-providers/src/openai_compat.rs#L1178) | [§9.2-18](../docs/RFC.md?plain=1#L625) | 区分已有 finish_reason 的正常 EOF 与未完成的截断。 |
| [`non_streaming_json_fallback_parses_message_and_inline_error`](../crates/anix-providers/src/openai_compat.rs#L1201) | [§9.2-18](../docs/RFC.md?plain=1#L625) | 非流式 JSON 既能解码普通回复，也能识别正文中的 error。 |

## `crates/anix-providers/src/openai_responses.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L636) | [§3.3-07](../docs/RFC.md?plain=1#L131) | 有 max_output_tokens 的 IR 经 Responses 转换后写入上游请求。 |
| [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L636) | [§9.1-06](../docs/RFC.md?plain=1#L578) | Responses 请求把 header 的输出预算映射为对应上游参数。 |
| [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L636) | [§9.1-07](../docs/RFC.md?plain=1#L579) | 在接收的 Responses HTTP 请求中检查推理等参数映射。 |
| [`parses_responses_sse_text_tool_and_usage`](../crates/anix-providers/src/openai_responses.rs#L670) | [§9.2-18](../docs/RFC.md?plain=1#L625) | Responses SSE 分别解码正文、工具调用和 usage 帧。 |

## `crates/anix-providers/tests/e2e_mock.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`version_flag_works_for_base_binary_and_busybox_symlinks`](../crates/anix-providers/tests/e2e_mock.rs#L202) | [§2-04](../docs/RFC.md?plain=1#L59) | 真实主程序和三个方言软链均能输出版本并成功退出。 |
| [`unsupported_ir_version_exits_2`](../crates/anix-providers/tests/e2e_mock.rs#L232) | [§9.1-04](../docs/RFC.md?plain=1#L575) | oneshot 读取不兼容 ir_version 时报告诊断并以 2 退出。 |
| [`busybox_symlink_openai_responses_end_to_end`](../crates/anix-providers/tests/e2e_mock.rs#L251) | [§2-04](../docs/RFC.md?plain=1#L59) | 通过 Responses 软链请求本地端点，检查返回的协议帧。 |
| [`busybox_symlink_openai_compat_end_to_end_and_unauthenticated_local`](../crates/anix-providers/tests/e2e_mock.rs#L289) | [§2-04](../docs/RFC.md?plain=1#L59) | 通过 Chat 软链请求免鉴权本地端点并检查响应帧。 |
| [`busybox_symlink_anthropic_end_to_end_with_inclusive_usage_and_headers`](../crates/anix-providers/tests/e2e_mock.rs#L328) | [§2-04](../docs/RFC.md?plain=1#L59) | Anthropic 软链请求携带预期 HTTP 头，usage 帧包含缓存用量。 |
| [`http_429_and_529_map_to_retryable_errors_and_400_is_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L394) | [§9.2-15](../docs/RFC.md?plain=1#L659) | HTTP 429/529 转换为可重试错误，普通 400 不重试。 |
| [`coprocess_mode_serves_multiple_requests_survives_error_and_exits_0_on_eof`](../crates/anix-providers/tests/e2e_mock.rs#L458) | [§9.2-18](../docs/RFC.md?plain=1#L625) | 同一协进程处理多个请求，错误后仍能响应，stdin EOF 后以 0 退出。 |
| [`http_400_image_rejection_maps_to_unsupported_media_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L615) | [§5.5-06](../docs/RFC.md?plain=1#L321) | 本地模拟上游拒绝图片，驱动返回 unsupported_media 且 retryable=false。 |
| [`session_affinity_headers_and_body_mapping_across_all_dialects`](../crates/anix-providers/tests/e2e_mock.rs#L674) | [§9.2-04](../docs/RFC.md?plain=1#L631) | 对三个 HTTP 方言核对默认 session header 与各自的 body 映射。 |

## `crates/anix-tools/tests/rfc_control.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L50) | [§7.2-09](../docs/RFC.md?plain=1#L468) | compact 或 fold 写入一条合法帧，JSON 转义后摘要无损。 |
| [`aclr_without_a_summary_omits_the_field_at_each_valid_turn_id_boundary`](../crates/anix-tools/tests/rfc_control.rs#L67) | [§7.2-01](../docs/RFC.md?plain=1#L474) | 省略摘要时，aclr 在有效 turn_id 边界都省略 JSON summary 字段。 |
| [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L78) | [§6.1-09](../docs/RFC.md?plain=1#L350) | 无参 aclr 输出用法并退出 2。 |
| [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L78) | [§7.2-09](../docs/RFC.md?plain=1#L468) | 非法 aclr 参数以使用错误结束，控制通道没有帧。 |
| [`fold_control_frames_stay_within_64_kib_even_after_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L97) | [§7.2-07](../docs/RFC.md?plain=1#L461) | 转义后的 fold 帧超出 64 KiB 时不能写入控制通道。 |
| [`aview_resizes_portrait_landscape_and_square_images_only_when_needed`](../crates/anix-tools/tests/rfc_control.rs#L149) | [§7.2-08](../docs/RFC.md?plain=1#L467) | 比较横、竖、方图在阈值两侧的实际输出尺寸。 |
| [`aview_file_input_and_stdin_agree_on_image_dimensions`](../crates/anix-tools/tests/rfc_control.rs#L172) | [§7.2-08](../docs/RFC.md?plain=1#L467) | 同一图像经文件和 stdin 输入得到相同输出尺寸。 |
| [`aview_preserves_image_content_and_orientation_in_the_published_png`](../crates/anix-tools/tests/rfc_control.rs#L187) | [§7.2-08](../docs/RFC.md?plain=1#L467) | 读取发布的 PNG，检查像素内容与方向。 |
| [`undecodable_input_or_invalid_options_do_not_emit_media`](../crates/anix-tools/tests/rfc_control.rs#L231) | [§7.2-08](../docs/RFC.md?plain=1#L467) | 输入无法解码或参数非法时不发布媒体控制帧。 |
| [`aview_selects_pdf_pages_with_their_actual_geometry_and_colors`](../crates/anix-tools/tests/rfc_control.rs#L246) | [§7.2-03](../docs/RFC.md?plain=1#L467) | 四页文档的 15 种非空页集合，从文件、显式和隐式 stdin 输入，逐页核对 PNG 尺寸及色块。 |
| [`aview_rejects_invalid_pdf_page_expressions_without_publishing_attachments`](../crates/anix-tools/tests/rfc_control.rs#L315) | [§7.2-04](../docs/RFC.md?plain=1#L467) | 页码语法错误时以非零状态退出且不发布附件。 |

## `crates/msh/tests/rfc_shell.rs`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L35) | [§7.1-03](../docs/RFC.md?plain=1#L437) | 逐字节检查 NUL 的 8192 字节窗口及窗口外非法 UTF-8。 |
| [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L35) | [§7.1-04](../docs/RFC.md?plain=1#L439) | 对探测边界内外的 NUL 和非法 UTF-8 检查分类结果。 |
| [`ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites`](../crates/msh/tests/rfc_shell.rs#L66) | [§7.1-06](../docs/RFC.md?plain=1#L441) | CSI 终结字节、OSC 终止方式和回车覆盖。 |
| [`line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text`](../crates/msh/tests/rfc_shell.rs#L107) | [§7.1-07](../docs/RFC.md?plain=1#L442) | 等于行数阈值时保留全部；超过时核对首尾、行数和完整文件。 |
| [`byte_limit_independent_from_line_limit_preserves_multibyte_file`](../crates/msh/tests/rfc_shell.rs#L143) | [§7.1-07](../docs/RFC.md?plain=1#L442) | 越过字节阈值时单独触发折叠，多字节内容的保存文件无损。 |
| [`cli_applies_environment_limits_to_cleaned_lines_and_bytes_before_saving_output`](../crates/msh/tests/rfc_shell.rs#L178) | [§7.1-07](../docs/RFC.md?plain=1#L442) | 环境变量覆盖阈值后，按清洗后的行数和字节数触发折叠。 |
| [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L220) | [§7.1-02](../docs/RFC.md?plain=1#L436) | 对照真实子 shell 的六个非交互变量，同时验证合并双流及退出状态。 |
| [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L244) | [§7.1-03](../docs/RFC.md?plain=1#L437) | 真实 msh 在 ANSI 清洗和折叠前先识别二进制输出。 |
| [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L244) | [§7.1-04](../docs/RFC.md?plain=1#L439) | 检查保存的二进制原始字节和熔断提示，标准输出不含原始数据。 |
| [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L244) | [§7.1-05](../docs/RFC.md?plain=1#L440) | 二进制输出单行说明。 |
| [`timeout_reports_124_before_a_finite_longer_command_completes`](../crates/msh/tests/rfc_shell.rs#L269) | [§7.1-09](../docs/RFC.md?plain=1#L444) | 独立 msh 的环境变量配置前台等待时长及显式强杀。 |
| [`unconfigured_msh_folds_only_above_200_lines_or_8192_bytes`](../crates/msh/tests/rfc_shell.rs#L283) | [§7.1-07](../docs/RFC.md?plain=1#L442) | 用无配置的真实 msh 检查 200 行、8192 字节及各自的相邻值。 |

## `tests/test_approval.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L132) | [§5.3-01](../docs/RFC.md?plain=1#L284) | 提示与回答走控制终端，管道 stdin 保留为模型输入。 |
| [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L132) | [§5.3-03](../docs/RFC.md?plain=1#L287) | n 返回模型错误，支持空理由及 UTF-8 文本。 |
| [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L158) | [§5.1-03](../docs/RFC.md?plain=1#L246) | Always 对同一可执行程序生效，换进程读取同一 WAL 后需重新审批。 |
| [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L158) | [§5.3-04](../docs/RFC.md?plain=1#L288) | a 的授权以可执行程序及 PID 为界。 |
| [`test_editor_receives_private_original_and_only_the_edited_command_runs`](../tests/test_approval.py#L169) | [§5.3-05](../docs/RFC.md?plain=1#L289) | 编辑器收到私有临时文件，只执行保存后的命令并清理文件。 |
| [`test_failed_or_empty_edit_removes_the_file_and_runs_neither_command`](../tests/test_approval.py#L196) | [§5.3-05](../docs/RFC.md?plain=1#L289) | 编辑失败或保存为空时，原命令和编辑结果均未执行，临时文件被清理。 |
| [`test_yes_authorizes_only_one_call_in_a_batch`](../tests/test_approval.py#L208) | [§5.3-02](../docs/RFC.md?plain=1#L286) | y 只批准一条，同批下一条可单独拒绝。 |
| [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L223) | [§5.2-04](../docs/RFC.md?plain=1#L276) | 需交给人工处理且无控制终端时返回 75，不执行命令。 |
| [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L223) | [§5.4-01](../docs/RFC.md?plain=1#L293) | 无控制终端时打印原命令、记录终止事件并退出 75。 |
| [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L223) | [§6.5-06](../docs/RFC.md?plain=1#L424) | 无控制终端且需要人工审批时返回 75。 |
| [`test_external_escalation_hands_the_decision_to_the_terminal`](../tests/test_approval.py#L238) | [§5.2-04](../docs/RFC.md?plain=1#L276) | 外部返回 2 后从控制终端读取最终选择。 |
| [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L253) | [§5.2-01](../docs/RFC.md?plain=1#L264) | 外部裁决器读到一行 JSON、真实进程标识及 EOF。 |
| [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L253) | [§5.2-02](../docs/RFC.md?plain=1#L274) | 外部返回 0，无控制终端也能执行一次已批准的命令。 |
| [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L253) | [§5.2-06](../docs/RFC.md?plain=1#L279) | 外部裁决允许时 Trace 关联真实调用和裁决来源。 |
| [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L272) | [§5.2-03](../docs/RFC.md?plain=1#L275) | 外部返回 1，裁剪拒绝理由、为空时给出默认值，继续推理。 |
| [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L272) | [§5.2-06](../docs/RFC.md?plain=1#L279) | 外部拒绝时保留对应来源及裁决结果。 |
| [`test_external_error_and_timeout_surface_stderr_and_end_the_cycle_with_75`](../tests/test_approval.py#L292) | [§5.2-05](../docs/RFC.md?plain=1#L277) | 其他非零或超时透出诊断，结束当前轮且不执行工具。 |
| [`test_unavailable_external_decider_exits_75_and_records_the_failed_approval`](../tests/test_approval.py#L314) | [§5.2-05](../docs/RFC.md?plain=1#L277) | 裁决器不可用时以 75 退出，Trace 记录审批失败且命令未执行。 |
| [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L327) | [§5.1-01](../docs/RFC.md?plain=1#L243) | auto 和白名单命令无需调用外部裁决器。 |
| [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L327) | [§5.1-02](../docs/RFC.md?plain=1#L244) | deny 保留白名单，拦截时返回工具错误，开启 fail-on-deny 时退出 75。 |
| [`test_ctrl_c_at_the_approval_prompt_returns_to_the_same_shell`](../tests/test_approval.py#L353) | [§6.4-02](../docs/RFC.md?plain=1#L409) | 在内置审批处只按一次 Ctrl+C，命令不执行，原 shell 恢复并读到退出码 130。 |
| [`test_interrupt_during_builtin_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L361) | [§5.2-06](../docs/RFC.md?plain=1#L279) | 内置审批被 SIGINT 中断时 Trace 仍应标记 builtin。 |
| [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L388) | [§5.2-06](../docs/RFC.md?plain=1#L279) | 外部审批被 SIGTERM 中断时 Trace 仍应标记 external。 |
| [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L388) | [§6.4-03](../docs/RFC.md?plain=1#L410) | 外部审批中收到 SIGTERM，退出 143 并记录中断。 |

## `tests/test_cycle_accounting.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_each_declared_cycle_end_reason_matches_the_cause_and_os_status`](../tests/test_cycle_accounting.py#L49) | [§9.5-15](../docs/RFC.md?plain=1#L695) | 逐个触发 RFC 列举的 cycle_end.reason，包括共享同一进程退出码的不同原因，并与 OS、run_end 对照。 [断言 L82](../tests/test_cycle_accounting.py#L82) · [断言 L87](../tests/test_cycle_accounting.py#L87) |
| [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) | [§9.5-16](../docs/RFC.md?plain=1#L694) | 对初始输入和两封 Inbox 消息分别核对起止配对、来源、递增编号以及最后一次 idle。 [断言 L134](../tests/test_cycle_accounting.py#L134) · [断言 L137](../tests/test_cycle_accounting.py#L137) · [断言 L140](../tests/test_cycle_accounting.py#L140) |
| [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) | [§9.5-17](../docs/RFC.md?plain=1#L695) | 四次请求分属三个 cycle；每轮核对轮次数、五字段本轮和累计用量、会话总量及末次回答的水位。 [断言 L163](../tests/test_cycle_accounting.py#L163) |
| [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) | [§9.5-18](../docs/RFC.md?plain=1#L697) | 将每个 llm_response 与实际发给 anix 的四组五字段 usage 逐项比对。 [断言 L150](../tests/test_cycle_accounting.py#L150) |
| [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) | [§9.5-19](../docs/RFC.md?plain=1#L701) | 在队列排空并闲置退出后，比对 run_end 总轮数、进程用量、会话用量及最终水位。 [断言 L170](../tests/test_cycle_accounting.py#L170) |
| [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) | [§9.3-04](../docs/RFC.md?plain=1#L666) | 同一批任务在无状态和恢复旧 WAL 时，on-exit 钩子均能读到独立核算的五项用量及最终水位。 [断言 L168](../tests/test_cycle_accounting.py#L168) |
| [`test_failed_exit_hook_preserves_reports_later_hooks_and_primary_status`](../tests/test_cycle_accounting.py#L179) | [§9.3-05](../docs/RFC.md?plain=1#L666) | on-exit 钩子非零退出时，后续钩子仍收到相同的退出报告；失败只写 Trace，主进程保留成功或失败状态。 [断言 L191](../tests/test_cycle_accounting.py#L191) · [断言 L194](../tests/test_cycle_accounting.py#L194) · [断言 L197](../tests/test_cycle_accounting.py#L197) |

## `tests/test_discovery_indexes.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_declared_builtin_guides_match_materialized_files_and_the_first_request`](../tests/test_discovery_indexes.py#L49) | [§9.6-05](../docs/RFC.md?plain=1#L712) | 对照 RFC 内置名单，核对默认及自定义 XDG 状态目录实际释放的指南和 Provider 收到的绝对路径。 [断言 L59](../tests/test_discovery_indexes.py#L59) · [断言 L61](../tests/test_discovery_indexes.py#L61) |
| [`test_tip_masks_overrides_and_sort_order_across_both_user_layers`](../tests/test_discovery_indexes.py#L67) | [§9.6-06](../docs/RFC.md?plain=1#L715) | 对同一内置指南交叉测试全局和项目层的缺席、替换、空文件及 /dev/null 屏蔽，核对整个索引。 [断言 L110](../tests/test_discovery_indexes.py#L110) |
| [`test_tip_masks_overrides_and_sort_order_across_both_user_layers`](../tests/test_discovery_indexes.py#L67) | [§9.6-07](../docs/RFC.md?plain=1#L716) | 逐项比对跨目录、大小写与空格的路径排序，排除隐藏文件、子目录、其他后缀和正文。 [断言 L113](../tests/test_discovery_indexes.py#L113) · [断言 L115](../tests/test_discovery_indexes.py#L115) |
| [`test_skill_index_requires_both_frontmatter_fields_and_excludes_private_text`](../tests/test_discovery_indexes.py#L118) | [§9.4-05](../docs/RFC.md?plain=1#L673) | 仅从 SKILL.md 开头的 YAML 读取两个必需字段，缺字段、正文中的伪字段和其他元数据均不进入索引。 [断言 L135](../tests/test_discovery_indexes.py#L135) · [断言 L137](../tests/test_discovery_indexes.py#L137) |
| [`test_skill_xml_paths_reopen_files_with_posix_whitespace`](../tests/test_discovery_indexes.py#L140) | [§9.4-06](../docs/RFC.md?plain=1#L679) | 解析实际 XML 后逐项打开技能文件，检查目录名中的空格、Tab、LF、CR 是否仍指向原文件。 |

## `tests/test_distribution.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L121) | [§10.3-01](../docs/RFC.md?plain=1#L756) | 重新安装后 rc 文件保留，私有工具仅在 libexec。 |
| [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L165) | [§10.3-05](../docs/RFC.md?plain=1#L762) | doctor 发现无法创建匿名管道并给出修复提示。 |
| [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L165) | [§10.3-06](../docs/RFC.md?plain=1#L763) | doctor 检测锁操作失败或失去互斥性。 |
| [`test_usage_line_resets_dim_before_the_final_newline_on_a_terminal`](../tests/test_distribution.py#L219) | [§4.1-07](../docs/RFC.md?plain=1#L182) | 在真实 TTY 上精确检查 Dim 使用量行在最后一个换行前重置 ANSI 样式。 |
| [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L251) | [§1.2-01](../docs/RFC.md?plain=1#L21) | 安装后的 .agent 脚本以 shebang 从 PATH 或相对路径直接启动。 |
| [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L251) | [§4.1-01](../docs/RFC.md?plain=1#L171) | 有参数的可执行脚本通过 PATH 或相对路径启动，TTY 保持打开即可推理。 |
| [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L251) | [§10.3-01](../docs/RFC.md?plain=1#L756) | 安装后的 bash.agent 保持 stdin TTY 打开，直接或经相对路径启动都能完成。 |
| [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) | [§2-03](../docs/RFC.md?plain=1#L63) | 安装后从 msh 调用私有 aview 和 aclr，检查附件及折叠结果。 |
| [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) | [§4.1-04](../docs/RFC.md?plain=1#L172) | 已安装的脚本通过真实 shell 输出 A、NUL、B，不掺入模型回复。 |
| [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) | [§7.2-06](../docs/RFC.md?plain=1#L460) | 原生工具和图片管道的字节一致性。 |
| [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) | [§9.1-15](../docs/RFC.md?plain=1#L589) | 已安装 Provider 将真实 aview 附件送入本地 HTTP 请求。 |

## `tests/test_msh_jobs.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit`](../tests/test_msh_jobs.py#L88) | [§7.1-12](../docs/RFC.md?plain=1#L448) | 立即后台启动时 fd 4–6 和三个侧信道变量均已关闭或清除，进程组和会话隔离。 |
| [`test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit`](../tests/test_msh_jobs.py#L88) | [§7.1-14](../docs/RFC.md?plain=1#L445) | 运行时目录下 jobs 为 0700、日志为 0600，后台退出前不存在最终状态文件，随后读到真实的非零退出码。 |
| [`test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log`](../tests/test_msh_jobs.py#L118) | [§1.4-01](../docs/RFC.md?plain=1#L37) | 正等待到期后 msh 成功返回后台预览，子进程继续写完整日志。 |
| [`test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log`](../tests/test_msh_jobs.py#L118) | [§7.1-13](../docs/RFC.md?plain=1#L449) | 真实命令等待到期时仅返回清洗后的有限预览，后台继续执行并保留完整原始日志。 |
| [`test_foreground_completion_returns_real_status_and_removes_job_files`](../tests/test_msh_jobs.py#L142) | [§7.1-11](../docs/RFC.md?plain=1#L446) | 前台成功与普通非零退出保留双流输出、透传真实退出码并清理任务文件。 |
| [`test_foreground_int_and_term_clean_up_in_both_timeout_modes`](../tests/test_msh_jobs.py#L151) | [§7.1-16](../docs/RFC.md?plain=1#L456) | 前台等待中分别发送 INT、TERM，两种超时策略都清理整组并返回 130、143。 |
| [`test_zero_with_explicit_kill_waits_one_second_and_leaves_no_job`](../tests/test_msh_jobs.py#L182) | [§7.1-15](../docs/RFC.md?plain=1#L455) | 显式强杀且指定 0 秒时先等待一秒，输出已有文本与超时标记，清理任务文件。 |
| [`test_long_commands_execute_their_suffix_and_handoff_with_a_single_bounded_frame`](../tests/test_msh_jobs.py#L193) | [§7.2-10](../docs/RFC.md?plain=1#L475) | 511 至 66,000 字节的命令都执行到末尾，fd 6 只产生一条有效且有界的 JSONL 帧。 |
| [`test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call`](../tests/test_msh_jobs.py#L255) | [§7.1-10](../docs/RFC.md?plain=1#L444) | 单次请求依次覆盖缺省、零、正数、上限两侧与回到默认，自定义 shell 收到 -c、原命令、策略变量及任务目录。 |
| [`test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call`](../tests/test_msh_jobs.py#L255) | [§9.1-20](../docs/RFC.md?plain=1#L599) | msh 的工具声明仅要求 cmd，timeout_sec 为非负整数，kill_on_timeout 为可选布尔值。 |
| [`test_single_call_never_exceeds_a_configured_maximum_below_the_default_wait`](../tests/test_msh_jobs.py#L270) | [§8.2-03](../docs/RFC.md?plain=1#L506) | 只将硬上限配置为 2 时，每个显式请求仍应裁剪到 2，即使默认等待为 120。 |
| [`test_environment_maximum_overrides_toml_for_each_explicit_request`](../tests/test_msh_jobs.py#L276) | [§8.2-04](../docs/RFC.md?plain=1#L505) | 环境覆盖 TOML 的默认等待与单次上限，单次覆盖后恢复默认等待。 |
| [`test_background_completion_is_reported_once_before_inference_and_replays_from_wal`](../tests/test_msh_jobs.py#L284) | [§7.2-12](../docs/RFC.md?plain=1#L479) | 后台任务结束后仅在下一次请求前通知一次，冻结前缀、WAL 和重启恢复结果可核对。 |
| [`test_background_completion_is_reported_once_before_inference_and_replays_from_wal`](../tests/test_msh_jobs.py#L284) | [§9.5-08](../docs/RFC.md?plain=1#L699) | started 和 finished 通过相同任务标识、PID、进程组和日志路径关联，并报告真实状态及经过的毫秒数。 |
| [`test_positive_timeout_dispatches_the_next_request_while_the_job_is_alive`](../tests/test_msh_jobs.py#L288) | [§7.1-17](../docs/RFC.md?plain=1#L454) | 正数等待到期后，后一轮推理可以在任务仍存活时运行，随后报告一次完成结果。 |
| [`test_terminal_title_and_overwritten_progress_are_cleaned_from_the_completion_notice`](../tests/test_msh_jobs.py#L292) | [§7.2-13](../docs/RFC.md?plain=1#L479) | 终端标题和已覆盖的进度文本应从后台通知中清除，在 WAL 及后续 Provider 请求中使用清洗后的文本。 |
| [`test_finished_group_without_an_exit_file_reports_null_exactly_once`](../tests/test_msh_jobs.py#L346) | [§9.5-07](../docs/RFC.md?plain=1#L699) | 检查已终止的任务组且没有退出码文件时，完成事件含显式 null，后续推理不重复通知。 |
| [`test_hard_kill_grace_waits_for_a_living_descendant_without_repeated_polling`](../tests/test_msh_jobs.py#L390) | [§6.3-03](../docs/RFC.md?plain=1#L399) | 强杀时组长退出、有限子进程仍在，Linux 宽限期的等待不应持续空转。 |

## `tests/test_output_contract.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_output_resolves_every_subset_of_cli_env_script_project_and_global`](../tests/test_output_contract.py#L17) | [§3.4-12](../docs/RFC.md?plain=1#L141) | 对五层覆盖的 32 种开关组合轮换三个候选值，最高生效层决定 WAL、IR 和 stdout。 [断言 L50](../tests/test_output_contract.py#L50) |
| [`test_output_startup_matrix_warns_or_stops_before_wal_and_provider`](../tests/test_output_contract.py#L54) | [§3.4-13](../docs/RFC.md?plain=1#L153) | 遍历 shell、output、approval 组合和正文迁移提示；禁止的组合在 WAL 或 Provider 请求出现之前报错。 [断言 L69](../tests/test_output_contract.py#L69) |
| [`test_shell_resolution_requires_an_executable_file_before_any_provider_request`](../tests/test_output_contract.py#L74) | [§3.4-11](../docs/RFC.md?plain=1#L152) | 对不可执行的文件和目录，分别通过 PATH 名称与绝对路径启动，须在写 WAL 和启动 Provider 前以 2 退出。 [断言 L92](../tests/test_output_contract.py#L92) |
| [`test_reply_visibility_before_done_in_oneshot_and_coprocess`](../tests/test_output_contract.py#L96) | [§4.1-19](../docs/RFC.md?plain=1#L172) | 以已处理的第二帧作同步点：纯文本的首段在 done 前可读，启用 shell 时同一时点的 stdout 为空。 [断言 L149](../tests/test_output_contract.py#L149) · [断言 L155](../tests/test_output_contract.py#L155) |
| [`test_reply_newline_depends_on_stdout_tty_and_last_byte`](../tests/test_output_contract.py#L158) | [§4.1-20](../docs/RFC.md?plain=1#L172) | 比较真实 TTY 和管道；空回复、无换行、已有一个或两个换行时只在规定条件下补一个 LF。 [断言 L183](../tests/test_output_contract.py#L183) |
| [`test_resume_distinguishes_shell_capability_from_executable_path_and_output`](../tests/test_output_contract.py#L186) | [§6.2-20](../docs/RFC.md?plain=1#L366) | 恢复同一 WAL 时只对 output 或 shell 启用状态的实际变化追加一次尾部通知，冻结前缀保持原样。 [断言 L206](../tests/test_output_contract.py#L206) · [断言 L211](../tests/test_output_contract.py#L211) · [断言 L215](../tests/test_output_contract.py#L215) |

## `tests/test_provider_dialects.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L112) | [§9.1-10](../docs/RFC.md?plain=1#L583) | 首条系统文本字节保真与尾部系统消息映射。 |
| [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L112) | [§9.1-18](../docs/RFC.md?plain=1#L593) | 对三个实际 HTTP 方言核对冻结首条文本及后续 runtime_update 的顺序。 |
| [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L149) | [§9.1-11](../docs/RFC.md?plain=1#L584) | 含 media 的 user 消息转换为与原文件字节一致的 HTTP 图片。 |
| [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L149) | [§9.1-14](../docs/RFC.md?plain=1#L588) | 逐个发送 PNG、JPEG、WebP、GIF，核对实际上游请求的 MIME 和解码字节。 |
| [`test_coprocess_reopens_replaced_media_and_recovers_after_unlink`](../tests/test_provider_dialects.py#L176) | [§9.1-15](../docs/RFC.md?plain=1#L589) | 同一协进程按请求重开图片，替换后读取新字节、删除后给出可恢复错误。 |
| [`test_oneshot_unreadable_and_missing_paths_fail_before_dispatch`](../tests/test_provider_dialects.py#L199) | [§9.1-15](../docs/RFC.md?plain=1#L589) | 文件不可读或缺失时 oneshot 在 HTTP 前失败。 |
| [`test_upstream_media_rejection_is_distinct_from_auth_and_transient_failures`](../tests/test_provider_dialects.py#L217) | [§5.5-06](../docs/RFC.md?plain=1#L321) | 上游拒绝媒体时产生专属且不可重试的错误。 |
| [`test_partial_text_then_upstream_eof_yields_error_and_coprocess_accepts_next_request`](../tests/test_provider_dialects.py#L244) | [§9.2-18](../docs/RFC.md?plain=1#L625) | 上游正文中途断流后本轮以 error 收尾，同一协进程可处理下一帧。 |
| [`test_unsupported_ir_version_is_diagnosed_before_http_in_both_modes`](../tests/test_provider_dialects.py#L273) | [§9.1-04](../docs/RFC.md?plain=1#L575) | 不支持的 IR 版本在 HTTP 请求前被检测。 |
| [`test_fragmented_tool_arguments_produce_one_call_with_the_original_id`](../tests/test_provider_dialects.py#L289) | [§9.2-18](../docs/RFC.md?plain=1#L625) | 跨帧工具参数最终产生一条 tool_call，保留原始 ID 和参数内容。 |
| [`test_inclusive_usage_preserves_wide_counts_and_available_cache_details`](../tests/test_provider_dialects.py#L331) | [§9.2-11](../docs/RFC.md?plain=1#L655) | 从实际 HTTP 方言读取大整数和缓存明细，核对包含式总量。 |
| [`test_wal_session_reaches_http_across_cli_restarts_and_provider_switches`](../tests/test_provider_dialects.py#L357) | [§9.2-04](../docs/RFC.md?plain=1#L631) | 从同一 WAL 多次启动并切换 Provider，实际 HTTP 请求均带持久化的会话标识。 |
| [`test_session_routing_works_in_oneshot_and_per_envelope_in_coprocess`](../tests/test_provider_dialects.py#L409) | [§9.2-02](../docs/RFC.md?plain=1#L624) | 检查 oneshot 环境与协进程逐帧 session_id 覆盖的实际 HTTP 路由。 |
| [`test_header_rename_and_disable_apply_only_to_the_current_request`](../tests/test_provider_dialects.py#L435) | [§9.2-04](../docs/RFC.md?plain=1#L631) | 运行中重命名或禁用指定的路由头，只影响当前请求。 |
| [`test_responses_cache_key_has_native_string_boolean_and_null_semantics`](../tests/test_provider_dialects.py#L458) | [§9.2-05](../docs/RFC.md?plain=1#L632) | Responses 原生缓存键支持默认、字符串、布尔值及 null。 |
| [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L477) | [§9.2-06](../docs/RFC.md?plain=1#L633) | 兼容 Chat 端点的两个 Body 路由字段分别按需启用。 |
| [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L477) | [§9.2-07](../docs/RFC.md?plain=1#L634) | Anthropic 只发送路由头，Body 保持协议格式。 |

## `tests/test_provider_streaming.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_coprocess_overrides_apply_to_one_frame_then_environment_defaults_resume`](../tests/test_provider_streaming.py#L102) | [§9.2-02](../docs/RFC.md?plain=1#L624) | 覆盖 model 和 timeout_sec 后下一帧回到环境值，前一超时不会传染。 |
| [`test_wire_trace_keeps_raw_msh_output_while_the_model_receives_cleaned_text`](../tests/test_provider_streaming.py#L198) | [§9.5-04](../docs/RFC.md?plain=1#L704) | 模型只看清洗后文本，私有 wire 文件保留命令实际输出的 ANSI 字节。 |
| [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L231) | [§6.5-01](../docs/RFC.md?plain=1#L425) | 第一轮的长 msh 命令可超过 Provider 时限，后续请求仍然成功。 |
| [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L231) | [§9.2-08](../docs/RFC.md?plain=1#L660) | 工具执行发生在两轮 HTTP 请求之间，不占用下一请求的超时预算。 |
| [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L264) | [§1.4-01](../docs/RFC.md?plain=1#L37) | 显式请求 kill_on_timeout 时，下一轮 Provider 收到工具超时错误。 |
| [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L264) | [§6.5-01](../docs/RFC.md?plain=1#L425) | 显式强杀超时后，把工具错误送入下一次 Provider 请求。 |
| [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L293) | [§9.2-08](../docs/RFC.md?plain=1#L660) | 缓慢发送工具参数，参数完整前不启动 shell。 |
| [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L319) | [§9.5-13](../docs/RFC.md?plain=1#L703) | 下一帧尚未发送时上一帧已在 Trace 中可见；完成前 WAL 没有 assistant，完成后才提交。 |
| [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L352) | [§9.2-14](../docs/RFC.md?plain=1#L658) | 多次交错工具调用结束后的实际 HTTP 请求保留此前的不透明推理状态。 |
| [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L412) | [§9.2-15](../docs/RFC.md?plain=1#L659) | 对 429、503 先记录错误，再向真实本地端点重发同一请求。 |
| [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L412) | [§9.5-05](../docs/RFC.md?plain=1#L702) | 逐个检查 HTTP 429、503 对应的 provider_error，provider_stderr 字段均为字符串。 |
| [`test_executable_script_on_path_and_by_relative_name_obeys_tty_eof_rules`](../tests/test_provider_streaming.py#L445) | [§4.1-02](../docs/RFC.md?plain=1#L171) | 无参数且 stdin 为终端时等待实际 EOF，再将输入交给 Provider。 |
| [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L495) | [§6.4-01](../docs/RFC.md?plain=1#L409) | 等待 Provider 时中断，原 shell 可继续执行管道。 |
| [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L552) | [§6.4-01](../docs/RFC.md?plain=1#L409) | 在 Provider 已关 stdout、进程仍未退出时中断，原 shell 恢复且状态为 130。 |
| [`test_inbox_commits_one_message_then_interrupts_the_next_in_the_same_coprocess`](../tests/test_provider_streaming.py#L592) | [§4.2-02](../docs/RFC.md?plain=1#L202) | 成功消息先提交，下一条被中断时仍可识别已完成的轮次。 |

## `tests/test_rfc_tables.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) | [§3.3-09](../docs/RFC.md?plain=1#L133) | 从 RFC 读取 approval 的所有合法值，与真实 --help 公布的集合逐项比较。 [断言 L77](../tests/test_rfc_tables.py#L77) |
| [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) | [§3.3-10](../docs/RFC.md?plain=1#L134) | 从 RFC 读取 stderr 的所有合法值，与真实 --help 公布的集合逐项比较。 [断言 L77](../tests/test_rfc_tables.py#L77) |
| [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) | [§3.1-12](../docs/RFC.md?plain=1#L83) | 对照 §3.1 的封闭选项集合、§3.3 的枚举和真实 CLI help；新增、缺失或多余取值均可定位。 [断言 L75](../tests/test_rfc_tables.py#L75) |
| [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) | [§3.3-14](../docs/RFC.md?plain=1#L128) | 从 RFC 读取 output 的所有合法值，与真实 --help 公布的集合逐项比较。 [断言 L77](../tests/test_rfc_tables.py#L77) |
| [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) | [§3.3-09](../docs/RFC.md?plain=1#L133) | 读取 approval 默认值；用 CLI 和 Frontmatter 逐个选择合法值，核对 session_init.config。 [断言 L100](../tests/test_rfc_tables.py#L100) · [断言 L109](../tests/test_rfc_tables.py#L109) |
| [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) | [§3.3-10](../docs/RFC.md?plain=1#L134) | 读取 stderr 默认值；用 CLI 和 Frontmatter 逐个选择合法值，核对 session_init.config。 [断言 L100](../tests/test_rfc_tables.py#L100) · [断言 L109](../tests/test_rfc_tables.py#L109) |
| [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) | [§3.3-14](../docs/RFC.md?plain=1#L128) | 读取 output 默认值；用 CLI 和 Frontmatter 逐个选择合法值，核对 session_init.config。 [断言 L100](../tests/test_rfc_tables.py#L100) · [断言 L109](../tests/test_rfc_tables.py#L109) |
| [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) | [§3.4-10](../docs/RFC.md?plain=1#L150) | 未知 output、approval 或 stderr 在 CLI 与 Frontmatter 两条路径都返回 2，WAL 和 Provider 请求均不存在。 [断言 L115](../tests/test_rfc_tables.py#L115) · [断言 L118](../tests/test_rfc_tables.py#L118) |
| [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) | [§6.5-09](../docs/RFC.md?plain=1#L426) | Provider 消费完请求后向主进程发送 INT 或 TERM，实际退出状态等于 128 加相应信号编号。 [断言 L158](../tests/test_rfc_tables.py#L158) · [断言 L161](../tests/test_rfc_tables.py#L161) |
| [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) | [§6.5-10](../docs/RFC.md?plain=1#L421) | 正常回复的进程状态等于表中 EX_OK。 [断言 L161](../tests/test_rfc_tables.py#L161) |
| [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) | [§6.5-11](../docs/RFC.md?plain=1#L422) | 永久 Provider 错误的进程状态等于 GENERAL_ERR。 [断言 L161](../tests/test_rfc_tables.py#L161) |
| [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) | [§6.5-12](../docs/RFC.md?plain=1#L423) | 未知 CLI 选项的进程状态等于 EX_USAGE。 [断言 L161](../tests/test_rfc_tables.py#L161) |
| [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) | [§6.5-13](../docs/RFC.md?plain=1#L424) | 拒绝工具并启用 fail-on-deny 时进程状态等于 EX_TEMPFAIL。 [断言 L161](../tests/test_rfc_tables.py#L161) |
| [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) | [§6.5-14](../docs/RFC.md?plain=1#L425) | 第一轮仍需执行工具且 max_turns=1 时进程状态等于 EX_TIMEOUT。 [断言 L161](../tests/test_rfc_tables.py#L161) |

## `tests/test_shell_pipeline.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_output_mode_controls_payload_fd_without_changing_diagnostic_or_control_pipes`](../tests/test_shell_pipeline.py#L73) | [§4.3-07](../docs/RFC.md?plain=1#L216) | 分别在三种 output 下检查真实 msh 的 fd 4 和 ANIX_OUT，并核对 fd 5、6 始终是只写管道。 [断言 L102](../tests/test_shell_pipeline.py#L102) |
| [`test_output_mode_controls_payload_fd_without_changing_diagnostic_or_control_pipes`](../tests/test_shell_pipeline.py#L73) | [§4.3-08](../docs/RFC.md?plain=1#L215) | 向工具 stdout 与 stderr 分别写入不同文本，两者均进入同一条模型工具结果。 [断言 L97](../tests/test_shell_pipeline.py#L97) |
| [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L106) | [§7.2-02](../docs/RFC.md?plain=1#L461) | 按含 LF 的实际序列化长度逐个比较 aclr 发送端和内核接收端的判定。 |
| [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L106) | [§7.2-07](../docs/RFC.md?plain=1#L461) | 按序列化字节数逐一比较发送端及接收端的 64 KiB 边界。 |
| [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L158) | [§6.2-07](../docs/RFC.md?plain=1#L389) | 文件增长受限时保留原有字节，只在尾行完整可回放时继续。 |
| [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L158) | [§6.2-12](../docs/RFC.md?plain=1#L368) | 限制文件增长制造写入失败，检查已写字节和恢复边界。 |
| [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L158) | [§6.5-04](../docs/RFC.md?plain=1#L422) | WAL 无法追加时返回 1，已有前缀保留。 |
| [`test_archived_wal_growth_keeps_resume_reads_bounded_at_the_latest_checkpoint`](../tests/test_shell_pipeline.py#L203) | [§6.2-08](../docs/RFC.md?plain=1#L388) | 活跃窗口不变时将归档从 1 MiB 增至 32 MiB，用系统调用计量恢复读取量。 |
| [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L258) | [§9.4-01](../docs/RFC.md?plain=1#L679) | 模型从真实 XML 索引读取含特殊字符的技能路径，经 msh 按需获取完整正文。 |
| [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L258) | [§9.4-04](../docs/RFC.md?plain=1#L673) | 含特殊字符的路径能从 XML 索引解码，并由 msh 按需读取正文。 |
| [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L333) | [§4.1-10](../docs/RFC.md?plain=1#L171) | 真实管道有无脚本参数均等到 EOF，保留跨写入的 UTF-8 及内嵌 NUL。 |
| [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L333) | [§4.1-11](../docs/RFC.md?plain=1#L175) | 逐个通过真实管道连接 fd 4 至 9，读完后保留输入来源和完整正文。 |
| [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L333) | [§4.1-13](../docs/RFC.md?plain=1#L175) | 每个额外输入管道跨写入拼接字节，读到 EOF 后才生成带编号的输入。 |
| [`test_cancelling_input_does_not_need_eof_or_dispatch_the_partial_message`](../tests/test_shell_pipeline.py#L359) | [§6.5-02](../docs/RFC.md?plain=1#L426) | 读 stdin 或额外输入管道期间取消，写端仍打开也能返回 shell 可见的 130 或 143。 |
| [`test_msh_timeout_signals_the_whole_group_and_grants_two_seconds_before_kill`](../tests/test_shell_pipeline.py#L372) | [§7.1-01](../docs/RFC.md?plain=1#L455) | 独立 msh 显式强杀时向整组发 TERM，2 秒宽限后强制回收并返回 124。 |
| [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L411) | [§6.2-16](../docs/RFC.md?plain=1#L393) | 打断有部分输出的 msh，恢复后没有半条工具结果或悬空调用。 |
| [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L411) | [§6.4-04](../docs/RFC.md?plain=1#L409) | 工具输出一部分后中断，shell 后代收到 TERM、释放 WAL 锁，恢复时没有半截工具结果。 |
| [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) | [§4.3-01](../docs/RFC.md?plain=1#L214) | output=explicit 时真实 msh 子进程的标准输入指向空设备，fd 4、5、6 都是只写匿名管道。 |
| [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) | [§4.3-02](../docs/RFC.md?plain=1#L216) | 多次打开输出管道，超过 1 MiB 的二进制 Payload 逐字节完整转发。 |
| [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) | [§4.3-03](../docs/RFC.md?plain=1#L217) | 诊断管道可独立传输超过 1 MiB 的二进制数据。 |
| [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) | [§4.3-04](../docs/RFC.md?plain=1#L218) | 真实 msh 子进程中 fd 6 是只写匿名管道。 |
| [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) | [§4.1-12](../docs/RFC.md?plain=1#L172) | stdout 与 stderr 各自为终端或普通文件的四种组合，测试 auto 和 explicit。 |
| [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) | [§4.4-02](../docs/RFC.md?plain=1#L228) | stdout/stderr 独立连接到 TTY 或文件，核对 auto 模式诊断字节。 |
| [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) | [§4.4-03](../docs/RFC.md?plain=1#L228) | explicit 模式只交付约定诊断，并在四种 TTY 组合下保持原始输出。 |
| [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) | [§4.1-22](../docs/RFC.md?plain=1#L173) | auto 模式在 stderr 显示思考和 usage，explicit 模式只留下工具发出的诊断。 |
| [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L576) | [§2-01](../docs/RFC.md?plain=1#L62) | PATH 中的同名 msh 可覆盖私有安装位置，显式路径仍优先。 |
| [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L576) | [§3.3-05](../docs/RFC.md?plain=1#L127) | shell 路径或命令名。 |
| [`test_nested_anix_uses_own_model_wal_and_budget_and_inherits_cli_auto`](../tests/test_shell_pipeline.py#L640) | [§3.4-01](../docs/RFC.md?plain=1#L156) | 父进程 CLI 开启 auto 后子进程无需重新审批，子模型及 WAL 仍独立。 |
| [`test_nested_anix_inherits_cli_deny_over_its_own_auto_frontmatter`](../tests/test_shell_pipeline.py#L644) | [§3.4-01](../docs/RFC.md?plain=1#L156) | 父进程的 deny 覆盖子脚本的 auto；子命令没有产生磁盘副作用。 |

## `tests/test_static_release.py`

| 测试 | RFC 原文 | 本测试的预期结果 |
| --- | --- | --- |
| [`test_every_musl_release_executable_has_no_interpreter_or_needed_libraries`](../tests/test_static_release.py#L32) | [§2-05](../docs/RFC.md?plain=1#L53) | 核对五个发行程序均是可执行的 x86_64 ELF，并逐个排除动态解释器和共享库依赖。 |
