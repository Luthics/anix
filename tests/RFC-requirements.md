# v1.1.2 RFC 条款验收表

每行给出一项可观察行为。第一列链接到 RFC 原文，第二列链接到执行断言或待裁决的 issue。通过与失败见对应提交的 CI。

本表按条款逐步补录。[逐测试索引](RFC-cases.md)列出当前套件的全部测试函数。编辑 `tests/rfc-requirements.tsv` 后运行 `python3 scripts/rfc-test-index.py` 更新两份索引。

## §1.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [安装后的 .agent 脚本以 shebang 从 PATH 或相对路径直接启动](../docs/v1.1.2-RFC.md?plain=1#L21) | [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L248) |

## §1.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [旧命令仍可解析，默认超时已从杀组并返回 124 改为返回 0 并移交后台任务](../docs/v1.1.2-RFC.md?plain=1#L37) | [`test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log`](../tests/test_msh_jobs.py#L116) · [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L262) · [#59](https://github.com/Luthics/anix/issues/59) |

## §2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [PATH 中的同名 msh 可覆盖私有安装位置，显式路径仍优先](../docs/v1.1.2-RFC.md?plain=1#L57) | [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L529) |
| [Provider 的命令名依次查找 PATH 和私有目录，显式路径直接执行](../docs/v1.1.2-RFC.md?plain=1#L57) | [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L929) · [#28](https://github.com/Luthics/anix/issues/28) |
| [子进程优先解析私有工具](../docs/v1.1.2-RFC.md?plain=1#L58) | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L149) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L272) |

## §3.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [直接传参、可选分隔符及以短横线开头的参数](../docs/v1.1.2-RFC.md?plain=1#L67) | [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/anix-core/tests/rfc_runtime/io.rs#L186) |
| [trust 子命令与普通脚本参数分别解析](../docs/v1.1.2-RFC.md?plain=1#L68) | [`trust_subcommand_does_not_conflict_with_script_args`](../crates/anix-core/src/cli.rs#L250) |
| [自检与凭据诊断](../docs/v1.1.2-RFC.md?plain=1#L69) | [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L70) · [#31](https://github.com/Luthics/anix/issues/31) |
| [版本与 ir_version](../docs/v1.1.2-RFC.md?plain=1#L70) | [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/anix-core/tests/rfc_runtime/io.rs#L8) |
| [空 context 和显式 WAL 路径](../docs/v1.1.2-RFC.md?plain=1#L76) | [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L431) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L786) |
| [Provider、模型、shell 选择和覆盖](../docs/v1.1.2-RFC.md?plain=1#L77) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) · [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L929) |
| [轮数、上下文、单轮输出和压缩参数](../docs/v1.1.2-RFC.md?plain=1#L79) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) · [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L347) · [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L91) |
| [审批枚举及简写](../docs/v1.1.2-RFC.md?plain=1#L80) | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L571) · [#27](https://github.com/Luthics/anix/issues/27) |
| [重复键、JSON 值、原字符串与格式错误](../docs/v1.1.2-RFC.md?plain=1#L81) | [`cli_params_preserve_json_literals_and_the_exact_fallback_string`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L724) · [`malformed_cli_param_stops_before_provider_or_wal_creation`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L761) · [#27](https://github.com/Luthics/anix/issues/27) |
| [quiet 与显式 stderr 模式](../docs/v1.1.2-RFC.md?plain=1#L82) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L87) · [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L467) · [#27](https://github.com/Luthics/anix/issues/27) |
| [默认闲置时限和有限等待的计时起点](../docs/v1.1.2-RFC.md?plain=1#L85) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) · [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L179) |

## §3.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [UTF-8 字节、截断、非法序列、LF 与 CRLF](../docs/v1.1.2-RFC.md?plain=1#L89) | [`all_256_single_bytes_inside_a_plain_prompt`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L11) · [`valid_multibyte_boundaries_survive_file_decoding`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L37) · [`eof_at_each_interior_byte_of_a_utf8_scalar`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L104) · [`malformed_sequences_are_rejected_in_every_script_region`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L64) · [`layout_matrix_for_line_ending_shebang_header_body_and_eof`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L9) · [`crlf_script_reaches_the_provider_as_the_intended_system_message`](../crates/anix-core/tests/rfc_3_2/cli.rs#L44) |
| [仅去掉首行 shebang](../docs/v1.1.2-RFC.md?plain=1#L90) | [`only_a_first_line_beginning_with_hash_bang_is_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L46) |
| [分隔行、提示词空行及 YAML 1.2](../docs/v1.1.2-RFC.md?plain=1#L91) | [`opening_delimiter_requires_the_designated_line_and_exact_marker`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L69) · [`first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L92) · [`yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L115) · [`yaml_block_scalar_chomping_survives_the_frontmatter_boundary`](../crates/anix-core/tests/rfc_3_2/cli.rs#L107) · [`duplicate_yaml_keys_are_invalid_even_inside_open_maps`](../crates/anix-core/tests/rfc_3_2/cli.rs#L150) · [#1](https://github.com/Luthics/anix/issues/1) · [#23](https://github.com/Luthics/anix/issues/23) · [`incomplete_delimiters_and_malformed_yaml_fail_at_the_header`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L145) |
| [未知字段和 x-* 扩展](../docs/v1.1.2-RFC.md?plain=1#L92) | [`unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position`](../crates/anix-core/tests/rfc_3_2/cli.rs#L67) · [`yaml_aliases_can_supply_typed_provider_params_from_an_extension_field`](../crates/anix-core/tests/rfc_3_2/cli.rs#L163) |

## §3.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [最后一轮完成返回 0，仍需推理时返回 124，均不再请求 Provider](../docs/v1.1.2-RFC.md?plain=1#L122) | [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L234) |
| [显式 name 的字符集，默认文件名含非法字符时的处理待定](../docs/v1.1.2-RFC.md?plain=1#L117) | [`agent_name_has_exactly_the_documented_ascii_alphabet`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L11) · [#26](https://github.com/Luthics/anix/issues/26) |
| [Provider 名必须已定义](../docs/v1.1.2-RFC.md?plain=1#L118) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L34) · [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L516) |
| [model 类型、透传与默认](../docs/v1.1.2-RFC.md?plain=1#L119) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L34) · [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L116) |
| [shell 路径或命令名](../docs/v1.1.2-RFC.md?plain=1#L121) | [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L529) |
| [max_tokens 范围及硬上限退出](../docs/v1.1.2-RFC.md?plain=1#L123) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L34) · [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L459) |
| [单轮输出必须小于上下文预算，0 时省略](../docs/v1.1.2-RFC.md?plain=1#L124) | [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L347) · [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L634) |
| [比例向下取整、绝对阈值与禁用值](../docs/v1.1.2-RFC.md?plain=1#L125) | [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L91) · [`absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L693) · [#5](https://github.com/Luthics/anix/issues/5) · [#24](https://github.com/Luthics/anix/issues/24) |
| [approval 枚举及默认值](../docs/v1.1.2-RFC.md?plain=1#L126) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L34) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) |
| [stderr 枚举及默认值](../docs/v1.1.2-RFC.md?plain=1#L127) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L34) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) |
| [context 按 cwd 解析](../docs/v1.1.2-RFC.md?plain=1#L128) | [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L786) · [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L431) |
| [params 的扩展键与浅合并](../docs/v1.1.2-RFC.md?plain=1#L129) | [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L851) · [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L386) |

## §3.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [嵌套子进程继承实际生效的 auto 或 deny，模型、WAL 和轮数预算独立](../docs/v1.1.2-RFC.md?plain=1#L144) | [`test_nested_anix_uses_own_model_wal_and_budget_and_inherits_cli_auto`](../tests/test_shell_pipeline.py#L593) · [`test_nested_anix_inherits_cli_deny_over_its_own_auto_frontmatter`](../tests/test_shell_pipeline.py#L597) |
| [优先级各层的 32 种子集](../docs/v1.1.2-RFC.md?plain=1#L134) | [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L116) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) |
| [环境变量、布尔别名及非法值](../docs/v1.1.2-RFC.md?plain=1#L140) | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L571) · [`invalid_resolved_environment_values_exit_two_before_contacting_the_provider`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L618) |
| [显式空串覆盖会话路径](../docs/v1.1.2-RFC.md?plain=1#L141) | [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L431) |
| [清除父实例状态、保留观测变量与普通环境变量](../docs/v1.1.2-RFC.md?plain=1#L142) | [`shell_environment_contains_only_observability_effective_approval_and_routed_fds`](../crates/anix-core/tests/rfc_runtime/io.rs#L280) |
| [fd 4–6 的环境映射](../docs/v1.1.2-RFC.md?plain=1#L145) | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L149) |
| [跨配置层保留无冲突键、高层整体替换同名子对象](../docs/v1.1.2-RFC.md?plain=1#L147) | [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L851) · [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L386) |

## §4.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [有参数的可执行脚本通过 PATH 或相对路径启动，TTY 保持打开即可推理](../docs/v1.1.2-RFC.md?plain=1#L159) | [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L248) |
| [无参数且 stdin 为终端时等待实际 EOF，再将输入交给 Provider](../docs/v1.1.2-RFC.md?plain=1#L159) | [`test_executable_script_on_path_and_by_relative_name_obeys_tty_eof_rules`](../tests/test_provider_streaming.py#L443) · [#34](https://github.com/Luthics/anix/issues/34) |
| [带参数启动，Provider 首帧前是否必须显示等待提示](../docs/v1.1.2-RFC.md?plain=1#L159) | [#36](https://github.com/Luthics/anix/issues/36) |
| [重定向与真实管道只转发显式 Payload，保留原始字节](../docs/v1.1.2-RFC.md?plain=1#L160) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L87) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L272) |
| [末尾无换行的模型文本与使用量行分行显示](../docs/v1.1.2-RFC.md?plain=1#L169) | [`trace_fd_explicit_only_no_implicit_fd3_and_cycle_events`](../crates/anix-core/tests/v1_1_e2e.rs#L161) |
| [流式正文和思考帧在工具状态、诊断侧信道及使用量汇总处的换行，显式模式只输出诊断](../docs/v1.1.2-RFC.md?plain=1#L169) | [`unterminated_deltas_are_separated_from_tool_status_diagnostics_and_usage`](../crates/anix-core/tests/rfc_runtime/io.rs#L393) |
| [在真实 TTY 上精确检查 Dim 使用量行在最后一个换行前重置 ANSI 样式](../docs/v1.1.2-RFC.md?plain=1#L170) | [`test_usage_line_resets_dim_before_the_final_newline_on_a_terminal`](../tests/test_distribution.py#L218) |
| [可读写的显式 Trace FD 不进入静态输入](../docs/v1.1.2-RFC.md?plain=1#L163) | [`each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input`](../crates/anix-core/tests/rfc_runtime/fd.rs#L94) |
| [显式 FD 校验失败时先返回 2，有效时 CLI 优先于环境配置](../docs/v1.1.2-RFC.md?plain=1#L162) | [`invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider`](../crates/anix-core/tests/rfc_runtime/fd.rs#L136) · [`only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment`](../crates/anix-core/tests/rfc_runtime/fd.rs#L61) |
| [真实管道有无脚本参数均等到 EOF，保留跨写入的 UTF-8 及内嵌 NUL](../docs/v1.1.2-RFC.md?plain=1#L159) | [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L294) |
| [逐个通过真实管道连接 fd 4 至 9，读完后保留输入来源和完整正文](../docs/v1.1.2-RFC.md?plain=1#L163) | [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L294) |
| [stdout 与 stderr 各自为终端或普通文件的四种组合，测试 auto 和 explicit](../docs/v1.1.2-RFC.md?plain=1#L160) | [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L464) |
| [fd 4–9 访问模式、拼接顺序与 EOF](../docs/v1.1.2-RFC.md?plain=1#L163) | [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L207) · [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L294) |
| [Provider、钩子、shell 不能继承已消费的文件](../docs/v1.1.2-RFC.md?plain=1#L173) | [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L207) |
| [管道输入含非法 UTF-8 时如何构造 JSON 用户消息](../docs/v1.1.2-RFC.md?plain=1#L159) | [#12](https://github.com/Luthics/anix/issues/12) |

## §4.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [当前工具批次闭合后才取下一条已发布消息](../docs/v1.1.2-RFC.md?plain=1#L188) | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L19) |
| [成功消息先提交，下一条被中断时仍可识别已完成的轮次](../docs/v1.1.2-RFC.md?plain=1#L190) | [`test_inbox_commits_one_message_then_interrupts_the_next_in_the_same_coprocess`](../tests/test_provider_streaming.py#L591) · [#37](https://github.com/Luthics/anix/issues/37) |
| [Linux 空闲时只发一次 idle，新任务结束后重新待命](../docs/v1.1.2-RFC.md?plain=1#L193) | [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L336) |
| [新建四个子目录及根目录的权限与就绪时间](../docs/v1.1.2-RFC.md?plain=1#L185) | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L19) · [`a_publisher_can_queue_work_while_the_initial_cli_task_is_running`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L120) · [#21](https://github.com/Luthics/anix/issues/21) |
| [多发送方只发布完整文件](../docs/v1.1.2-RFC.md?plain=1#L186) | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L19) |
| [上一任务结束后才开始计算闲置超时并释放 WAL 锁](../docs/v1.1.2-RFC.md?plain=1#L194) | [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L179) |
| [进程重启时如何处理已领取且仍留在 cur/ 的消息](../docs/v1.1.2-RFC.md?plain=1#L189) | [#2](https://github.com/Luthics/anix/issues/2) |

## §4.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [真实 msh 子进程的标准输入指向空设备，fd 4、5、6 都是只写匿名管道](../docs/v1.1.2-RFC.md?plain=1#L202) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L424) |
| [多次打开输出管道，超过 1 MiB 的二进制 Payload 逐字节完整转发](../docs/v1.1.2-RFC.md?plain=1#L204) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L424) |
| [诊断管道可独立传输超过 1 MiB 的二进制数据](../docs/v1.1.2-RFC.md?plain=1#L205) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L424) |
| [控制管道写端与实际 JSONL 帧](../docs/v1.1.2-RFC.md?plain=1#L206) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L424) · [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L24) |
| [连续工具重定向到普通文件时顺序追加](../docs/v1.1.2-RFC.md?plain=1#L208) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L87) |

## §4.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [同名全局或已信任项目钩子可覆写和屏蔽](../docs/v1.1.2-RFC.md?plain=1#L212) | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L524) |
| [auto 首轮提示中的交付通道和工具，实际 fd 2 可见性](../docs/v1.1.2-RFC.md?plain=1#L214) | [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L467) · [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L464) |
| [explicit 首轮契约、切换模式后的冻结前缀及输出](../docs/v1.1.2-RFC.md?plain=1#L215) | [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L467) · [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L464) |

## §5.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [自动放行和其环境变量别名，外部裁决器不被调用](../docs/v1.1.2-RFC.md?plain=1#L229) | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L571) · [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L319) |
| [deny 保留白名单，拦截时返回工具错误，开启 fail-on-deny 时退出 75](../docs/v1.1.2-RFC.md?plain=1#L230) | [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L319) · [#9](https://github.com/Luthics/anix/issues/9) |
| [Always 对同一可执行程序生效，换进程读取同一 WAL 后需重新审批](../docs/v1.1.2-RFC.md?plain=1#L232) | [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L156) |

## §5.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [外部裁决器读到一行 JSON、真实进程标识及 EOF](../docs/v1.1.2-RFC.md?plain=1#L250) | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L247) |
| [外部返回 0，无控制终端也能执行一次已批准的命令](../docs/v1.1.2-RFC.md?plain=1#L260) | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L247) |
| [外部返回 1，裁剪拒绝理由、为空时给出默认值，继续推理](../docs/v1.1.2-RFC.md?plain=1#L261) | [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L265) |
| [返回 2 后交还终端裁决，无控制终端时退出 75](../docs/v1.1.2-RFC.md?plain=1#L262) | [`test_external_escalation_hands_the_decision_to_the_terminal`](../tests/test_approval.py#L234) · [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L219) |
| [其他非零或超时透出诊断，结束当前轮且不执行工具](../docs/v1.1.2-RFC.md?plain=1#L263) | [`test_external_error_and_timeout_surface_stderr_and_end_the_cycle_with_75`](../tests/test_approval.py#L285) |
| [普通放行、拒绝及被信号中断时记录裁决来源](../docs/v1.1.2-RFC.md?plain=1#L265) | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L247) · [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L265) · [`test_interrupt_during_builtin_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L353) · [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L379) · [#50](https://github.com/Luthics/anix/issues/50) |
| [退出码 2 转交人工、超时或启动失败时如何表示外部退出码](../docs/v1.1.2-RFC.md?plain=1#L265) | [#51](https://github.com/Luthics/anix/issues/51) |

## §5.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [提示与回答走控制终端，管道 stdin 保留为模型输入](../docs/v1.1.2-RFC.md?plain=1#L270) | [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L131) |
| [y 只批准一条，同批下一条可单独拒绝](../docs/v1.1.2-RFC.md?plain=1#L272) | [`test_yes_authorizes_only_one_call_in_a_batch`](../tests/test_approval.py#L206) |
| [n 返回模型错误，支持空理由及 UTF-8 文本](../docs/v1.1.2-RFC.md?plain=1#L273) | [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L131) |
| [a 的授权以可执行程序及 PID 为界](../docs/v1.1.2-RFC.md?plain=1#L274) | [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L156) |
| [e 使用私有临时文件，执行编辑结果并清理，编辑取消或为空时两条命令均未执行](../docs/v1.1.2-RFC.md?plain=1#L275) | [`test_editor_receives_private_original_and_only_the_edited_command_runs`](../tests/test_approval.py#L167) · [`test_failed_or_empty_edit_removes_the_file_and_runs_neither_command`](../tests/test_approval.py#L194) · [#10](https://github.com/Luthics/anix/issues/10) |

## §5.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [无控制终端时打印原命令、记录终止事件并退出 75](../docs/v1.1.2-RFC.md?plain=1#L279) | [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L219) |

## §5.5

| 可验收行为 | 测试与问题 |
| --- | --- |
| [依次覆盖 CLI、环境、脚本、所选 Provider、全局默认和内置默认](../docs/v1.1.2-RFC.md?plain=1#L285) | [`vision_resolves_from_cli_env_script_selected_provider_then_defaults`](../crates/anix-core/tests/rfc_runtime/vision.rs#L10) |
| [无原生视觉且无桥接器时，工具报错且附件不进入恢复的历史](../docs/v1.1.2-RFC.md?plain=1#L290) | [`no_native_vision_without_a_bridge_returns_the_documented_error_through_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L179) |
| [桥接器接收图片信息和 EOF，恢复会话后保留转译描述](../docs/v1.1.2-RFC.md?plain=1#L299) | [`bridge_gets_one_utf8_json_line_then_eof_and_its_description_survives_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L189) |
| [桥接成功的正文边界、空输出及无效 UTF-8 的裁决](../docs/v1.1.2-RFC.md?plain=1#L304) | [#43](https://github.com/Luthics/anix/issues/43) |
| [视觉桥接非零退出或超时成为可恢复的工具错误](../docs/v1.1.2-RFC.md?plain=1#L305) | [`bridge_nonzero_exit_and_timeout_both_return_stderr_as_a_recoverable_tool_error`](../crates/anix-core/tests/rfc_runtime/vision.rs#L216) |
| [上游拒绝媒体时产生专属且不可重试的错误](../docs/v1.1.2-RFC.md?plain=1#L307) | [`test_upstream_media_rejection_is_distinct_from_auth_and_transient_failures`](../tests/test_provider_dialects.py#L215) |

## §6.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [从同批工具的消息锚点折叠时回退到对应 assistant](../docs/v1.1.2-RFC.md?plain=1#L337) | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L321) |
| [折叠时沿用活跃轮次编号，耗尽 uint64 后的行为待裁决](../docs/v1.1.2-RFC.md?plain=1#L319) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) · [#18](https://github.com/Luthics/anix/issues/18) · [`running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L396) |
| [最后存活回答的 usage 与后续字节估算](../docs/v1.1.2-RFC.md?plain=1#L322) | [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L569) · [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L99) · [#5](https://github.com/Luthics/anix/issues/5) · [#20](https://github.com/Luthics/anix/issues/20) |
| [首轮及 checkpoint 后估算、排除压缩器用量](../docs/v1.1.2-RFC.md?plain=1#L323) | [`fresh_window_counts_utf8_bytes_in_groups_of_three`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L443) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L198) · [#5](https://github.com/Luthics/anix/issues/5) |
| [fold 后的水位回落](../docs/v1.1.2-RFC.md?plain=1#L324) | [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L569) |
| [等于硬上限允许、超过后停止](../docs/v1.1.2-RFC.md?plain=1#L325) | [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L459) · [`auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L489) |
| [显式 compact 写入新分节并刷新系统提示词](../docs/v1.1.2-RFC.md?plain=1#L328) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L198) |
| [有效范围、前缀保存、剪去发起调用的轮次](../docs/v1.1.2-RFC.md?plain=1#L329) | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L321) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) · [#3](https://github.com/Luthics/anix/issues/3) |
| [无参 aclr 输出用法并退出 2](../docs/v1.1.2-RFC.md?plain=1#L336) | [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L68) |
| [外部压缩器只收 IR、输出须为非空 UTF-8、故障退出](../docs/v1.1.2-RFC.md?plain=1#L340) | [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L374) |
| [provider、model、提示词文件与内置回退](../docs/v1.1.2-RFC.md?plain=1#L341) | [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L548) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L198) |
| [压缩请求没有工具](../docs/v1.1.2-RFC.md?plain=1#L342) | [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L374) · [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L548) |
| [工具退出 0、7、124 时，当前 IR、落盘 WAL 和重新加载的 IR 都含正确的轮次、退出码与过滤后文本](../docs/v1.1.2-RFC.md?plain=1#L319) | [`post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L585) |
| [外部自动摘要器收到无工具的对齐子窗口，摘要指令在 IR 中的位置待裁定](../docs/v1.1.2-RFC.md?plain=1#L332) | [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L870) · [#42](https://github.com/Luthics/anix/issues/42) |
| [自动摘要失败或为空时保留旧窗口，向模型返回工具错误](../docs/v1.1.2-RFC.md?plain=1#L334) | [`empty_or_failed_auto_summary_keeps_the_subwindow_and_reports_a_tool_error`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L900) |

## §6.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [新建会话在压缩、路径别名与恢复后仍使用原 session_id](../docs/v1.1.2-RFC.md?plain=1#L355) | [`persisted_id_survives_builtin_compaction_checkpoint_alias_and_subsequent_resume`](../crates/anix-core/tests/rfc_runtime/session.rs#L122) |
| [无状态运行每进程生成 UUID v4，经过推理和压缩仍一致](../docs/v1.1.2-RFC.md?plain=1#L355) | [`stateless_runs_get_distinct_uuid_v4s_while_all_provider_calls_in_a_run_share_one`](../crates/anix-core/tests/rfc_runtime/session.rs#L100) · [`empty_context_overrides_a_saved_wal_and_keeps_one_ephemeral_id_through_compaction`](../crates/anix-core/tests/rfc_runtime/session.rs#L349) |
| [旧 WAL 的回退值经路径别名与重启稳定，写入 checkpoint 后随文件迁移](../docs/v1.1.2-RFC.md?plain=1#L355) | [`legacy_wal_uses_a_stable_canonical_path_id_across_aliases_and_restarts`](../crates/anix-core/tests/rfc_runtime/session.rs#L161) · [`first_checkpoint_in_a_legacy_wal_persists_the_fallback_id_even_if_the_wal_moves`](../crates/anix-core/tests/rfc_runtime/session.rs#L313) |
| [最新 checkpoint 提供的标识原样到达 Trace、环境和协进程信封](../docs/v1.1.2-RFC.md?plain=1#L380) | [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L195) |
| [从 session_init 和 checkpoint 恢复，stderr、approval、vision 的八种变更组合只在首次变化时产生一条尾部通知](../docs/v1.1.2-RFC.md?plain=1#L352) | [`each_combination_of_stderr_approval_and_vision_drift_creates_one_tail_update`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L635) |
| [旧 runtime_update 只提供部分字段时，缺失的契约配置要从哪里恢复](../docs/v1.1.2-RFC.md?plain=1#L352) | [#17](https://github.com/Luthics/anix/issues/17) |
| [文件增长受限时保留原有字节，只在尾行完整可回放时继续](../docs/v1.1.2-RFC.md?plain=1#L375) | [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L122) |
| [活跃窗口固定，归档从 1 MiB 增至 32 MiB，读取量维持在误差范围内且 Provider 收到相同字节](../docs/v1.1.2-RFC.md?plain=1#L374) | [`checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L109) · [`test_archived_wal_growth_keeps_resume_reads_bounded_at_the_latest_checkpoint`](../tests/test_shell_pipeline.py#L167) |
| [独占非阻塞锁、冲突时退出并诊断](../docs/v1.1.2-RFC.md?plain=1#L348) | [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L196) |
| [session_init、冻结 system、恢复时复用](../docs/v1.1.2-RFC.md?plain=1#L351) | [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L215) · [`initial_section_interruption_never_dispatches_without_a_frozen_system_message`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L515) · [#19](https://github.com/Luthics/anix/issues/19) |
| [只在全量压缩时重新运行钩子](../docs/v1.1.2-RFC.md?plain=1#L353) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) · [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L321) · [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L438) · [#19](https://github.com/Luthics/anix/issues/19) |
| [WAL 每行提交、短写、五字段累计和跨分节恢复](../docs/v1.1.2-RFC.md?plain=1#L354) | [`wal_events_round_trip`](../crates/anix-proto/src/wal.rs#L101) · [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L122) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) · [#8](https://github.com/Luthics/anix/issues/8) |
| [逐字节截断、损坏时保留原件](../docs/v1.1.2-RFC.md?plain=1#L375) | [`every_partial_byte_write_of_a_final_record_fails_without_modifying_history`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L35) · [`resuming_a_complete_last_object_without_lf_never_joins_two_wal_records`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L59) · [#13](https://github.com/Luthics/anix/issues/13) |
| [多次重叠 fold 的剪枝水位与顺序](../docs/v1.1.2-RFC.md?plain=1#L377) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) · [`overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L137) |
| [被剪枝回答的 usage 仍进入会话总量](../docs/v1.1.2-RFC.md?plain=1#L378) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) |
| [回放剔除未闭合的工具调用，保留已提交的用量](../docs/v1.1.2-RFC.md?plain=1#L379) | [`replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage`](../crates/anix-core/src/wal.rs#L747) · [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L371) |
| [分节版本检查与前缀和](../docs/v1.1.2-RFC.md?plain=1#L380) | [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L173) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) |
| [正序回放与前向参考模型逐轮对照](../docs/v1.1.2-RFC.md?plain=1#L381) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) |

## §6.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [无截止时间的 Linux 空闲循环持续睡眠，发布消息可以唤醒](../docs/v1.1.2-RFC.md?plain=1#L385) | [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L336) |
| [子进程清理与退出钩子的截止时间在正常和缺少 pidfd 的 Linux 上可检查](../docs/v1.1.2-RFC.md?plain=1#L389) | [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L176) · [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L155) · [#48](https://github.com/Luthics/anix/issues/48) |
| [强杀时组长退出、有限子进程仍在，Linux 宽限期的等待不应持续空转](../docs/v1.1.2-RFC.md?plain=1#L385) | [`test_hard_kill_grace_waits_for_a_living_descendant_without_repeated_polling`](../tests/test_msh_jobs.py#L372) · [#57](https://github.com/Luthics/anix/issues/57) |

## §6.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [Ctrl+C 在等待 Provider 或其关闭 stdout 后回到可继续执行管道的同一 shell](../docs/v1.1.2-RFC.md?plain=1#L395) | [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L493) · [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L550) · [#35](https://github.com/Luthics/anix/issues/35) |
| [在内置审批处只按一次 Ctrl+C，命令不执行，原 shell 恢复并读到退出码 130](../docs/v1.1.2-RFC.md?plain=1#L395) | [`test_ctrl_c_at_the_approval_prompt_returns_to_the_same_shell`](../tests/test_approval.py#L345) · [#49](https://github.com/Luthics/anix/issues/49) |
| [外部审批中收到 SIGTERM，退出 143 并记录中断](../docs/v1.1.2-RFC.md?plain=1#L396) | [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L379) · [#50](https://github.com/Luthics/anix/issues/50) |
| [工具输出一部分后中断，shell 后代收到 TERM、释放 WAL 锁，恢复时没有半截工具结果](../docs/v1.1.2-RFC.md?plain=1#L395) | [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L371) |
| [保持模式进程收到 STOP 后暂停，CONT 恢复后继续响应快照请求](../docs/v1.1.2-RFC.md?plain=1#L397) | [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L50) |
| [USR1 在下次推理前执行一次无工具的上下文压缩](../docs/v1.1.2-RFC.md?plain=1#L398) | [`usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference`](../crates/anix-core/tests/rfc_runtime/signals.rs#L117) |
| [USR2 输出快照路径，文件权限为 0600，内容与活跃 IR 相符](../docs/v1.1.2-RFC.md?plain=1#L399) | [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L50) |

## §6.5

| 可验收行为 | 测试与问题 |
| --- | --- |
| [显式强杀超时返回模型错误，长时间运行的命令本身不耗掉下一次 Provider 请求的时限](../docs/v1.1.2-RFC.md?plain=1#L411) | [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L262) · [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L230) |
| [读 stdin 或额外输入管道期间取消，写端仍打开也能返回 shell 可见的 130 或 143](../docs/v1.1.2-RFC.md?plain=1#L412) | [`test_cancelling_input_does_not_need_eof_or_dispatch_the_partial_message`](../tests/test_shell_pipeline.py#L320) |
| [正常结束返回 0，普通文件中保留完整的显式输出](../docs/v1.1.2-RFC.md?plain=1#L407) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L87) |
| [Provider 提前 EOF、WAL 文件增长失败和锁冲突分别以 1 结束](../docs/v1.1.2-RFC.md?plain=1#L408) | [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/anix-core/tests/rfc_runtime/provider.rs#L147) · [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L122) · [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L196) |
| [脚本编码错误、损坏的配置及不支持的 WAL 版本在推理前以 2 结束](../docs/v1.1.2-RFC.md?plain=1#L409) | [`invalid_file_bytes_and_yaml_report_usage_errors_to_stderr`](../crates/anix-core/tests/rfc_3_2/cli.rs#L90) · [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L646) · [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L173) |
| [无控制终端需人工审批及未信任的项目裁决器以 75 结束](../docs/v1.1.2-RFC.md?plain=1#L410) | [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L219) · [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L293) |
| [最后一轮结束仍有工作、首轮超过硬上限或 Provider 超时均以 124 结束](../docs/v1.1.2-RFC.md?plain=1#L411) | [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L234) · [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L459) · [`provider_timeout_maps_to_124_without_a_retry`](../crates/anix-core/tests/rfc_runtime/provider.rs#L158) |
| [默认超时返回后台句柄并继续任务，显式强杀返回工具错误并继续推理](../docs/v1.1.2-RFC.md?plain=1#L411) | [`test_v1_1_2_msh_per_call_timeout_yield_bg_notification_and_kill_on_timeout`](../crates/anix-core/tests/v1_1_e2e.rs#L1029) |

## §7.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [独立 msh 显式强杀时向整组发 TERM，2 秒宽限后强制回收并返回 124](../docs/v1.1.2-RFC.md?plain=1#L441) | [`test_msh_timeout_signals_the_whole_group_and_grants_two_seconds_before_kill`](../tests/test_shell_pipeline.py#L333) |
| [六个变量强制生效、工具目录进入 PATH](../docs/v1.1.2-RFC.md?plain=1#L422) | [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L219) · [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L149) |
| [8 KiB NUL 边界与全流 UTF-8 校验](../docs/v1.1.2-RFC.md?plain=1#L423) | [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L34) · [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L241) |
| [探测 MIME、保存原始字节、路径和 0600](../docs/v1.1.2-RFC.md?plain=1#L425) | [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L241) · [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L34) |
| [二进制输出单行说明](../docs/v1.1.2-RFC.md?plain=1#L426) | [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L241) |
| [CSI 终结字节、OSC 终止方式和回车覆盖](../docs/v1.1.2-RFC.md?plain=1#L427) | [`ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites`](../crates/msh/tests/rfc_shell.rs#L65) |
| [默认双阈值、环境覆盖、首尾 20 行和完整清洗日志](../docs/v1.1.2-RFC.md?plain=1#L428) | [`unconfigured_msh_folds_only_above_200_lines_or_8192_bytes`](../crates/msh/tests/rfc_shell.rs#L280) · [`cli_applies_environment_limits_to_cleaned_lines_and_bytes_before_saving_output`](../crates/msh/tests/rfc_shell.rs#L177) · [`line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text`](../crates/msh/tests/rfc_shell.rs#L106) · [#15](https://github.com/Luthics/anix/issues/15) · [`byte_limit_independent_from_line_limit_preserves_multibyte_file`](../crates/msh/tests/rfc_shell.rs#L142) |
| [普通命令退出 7 时，用于 Provider 的 is_error 布尔值是否有规定](../docs/v1.1.2-RFC.md?plain=1#L432) | [#55](https://github.com/Luthics/anix/issues/55) |
| [独立 msh 的环境变量配置前台等待时长及显式强杀](../docs/v1.1.2-RFC.md?plain=1#L430) | [`timeout_reports_124_before_a_finite_longer_command_completes`](../crates/msh/tests/rfc_shell.rs#L266) |
| [单次请求依次覆盖缺省、零、正数、上限两侧与回到默认，自定义 shell 收到 -c、原命令、策略变量及任务目录](../docs/v1.1.2-RFC.md?plain=1#L430) | [`test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call`](../tests/test_msh_jobs.py#L252) |
| [前台成功与普通非零退出保留双流输出、透传真实退出码并清理任务文件](../docs/v1.1.2-RFC.md?plain=1#L432) | [`test_foreground_completion_returns_real_status_and_removes_job_files`](../tests/test_msh_jobs.py#L140) |
| [立即后台启动时 fd 4–6 和三个侧信道变量均已关闭或清除，进程组和会话隔离](../docs/v1.1.2-RFC.md?plain=1#L434) | [`test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit`](../tests/test_msh_jobs.py#L87) |
| [真实命令等待到期时仅返回清洗后的有限预览，后台继续执行并保留完整原始日志](../docs/v1.1.2-RFC.md?plain=1#L435) | [`test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log`](../tests/test_msh_jobs.py#L116) |
| [运行时目录下 jobs 为 0700、日志为 0600，后台退出前不存在最终状态文件，随后读到真实的非零退出码](../docs/v1.1.2-RFC.md?plain=1#L431) | [`test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit`](../tests/test_msh_jobs.py#L87) |
| [显式强杀且指定 0 秒时先等待一秒，输出已有文本与超时标记，清理任务文件](../docs/v1.1.2-RFC.md?plain=1#L441) | [`test_zero_with_explicit_kill_waits_one_second_and_leaves_no_job`](../tests/test_msh_jobs.py#L180) |
| [前台等待中分别发送 INT、TERM，两种超时策略都清理整组并返回 130、143](../docs/v1.1.2-RFC.md?plain=1#L442) | [`test_foreground_int_and_term_clean_up_in_both_timeout_modes`](../tests/test_msh_jobs.py#L149) |

## §7.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [省略字段时工具发出无 summary 的 fold 帧，运行时生成摘要](../docs/v1.1.2-RFC.md?plain=1#L460) | [`aclr_without_a_summary_omits_the_field_at_each_valid_turn_id_boundary`](../crates/anix-tools/tests/rfc_control.rs#L58) · [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L870) · [#41](https://github.com/Luthics/anix/issues/41) |
| [按含 LF 的实际序列化长度逐个比较 aclr 发送端和内核接收端的判定](../docs/v1.1.2-RFC.md?plain=1#L447) | [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L72) · [#14](https://github.com/Luthics/anix/issues/14) |
| [四页文档的 15 种非空页集合，从文件、显式和隐式 stdin 输入，逐页核对 PNG 尺寸及色块](../docs/v1.1.2-RFC.md?plain=1#L453) | [`aview_selects_pdf_pages_with_their_actual_geometry_and_colors`](../crates/anix-tools/tests/rfc_control.rs#L236) |
| [页码语法错误时以非零状态退出且不发布附件](../docs/v1.1.2-RFC.md?plain=1#L453) | [`aview_rejects_invalid_pdf_page_expressions_without_publishing_attachments`](../crates/anix-tools/tests/rfc_control.rs#L305) |
| [重复或乱序页码的输出次序，以及合法页与越界页混用时能否先发布部分附件](../docs/v1.1.2-RFC.md?plain=1#L453) | [#54](https://github.com/Luthics/anix/issues/54) |
| [原生工具和图片管道的字节一致性](../docs/v1.1.2-RFC.md?plain=1#L446) | [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L272) |
| [fd 6 的 JSONL 跨读取边界与多帧顺序、64 KB 边界](../docs/v1.1.2-RFC.md?plain=1#L447) | [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L24) · [`two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order`](../crates/anix-core/tests/rfc_runtime/control.rs#L93) · [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L72) · [#14](https://github.com/Luthics/anix/issues/14) · [`fold_control_frames_stay_within_64_kib_even_after_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L87) · [`ctl_frames_round_trip`](../crates/anix-proto/src/ctl.rs#L46) · [#40](https://github.com/Luthics/anix/issues/40) |
| [不同方向与宽高比图片的尺寸、内容、路径和权限](../docs/v1.1.2-RFC.md?plain=1#L453) | [`aview_resizes_portrait_landscape_and_square_images_only_when_needed`](../crates/anix-tools/tests/rfc_control.rs#L139) · [`aview_preserves_image_content_and_orientation_in_the_published_png`](../crates/anix-tools/tests/rfc_control.rs#L177) · [`aview_file_input_and_stdin_agree_on_image_dimensions`](../crates/anix-tools/tests/rfc_control.rs#L162) · [`undecodable_input_or_invalid_options_do_not_emit_media`](../crates/anix-tools/tests/rfc_control.rs#L221) |
| [compact 和 fold 的字段与序列化](../docs/v1.1.2-RFC.md?plain=1#L454) | [`aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L41) · [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L68) |
| [511 至 66,000 字节的命令都执行到末尾，fd 6 只产生一条有效且有界的 JSONL 帧](../docs/v1.1.2-RFC.md?plain=1#L461) | [`test_long_commands_execute_their_suffix_and_handoff_with_a_single_bounded_frame`](../tests/test_msh_jobs.py#L191) |
| [长命令在移交帧和 Trace 的 cmd 字段中是否可以缩短、如何标识预览](../docs/v1.1.2-RFC.md?plain=1#L461) | [#58](https://github.com/Luthics/anix/issues/58) |
| [后台任务结束后仅在下一次请求前通知一次，冻结前缀、WAL 和重启恢复结果可核对](../docs/v1.1.2-RFC.md?plain=1#L465) | [`test_background_completion_is_reported_once_before_inference_and_replays_from_wal`](../tests/test_msh_jobs.py#L280) |

## §8.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [XDG_CONFIG_HOME 与 HOME 回退](../docs/v1.1.2-RFC.md?plain=1#L473) | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L995) |
| [XDG_STATE_HOME 与 HOME 回退](../docs/v1.1.2-RFC.md?plain=1#L474) | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L995) |
| [XDG_RUNTIME_DIR、按 UID 回退及权限](../docs/v1.1.2-RFC.md?plain=1#L475) | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L995) · [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L99) · [`each_missing_or_non_executable_private_binary_is_identified`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L31) |
| [从 cwd 向上寻找最近的 .anix，并在 .git 或 HOME 边界停止](../docs/v1.1.2-RFC.md?plain=1#L476) | [`project_search_stops_at_nearest_configuration_git_or_home_boundary`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L465) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L786) |

## §8.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [所选 Provider 的 params 浅合并](../docs/v1.1.2-RFC.md?plain=1#L517) | [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L851) |
| [内置及外部压缩器选取](../docs/v1.1.2-RFC.md?plain=1#L520) | [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L548) · [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L374) |
| [只将硬上限配置为 2 时，每个显式请求仍应裁剪到 2，即使默认等待为 120](../docs/v1.1.2-RFC.md?plain=1#L491) | [`test_single_call_never_exceeds_a_configured_maximum_below_the_default_wait`](../tests/test_msh_jobs.py#L267) · [#56](https://github.com/Luthics/anix/issues/56) |
| [环境覆盖 TOML 的默认等待与单次上限，单次覆盖后恢复默认等待](../docs/v1.1.2-RFC.md?plain=1#L490) | [`test_environment_maximum_overrides_toml_for_each_explicit_request`](../tests/test_msh_jobs.py#L273) |

## §8.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [三个内置 pre-context 钩子依次由全局及项目同名文件覆写，也可被空文件或空设备软链屏蔽](../docs/v1.1.2-RFC.md?plain=1#L533) | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L524) |
| [项目级裁决器首次运行需取得信任，再次启动按裁决处理工具](../docs/v1.1.2-RFC.md?plain=1#L535) | [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L293) |

## §9.1

| 可验收行为 | 测试与问题 |
| --- | --- |
| [IR 不含 model，oneshot 由环境注入，coprocess 改用请求信封](../docs/v1.1.2-RFC.md?plain=1#L557) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L36) · [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L516) · [`selected_model_changes_the_provider_environment_without_entering_the_ir`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1054) · [`coprocess_envelope_wraps_the_same_ir_without_moving_model_into_history`](../crates/anix-proto/tests/rfc_ir.rs#L297) |
| [扁平消息字段、附件引用、按行解析](../docs/v1.1.2-RFC.md?plain=1#L558) | [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L156) · [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L223) |
| [header 必须位于首行，必填字段及其类型受约束](../docs/v1.1.2-RFC.md?plain=1#L561) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L36) · [`ir_jsonl_header_and_messages_round_trip`](../crates/anix-proto/src/ir.rs#L372) |
| [不支持的 IR 版本在 HTTP 请求前被检测](../docs/v1.1.2-RFC.md?plain=1#L560) | [`test_unsupported_ir_version_is_diagnosed_before_http_in_both_modes`](../tests/test_provider_dialects.py#L271) · [#44](https://github.com/Luthics/anix/issues/44) |
| [IR 中 tools 必填，Chat Completions 请求省略空工具集](../docs/v1.1.2-RFC.md?plain=1#L562) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L36) · [`omits_tools_when_empty_and_supports_disabling_stream_options`](../crates/anix-providers/src/openai_compat.rs#L983) |
| [IR 输出上限的省略和显式设置，以及上游 Responses 映射](../docs/v1.1.2-RFC.md?plain=1#L563) | [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L347) · [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L634) |
| [params 的 wire 类型及 Responses 参数映射](../docs/v1.1.2-RFC.md?plain=1#L564) | [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L68) · [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L634) |
| [meta 的 cwd、stderr_mode、预算](../docs/v1.1.2-RFC.md?plain=1#L565) | [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L68) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L786) |
| [扩展字段的解码及钩子透传](../docs/v1.1.2-RFC.md?plain=1#L566) | [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L156) · [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L10) |
| [首条系统文本字节保真与尾部系统消息映射](../docs/v1.1.2-RFC.md?plain=1#L568) | [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L111) |
| [user 必需字段及可选媒体](../docs/v1.1.2-RFC.md?plain=1#L569) | [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L223) · [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L147) |
| [assistant 文本或工具调用非空，opaque 与 arguments 类型](../docs/v1.1.2-RFC.md?plain=1#L570) | [`assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state`](../crates/anix-proto/tests/rfc_ir.rs#L258) · [`validated_ir_only_serializes_objects_for_arguments_and_provider_opaque`](../crates/anix-proto/tests/rfc_ir.rs#L103) |
| [tool 标识、结果和附件](../docs/v1.1.2-RFC.md?plain=1#L571) | [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L223) · [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L185) · [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L24) |
| [四种有效媒体类型均从磁盘读取，无效媒体引用的报错方式待裁决](../docs/v1.1.2-RFC.md?plain=1#L573) | [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L147) · [#16](https://github.com/Luthics/anix/issues/16) |
| [请求前读取媒体，缺失或不可读时永久失败](../docs/v1.1.2-RFC.md?plain=1#L574) | [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L272) · [`test_unsupported_ir_version_is_diagnosed_before_http_in_both_modes`](../tests/test_provider_dialects.py#L271) · [`test_oneshot_unreadable_and_missing_paths_fail_before_dispatch`](../tests/test_provider_dialects.py#L197) · [`test_coprocess_reopens_replaced_media_and_recovers_after_unlink`](../tests/test_provider_dialects.py#L174) |
| [checkpoint 映射为新冻结提示词与 user 摘要](../docs/v1.1.2-RFC.md?plain=1#L576) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) · [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L438) |
| [fold 对齐后插入 user 摘要](../docs/v1.1.2-RFC.md?plain=1#L577) | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L321) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) |
| [尾部 runtime_update 及上游角色映射](../docs/v1.1.2-RFC.md?plain=1#L578) | [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L215) · [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L111) |
| [header、工具一一配对且连续、编号顺序与共享锚点](../docs/v1.1.2-RFC.md?plain=1#L579) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L36) · [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L185) · [`a_single_tool_result_cannot_close_two_calls_with_the_same_id`](../crates/anix-proto/tests/rfc_ir.rs#L213) · [`ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared`](../crates/anix-proto/tests/rfc_ir.rs#L277) · [#39](https://github.com/Luthics/anix/issues/39) |
| [msh 的工具声明仅要求 cmd，timeout_sec 为非负整数，kill_on_timeout 为可选布尔值](../docs/v1.1.2-RFC.md?plain=1#L584) | [`test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call`](../tests/test_msh_jobs.py#L252) |

## §9.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [父子会话各自路由，子进程完成后父会话保持原标识](../docs/v1.1.2-RFC.md?plain=1#L597) | [`nested_anix_gets_its_own_id_and_the_parent_resumes_with_its_original_id`](../crates/anix-core/tests/rfc_runtime/session.rs#L224) |
| [每帧路由覆盖只在本轮生效，省略时回到环境默认](../docs/v1.1.2-RFC.md?plain=1#L609) | [`test_session_routing_works_in_oneshot_and_per_envelope_in_coprocess`](../tests/test_provider_dialects.py#L407) · [`test_coprocess_overrides_apply_to_one_frame_then_environment_defaults_resume`](../tests/test_provider_streaming.py#L102) |
| [显式空串和含首尾空白的路由标识如何处理](../docs/v1.1.2-RFC.md?plain=1#L614) | [#47](https://github.com/Luthics/anix/issues/47) |
| [三种方言的默认路由头、改名和逐请求禁用](../docs/v1.1.2-RFC.md?plain=1#L616) | [`test_header_rename_and_disable_apply_only_to_the_current_request`](../tests/test_provider_dialects.py#L433) · [`session_affinity_headers_and_body_mapping_across_all_dialects`](../crates/anix-providers/tests/e2e_mock.rs#L672) |
| [Responses 原生缓存键支持默认、字符串、布尔值及 null](../docs/v1.1.2-RFC.md?plain=1#L617) | [`test_responses_cache_key_has_native_string_boolean_and_null_semantics`](../tests/test_provider_dialects.py#L456) |
| [兼容 Chat 端点的两个 Body 路由字段分别按需启用](../docs/v1.1.2-RFC.md?plain=1#L618) | [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L474) |
| [Anthropic 只发送路由头，Body 保持协议格式](../docs/v1.1.2-RFC.md?plain=1#L619) | [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L474) |
| [慢速流入工具参数与已经开始执行的长命令位于不同的计时阶段](../docs/v1.1.2-RFC.md?plain=1#L645) | [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L291) · [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L230) · [#33](https://github.com/Luthics/anix/issues/33) |
| [有效 model、base URL、timeout 与密钥来源](../docs/v1.1.2-RFC.md?plain=1#L597) | [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L516) · [#4](https://github.com/Luthics/anix/issues/4) |
| [WAL、Trace、on-exit 五字段一致，请求 IR 剥离用量](../docs/v1.1.2-RFC.md?plain=1#L639) | [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L30) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L198) |
| [总输入输出包含缓存与推理明细](../docs/v1.1.2-RFC.md?plain=1#L640) | [`test_inclusive_usage_preserves_wide_counts_and_available_cache_details`](../tests/test_provider_dialects.py#L329) · [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L30) |
| [可选明细、uint64 类型及零值处理](../docs/v1.1.2-RFC.md?plain=1#L641) | [`each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values`](../crates/anix-proto/tests/rfc_provider.rs#L7) · [`usage_requires_both_totals_even_when_all_three_details_are_present`](../crates/anix-proto/tests/rfc_provider.rs#L56) · [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L99) · [#22](https://github.com/Luthics/anix/issues/22) |
| [run 含压缩，区分跨会话历史消耗](../docs/v1.1.2-RFC.md?plain=1#L642) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) · [`retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L139) · [#8](https://github.com/Luthics/anix/issues/8) |
| [同 Provider 原样回传，切换时清除](../docs/v1.1.2-RFC.md?plain=1#L643) | [`opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch`](../crates/anix-core/tests/rfc_runtime/provider.rs#L164) · [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L350) · [#6](https://github.com/Luthics/anix/issues/6) |
| [HTTP 暂态错误分类、核心三次退避与可观测事件](../docs/v1.1.2-RFC.md?plain=1#L644) | [`http_429_and_529_map_to_retryable_errors_and_400_is_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L393) · [`retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr`](../crates/anix-core/tests/rfc_runtime/provider.rs#L20) · [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L409) · [#45](https://github.com/Luthics/anix/issues/45) |

## §9.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [pre-context 连续过滤 IR，失败向 fd 2 报告并停止](../docs/v1.1.2-RFC.md?plain=1#L649) | [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L10) · [`pre_context_failure_forwards_stderr_and_stops_before_inference`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L81) |
| [两个成功的 post-tool 钩子串联改写结果并持久化，钩子失败则中止后续工具](../docs/v1.1.2-RFC.md?plain=1#L650) | [`post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L585) · [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L107) |
| [on-exit 用量、退出状态、5 秒超时与错误隔离](../docs/v1.1.2-RFC.md?plain=1#L651) | [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L198) · [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L155) |

## §9.4

| 可验收行为 | 测试与问题 |
| --- | --- |
| [模型从真实 XML 索引读取含特殊字符的技能路径，经 msh 按需获取完整正文](../docs/v1.1.2-RFC.md?plain=1#L664) | [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L221) |
| [20-skills 内置、同名覆写与禁用](../docs/v1.1.2-RFC.md?plain=1#L655) | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L524) |
| [六层搜索顺序和同名覆盖](../docs/v1.1.2-RFC.md?plain=1#L657) | [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L195) |
| [SKILL.md 生成转义 XML 元数据，正文不进首轮](../docs/v1.1.2-RFC.md?plain=1#L658) | [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L221) · [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L195) |

## §9.5

| 可验收行为 | 测试与问题 |
| --- | --- |
| [采集端在第一条记录前或期间断开，Payload、stderr 和退出码保持一致](../docs/v1.1.2-RFC.md?plain=1#L671) | [`disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status`](../crates/anix-core/tests/rfc_runtime/trace.rs#L237) |
| [消费者保持连接但停止读取时的背压及恢复语义](../docs/v1.1.2-RFC.md?plain=1#L671) | [#11](https://github.com/Luthics/anix/issues/11) |
| [初始 Trace 与 WAL、Provider 环境和协进程信封可用同一标识关联](../docs/v1.1.2-RFC.md?plain=1#L678) | [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L195) |
| [模型只看清洗后文本，私有 wire 文件保留命令实际输出的 ANSI 字节](../docs/v1.1.2-RFC.md?plain=1#L689) | [`test_wire_trace_keeps_raw_msh_output_while_the_model_receives_cleaned_text`](../tests/test_provider_streaming.py#L198) · [#52](https://github.com/Luthics/anix/issues/52) |
| [协进程的 error 帧和 stderr 异步抵达时，诊断文本如何划分到对应请求](../docs/v1.1.2-RFC.md?plain=1#L687) | [#38](https://github.com/Luthics/anix/issues/38) |
| [Trace 通用 error 事件的适用故障范围待明确](../docs/v1.1.2-RFC.md?plain=1#L677) | [#32](https://github.com/Luthics/anix/issues/32) |
| [检查已终止的任务组且没有退出码文件时，完成事件含显式 null，后续推理不重复通知](../docs/v1.1.2-RFC.md?plain=1#L684) | [`test_finished_group_without_an_exit_file_reports_null_exactly_once`](../tests/test_msh_jobs.py#L328) |
| [started 和 finished 通过相同任务标识、PID、进程组和日志路径关联，并报告真实状态及经过的毫秒数](../docs/v1.1.2-RFC.md?plain=1#L684) | [`test_background_completion_is_reported_once_before_inference_and_replays_from_wal`](../tests/test_msh_jobs.py#L280) |

## §9.6

| 可验收行为 | 测试与问题 |
| --- | --- |
| [MCP 指南按 anix 状态目录、全局和项目路径披露](../docs/v1.1.2-RFC.md?plain=1#L697) | [`mcp_guide_follows_anix_xdg_and_project_precedence_without_disclosing_tool_schemas`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L376) |
| [首次释放、版本更新和状态目录不可写时回退](../docs/v1.1.2-RFC.md?plain=1#L697) | [`tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L258) · [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L297) |
| [整块覆写、两类禁用及磁盘副作用](../docs/v1.1.2-RFC.md?plain=1#L712) | [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L297) · [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L524) |

## §10.2

| 可验收行为 | 测试与问题 |
| --- | --- |
| [Linux 上主进程异常终止后，活跃的 Provider 或 shell 子进程也终止](../docs/v1.1.2-RFC.md?plain=1#L736) | [`abrupt_parent_exit_stops_the_active_provider_or_shell_child`](../crates/anix-core/tests/rfc_runtime/signals.rs#L220) |
| [缺少 pidfd 时，等待处理了 SIGTERM 的 Provider 和退出钩子仍须遵守各自期限](../docs/v1.1.2-RFC.md?plain=1#L736) | [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L176) · [`sigterm_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L170) · [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L155) · [#48](https://github.com/Luthics/anix/issues/48) |
| [探测内核 ABI 与可用性，启用后的写入范围与不支持时的 strict 行为仍待专项验证](../docs/v1.1.2-RFC.md?plain=1#L737) | [`probes_platform_capabilities`](../crates/anix-core/src/platform/mod.rs#L119) · 待补专项断言 |

## §10.3

| 可验收行为 | 测试与问题 |
| --- | --- |
| [全新 HOME 的安装布局、rc 文件、保留用户修改及安装后启动](../docs/v1.1.2-RFC.md?plain=1#L741) | [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L121) · [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L248) |
| [同时缺少 msh 且全局 TOML 损坏时先报告第 1 步](../docs/v1.1.2-RFC.md?plain=1#L743) | [`doctor_reports_missing_msh_before_a_malformed_global_config`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L154) · [#30](https://github.com/Luthics/anix/issues/30) |
| [逐个移除私有二进制或其执行权限，doctor 指出故障路径](../docs/v1.1.2-RFC.md?plain=1#L745) | [`each_missing_or_non_executable_private_binary_is_identified`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L31) |
| [免鉴权配置、密钥回退与项目级默认 Provider 的诊断](../docs/v1.1.2-RFC.md?plain=1#L746) | [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L70) · [`doctor_checks_the_selected_project_provider_and_its_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L122) |
| [doctor 发现无法创建匿名管道并给出修复提示](../docs/v1.1.2-RFC.md?plain=1#L747) | [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L164) |
| [doctor 检测锁操作失败或失去互斥性](../docs/v1.1.2-RFC.md?plain=1#L748) | [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L164) |
| [发布安装器的版本选择和验证来源](../docs/v1.1.2-RFC.md?plain=1#L741) | [#7](https://github.com/Luthics/anix/issues/7) |
| [默认 Provider 位于用户 PATH 时，doctor 应检查哪些实际可执行文件](../docs/v1.1.2-RFC.md?plain=1#L745) | [#25](https://github.com/Luthics/anix/issues/25) |
