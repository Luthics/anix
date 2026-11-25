# RFC 条款验收表

每行链接到规范原句、可复跑的断言或待裁决的 issue。测试链接表示已有用例，执行结果见对应提交的 CI。
§1.1 约定关键词，§10.1 给出部署建议，§11 为非规范性扩展。§10.2 的 WSL2 验证暂缓。

维护时编辑 `tests/rfc-requirements.tsv`，再运行 `python3 scripts/rfc-test-index.py`。

## §1.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [安装后直接通过 shebang 执行](../docs/v1.0.0-RFC.md?plain=1#L19) | [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) · [`executable_script_on_path_reads_tty_until_eof_only_without_arguments`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L205) |
| [核心依赖与进程边界（架构审阅）](../docs/v1.0.0-RFC.md?plain=1#L20) | [`crates/os-agent-core/Cargo.toml`](../crates/os-agent-core/Cargo.toml) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) |

## §2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [五个独立产物、私有安装目录和 Linux 静态链接](../docs/v1.0.0-RFC.md?plain=1#L25) | [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L90) · [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) |
| [Provider 与 shell 的 PATH、libexec 及显式路径](../docs/v1.0.0-RFC.md?plain=1#L34) | [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L928) · [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L554) · [#28](https://github.com/Luthics/os-agent/issues/28) |
| [子进程优先解析私有工具](../docs/v1.0.0-RFC.md?plain=1#L35) | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L149) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) |

## §3.1

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [trust 子命令与脚本参数](../docs/v1.0.0-RFC.md?plain=1#L43) | [`trust_subcommand_does_not_conflict_with_script_args`](../crates/os-agent-core/src/cli.rs#L238) · [`new_and_modified_project_hooks_require_explicit_trust`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L201) |
| [自检与凭据诊断](../docs/v1.0.0-RFC.md?plain=1#L44) | [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L70) · [#31](https://github.com/Luthics/os-agent/issues/31) |
| [版本与 ir_version](../docs/v1.0.0-RFC.md?plain=1#L45) | [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L8) |
| [分隔符、空白和选项形状的脚本参数](../docs/v1.0.0-RFC.md?plain=1#L48) | [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L186) · [`parses_script_and_args_with_double_dash`](../crates/os-agent-core/src/cli.rs#L197) · [`parses_direct_and_quoted_script_args_without_double_dash`](../crates/os-agent-core/src/cli.rs#L215) |
| [空 context 和显式 WAL 路径](../docs/v1.0.0-RFC.md?plain=1#L51) | [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L431) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L786) |
| [Provider、模型、shell 选择和覆盖](../docs/v1.0.0-RFC.md?plain=1#L52) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) · [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L928) |
| [轮数、上下文、单轮输出和压缩参数](../docs/v1.0.0-RFC.md?plain=1#L53) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) · [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L347) · [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L91) |
| [审批枚举及简写](../docs/v1.0.0-RFC.md?plain=1#L54) | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L574) · [#27](https://github.com/Luthics/os-agent/issues/27) |
| [重复键、JSON 值、原字符串与格式错误](../docs/v1.0.0-RFC.md?plain=1#L55) | [`cli_params_preserve_json_literals_and_the_exact_fallback_string`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L724) · [`malformed_cli_param_stops_before_provider_or_wal_creation`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L761) · [#27](https://github.com/Luthics/os-agent/issues/27) |
| [quiet 与显式 stderr 模式](../docs/v1.0.0-RFC.md?plain=1#L56) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L87) · [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L432) · [#27](https://github.com/Luthics/os-agent/issues/27) |
| [常驻模式](../docs/v1.0.0-RFC.md?plain=1#L57) | [`inbox_observes_only_published_files_in_ascii_order_and_moves_them_after_commit`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L11) |
| [默认 300 秒、有限等待和零值](../docs/v1.0.0-RFC.md?plain=1#L58) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) · [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L188) · [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L50) |

## §3.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [UTF-8 字节、截断、非法序列、LF 与 CRLF](../docs/v1.0.0-RFC.md?plain=1#L62) | [`all_256_single_bytes_inside_a_plain_prompt`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L11) · [`valid_multibyte_boundaries_survive_file_decoding`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L37) · [`eof_at_each_interior_byte_of_a_utf8_scalar`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L104) · [`malformed_sequences_are_rejected_in_every_script_region`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L64) · [`layout_matrix_for_line_ending_shebang_header_body_and_eof`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L9) · [`crlf_script_reaches_the_provider_as_the_intended_system_message`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L44) |
| [仅去掉首行 shebang](../docs/v1.0.0-RFC.md?plain=1#L63) | [`only_a_first_line_beginning_with_hash_bang_is_removed`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L46) |
| [分隔行、提示词空行及 YAML 1.2](../docs/v1.0.0-RFC.md?plain=1#L64) | [`opening_delimiter_requires_the_designated_line_and_exact_marker`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L69) · [`first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L92) · [`yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L115) · [`yaml_block_scalar_chomping_survives_the_frontmatter_boundary`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L107) · [`duplicate_yaml_keys_are_invalid_even_inside_open_maps`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L150) · [#1](https://github.com/Luthics/os-agent/issues/1) · [#23](https://github.com/Luthics/os-agent/issues/23) · [`incomplete_delimiters_and_malformed_yaml_fail_at_the_header`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L145) |
| [未知字段和 x-* 扩展](../docs/v1.0.0-RFC.md?plain=1#L65) | [`unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L67) · [`yaml_aliases_can_supply_typed_provider_params_from_an_extension_field`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L163) |

## §3.3

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [name 字符集与默认文件名](../docs/v1.0.0-RFC.md?plain=1#L89) | [`agent_name_has_exactly_the_documented_ascii_alphabet`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L11) · [`test_trace_envelope_identifies_the_live_process_and_effective_agent_name`](../tests/test_shell_pipeline.py#L127) · [#26](https://github.com/Luthics/os-agent/issues/26) |
| [Provider 名必须已定义](../docs/v1.0.0-RFC.md?plain=1#L90) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L34) · [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L520) |
| [model 类型、透传与默认](../docs/v1.0.0-RFC.md?plain=1#L91) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L34) · [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L116) |
| [shell 路径或命令名](../docs/v1.0.0-RFC.md?plain=1#L92) | [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L554) |
| [max_turns 边界和最后一轮完成](../docs/v1.0.0-RFC.md?plain=1#L93) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L34) · [`max_turns_stops_before_another_request_and_success_on_last_turn_is_allowed`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L135) |
| [max_tokens 范围及硬上限退出](../docs/v1.0.0-RFC.md?plain=1#L94) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L34) · [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L437) |
| [单轮输出必须小于上下文预算，0 时省略](../docs/v1.0.0-RFC.md?plain=1#L95) | [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L347) · [`maps_request_and_streams_text_reasoning_usage_and_done`](../crates/provider-openai-responses/tests/rfc_responses.rs#L131) |
| [比例向下取整、绝对阈值与禁用值](../docs/v1.0.0-RFC.md?plain=1#L96) | [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L91) · [`absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L693) · [#5](https://github.com/Luthics/os-agent/issues/5) · [#24](https://github.com/Luthics/os-agent/issues/24) |
| [approval 枚举及默认值](../docs/v1.0.0-RFC.md?plain=1#L97) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L34) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) |
| [stderr 枚举及默认值](../docs/v1.0.0-RFC.md?plain=1#L98) | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L34) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) |
| [context 按 cwd 解析](../docs/v1.0.0-RFC.md?plain=1#L99) | [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L786) · [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L431) |
| [params 的扩展键与浅合并](../docs/v1.0.0-RFC.md?plain=1#L100) | [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L851) · [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L386) |

## §3.4

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [优先级各层的 32 种子集](../docs/v1.0.0-RFC.md?plain=1#L105) | [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L116) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) |
| [环境变量、布尔别名及非法值](../docs/v1.0.0-RFC.md?plain=1#L108) | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L574) · [`invalid_resolved_environment_values_exit_two_before_contacting_the_provider`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L621) |
| [显式空串覆盖会话路径](../docs/v1.0.0-RFC.md?plain=1#L109) | [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L431) |
| [清除父实例状态、保留观测变量与普通环境变量](../docs/v1.0.0-RFC.md?plain=1#L110) | [`shell_environment_contains_only_observability_effective_approval_and_routed_fds`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L280) |
| [子 Agent 继承父实例的生效审批](../docs/v1.0.0-RFC.md?plain=1#L112) | [`test_nested_agent_uses_own_model_wal_and_budget_and_inherits_cli_auto`](../tests/test_shell_pipeline.py#L619) · [`test_nested_agent_inherits_cli_deny_over_its_own_auto_frontmatter`](../tests/test_shell_pipeline.py#L623) |
| [fd 4–6 的环境映射](../docs/v1.0.0-RFC.md?plain=1#L113) | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L149) |
| [跨配置层保留无冲突键、高层整体替换同名子对象](../docs/v1.0.0-RFC.md?plain=1#L115) | [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L851) · [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L386) |

## §4.1

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [管道等到 EOF，终端有参直接启动、无参等到 Ctrl+D](../docs/v1.0.0-RFC.md?plain=1#L127) | [`args_stdin_and_sparse_extra_descriptors_are_ordered_and_trace_is_separate`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L35) · [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L249) · [`tty_stdin_uses_args_immediately_or_reads_input_through_eof`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L173) · [#12](https://github.com/Luthics/os-agent/issues/12) · [`executable_script_on_path_reads_tty_until_eof_only_without_arguments`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L205) · [#34](https://github.com/Luthics/os-agent/issues/34) · [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L343) |
| [stdout 只转发显式原始字节](../docs/v1.0.0-RFC.md?plain=1#L128) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L87) · [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L462) |
| [两种 stderr 模式、静态终端汇总与会话计费](../docs/v1.0.0-RFC.md?plain=1#L129) | [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L462) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) · [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L343) · [#36](https://github.com/Luthics/os-agent/issues/36) |
| [可写 fd 3 优先，其余情况走 journal 或静默](../docs/v1.0.0-RFC.md?plain=1#L130) | [`regular_fd3_separates_events_deltas_and_wire_and_omits_provider_key`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L118) · [`journal_receives_events_on_fd3_fallback_and_never_receives_wire_ir`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L175) |
| [fd 4–9 访问模式、拼接顺序与 EOF](../docs/v1.0.0-RFC.md?plain=1#L131) | [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L207) · [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L249) |
| [Provider、钩子、shell 不能继承已消费的文件](../docs/v1.0.0-RFC.md?plain=1#L133) | [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L207) |

## §4.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [单次任务结束后退出、交付全部字节](../docs/v1.0.0-RFC.md?plain=1#L144) | [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L8) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) |
| [新建四个子目录及根目录的权限与就绪时间](../docs/v1.0.0-RFC.md?plain=1#L146) | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L19) · [`a_publisher_can_queue_work_while_the_initial_cli_task_is_running`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L120) · [#21](https://github.com/Luthics/os-agent/issues/21) |
| [多发送方只发布完整文件](../docs/v1.0.0-RFC.md?plain=1#L147) | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L19) |
| [文件名排序、通知不可用后的 200 ms 轮询](../docs/v1.0.0-RFC.md?plain=1#L148) | [`inbox_observes_only_published_files_in_ascii_order_and_moves_them_after_commit`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L11) · [`test_inbox_polls_new_publications_when_kernel_notifications_are_unavailable`](../tests/test_linux_fallback.py#L184) · [`test_idle_inbox_polls_the_empty_new_directory_at_200_ms_without_a_watcher`](../tests/test_linux_fallback.py#L150) |
| [工具全部闭合后才领取下一项](../docs/v1.0.0-RFC.md?plain=1#L149) | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L19) |
| [成功后归档、失败不确认](../docs/v1.0.0-RFC.md?plain=1#L151) | [`provider_nonzero_after_done_keeps_the_failed_inbox_task_unacknowledged`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L274) · [`inbox_observes_only_published_files_in_ascii_order_and_moves_them_after_commit`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L11) · [#2](https://github.com/Luthics/os-agent/issues/2) |
| [空闲计时起点、持锁与退出](../docs/v1.0.0-RFC.md?plain=1#L154) | [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L188) · [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L50) |

## §4.3

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [工具 stdin 与父 stdin 隔离](../docs/v1.0.0-RFC.md?plain=1#L164) | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L149) |
| [捕获并清洗工具 stdout 与 stderr](../docs/v1.0.0-RFC.md?plain=1#L165) | [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L220) · [`test_wire_file_has_raw_ansi_and_model_receives_cleaned_text`](../tests/test_shell_pipeline.py#L534) |
| [载荷管道写端、转发大量二进制](../docs/v1.0.0-RFC.md?plain=1#L166) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L422) |
| [诊断管道写端及两种展示模式](../docs/v1.0.0-RFC.md?plain=1#L167) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L422) · [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L87) |
| [控制管道写端与实际 JSONL 帧](../docs/v1.0.0-RFC.md?plain=1#L168) | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L422) · [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L115) |
| [连续工具重定向到普通文件时顺序追加](../docs/v1.0.0-RFC.md?plain=1#L170) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L87) |

## §4.4

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [同名全局或已信任项目钩子可覆写和屏蔽](../docs/v1.0.0-RFC.md?plain=1#L174) | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L489) |
| [auto 首轮提示中的交付通道和工具，实际 fd 2 可见性](../docs/v1.0.0-RFC.md?plain=1#L176) | [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L432) · [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L462) |
| [explicit 首轮契约、切换模式后的冻结前缀及输出](../docs/v1.0.0-RFC.md?plain=1#L177) | [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L432) · [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L462) |

## §5.1

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [auto 与环境别名放行工具](../docs/v1.0.0-RFC.md?plain=1#L185) | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L574) |
| [deny 命中或拒绝、错误结果与退出 42](../docs/v1.0.0-RFC.md?plain=1#L186) | [`deny_and_fail_on_deny_differ_in_exit_status_tool_reply_and_side_effects`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L89) · [#9](https://github.com/Luthics/os-agent/issues/9) |
| [ask 的白名单与本 PID 的 Always 决策](../docs/v1.0.0-RFC.md?plain=1#L187) | [`always_approval_is_scoped_to_executable_and_pid_even_with_the_same_wal`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L400) · [`approval_choices_use_dev_tty_and_preserve_piped_stdin`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L255) · [#9](https://github.com/Luthics/os-agent/issues/9) |

## §5.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [从控制终端审批，保留外部 stdin](../docs/v1.0.0-RFC.md?plain=1#L191) | [`approval_choices_use_dev_tty_and_preserve_piped_stdin`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L255) |
| [本次允许](../docs/v1.0.0-RFC.md?plain=1#L193) | [`approval_choices_use_dev_tty_and_preserve_piped_stdin`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L255) |
| [拒绝理由写入返回模型的工具消息](../docs/v1.0.0-RFC.md?plain=1#L194) | [`approval_choices_use_dev_tty_and_preserve_piped_stdin`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L255) |
| [Always 仅作用于同一 PID 和可执行程序](../docs/v1.0.0-RFC.md?plain=1#L195) | [`always_approval_is_scoped_to_executable_and_pid_even_with_the_same_wal`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L400) |
| [编辑后执行、0600、清理与取消](../docs/v1.0.0-RFC.md?plain=1#L196) | [`approval_choices_use_dev_tty_and_preserve_piped_stdin`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L255) · [`unsuccessful_or_empty_terminal_edit_runs_neither_original_nor_edited_command`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L361) · [#10](https://github.com/Luthics/os-agent/issues/10) |

## §5.3

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [无控制终端立即诊断并以 42 退出](../docs/v1.0.0-RFC.md?plain=1#L200) | [`detached_session_requiring_approval_fails_with_42_without_executing_tool`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L462) |

## §6.1

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [单调编号、工具结果锚点及编号耗尽](../docs/v1.0.0-RFC.md?plain=1#L206) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L347) · [`test_real_shell_timeout_returns_an_error_tool_result_and_inference_continues`](../tests/test_shell_pipeline.py#L527) · [`running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L461) · [#18](https://github.com/Luthics/os-agent/issues/18) |
| [最后存活回答的 usage 与后续字节估算](../docs/v1.0.0-RFC.md?plain=1#L209) | [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L634) · [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L95) · [#5](https://github.com/Luthics/os-agent/issues/5) · [#20](https://github.com/Luthics/os-agent/issues/20) |
| [首轮及 checkpoint 后估算、排除压缩器用量](../docs/v1.0.0-RFC.md?plain=1#L210) | [`fresh_window_counts_utf8_bytes_in_groups_of_three`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L421) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) · [#5](https://github.com/Luthics/os-agent/issues/5) |
| [fold 后的水位回落](../docs/v1.0.0-RFC.md?plain=1#L211) | [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L634) |
| [等于硬上限允许、超过后停止](../docs/v1.0.0-RFC.md?plain=1#L212) | [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L437) · [`auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L467) |
| [显式 compact 写入新分节并刷新系统提示词](../docs/v1.0.0-RFC.md?plain=1#L216) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) |
| [有效范围、前缀保存、剪去发起调用的轮次](../docs/v1.0.0-RFC.md?plain=1#L217) | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L297) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L347) · [#3](https://github.com/Luthics/os-agent/issues/3) |
| [缺摘要的 CLI 用法错误](../docs/v1.0.0-RFC.md?plain=1#L218) | [`invalid_aclr_argv_never_submits_a_frame`](../crates/agent-tools/tests/rfc_control.rs#L57) |
| [无参 aclr 输出用法并退出 2](../docs/v1.0.0-RFC.md?plain=1#L219) | [`invalid_aclr_argv_never_submits_a_frame`](../crates/agent-tools/tests/rfc_control.rs#L57) |
| [tool 锚点回退到成批调用的 assistant](../docs/v1.0.0-RFC.md?plain=1#L220) | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L297) |
| [外部压缩器只收 IR、输出须为非空 UTF-8、故障退出](../docs/v1.0.0-RFC.md?plain=1#L222) | [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L350) |
| [provider、model、提示词文件与内置回退](../docs/v1.0.0-RFC.md?plain=1#L223) | [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L526) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) |
| [压缩请求没有工具](../docs/v1.0.0-RFC.md?plain=1#L224) | [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L350) · [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L526) |

## §6.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [独占非阻塞锁、冲突时退出并诊断](../docs/v1.0.0-RFC.md?plain=1#L230) | [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L196) |
| [session_init、冻结 system、恢复时复用](../docs/v1.0.0-RFC.md?plain=1#L235) | [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L215) · [`initial_section_interruption_never_dispatches_without_a_frozen_system_message`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L580) · [#19](https://github.com/Luthics/os-agent/issues/19) |
| [只在漂移处追加更新](../docs/v1.0.0-RFC.md?plain=1#L236) | [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L215) · [`resume_compares_both_contract_fields_after_a_sparse_runtime_update`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L377) · [#17](https://github.com/Luthics/os-agent/issues/17) |
| [只在全量压缩时重新运行钩子](../docs/v1.0.0-RFC.md?plain=1#L237) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) · [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L297) · [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L503) · [#19](https://github.com/Luthics/os-agent/issues/19) |
| [WAL 每行提交、短写、五字段累计和跨分节恢复](../docs/v1.0.0-RFC.md?plain=1#L238) | [`wal_events_round_trip`](../crates/os-agent-proto/src/wal.rs#L93) · [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L163) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) · [#8](https://github.com/Luthics/os-agent/issues/8) |
| [跨块逆读到最近分节，读取量独立于归档长度](../docs/v1.0.0-RFC.md?plain=1#L259) | [`checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L109) · [`test_archived_wal_growth_does_not_require_reading_the_archive_on_resume`](../tests/test_linux_fallback.py#L37) |
| [逐字节截断、损坏时保留原件](../docs/v1.0.0-RFC.md?plain=1#L260) | [`every_partial_byte_write_of_a_final_record_fails_without_modifying_history`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L35) · [`resuming_a_complete_last_object_without_lf_never_joins_two_wal_records`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L59) · [#13](https://github.com/Luthics/os-agent/issues/13) |
| [多次重叠 fold 的剪枝水位与顺序](../docs/v1.0.0-RFC.md?plain=1#L262) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L347) · [`overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L137) |
| [被剪枝回答的 usage 仍进入会话总量](../docs/v1.0.0-RFC.md?plain=1#L263) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L347) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) |
| [未闭合工具对在回放中被剪除](../docs/v1.0.0-RFC.md?plain=1#L264) | [`replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage`](../crates/os-agent-core/src/wal.rs#L711) · [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L369) |
| [分节版本检查与前缀和](../docs/v1.0.0-RFC.md?plain=1#L265) | [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L173) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) |
| [正序回放与前向参考模型逐轮对照](../docs/v1.0.0-RFC.md?plain=1#L266) | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L347) |

## §6.3

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [INT 中断、三秒宽限、部分轮次丢弃、释放锁](../docs/v1.0.0-RFC.md?plain=1#L272) | [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L12) · [`interruption_waits_three_seconds_then_stops_a_provider_that_handles_term_without_exiting`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L170) · [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L369) · [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L343) · [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L399) · [#35](https://github.com/Luthics/os-agent/issues/35) |
| [TERM 中断、PID 与清理结果](../docs/v1.0.0-RFC.md?plain=1#L273) | [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L12) · [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L369) |
| [STOP 挂起与 CONT 恢复](../docs/v1.0.0-RFC.md?plain=1#L274) | [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L50) |
| [USR1 在下次推理前压缩一次](../docs/v1.0.0-RFC.md?plain=1#L275) | [`usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L117) |
| [USR2 快照的实际内容、位置和权限](../docs/v1.0.0-RFC.md?plain=1#L276) | [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L50) |

## §6.4

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [0 与已刷新的 payload](../docs/v1.0.0-RFC.md?plain=1#L282) | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L87) |
| [1 对应 Provider、WAL I/O 和锁冲突](../docs/v1.0.0-RFC.md?plain=1#L283) | [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L144) · [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L163) · [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L196) |
| [2 对应参数、配置或协议版本错误](../docs/v1.0.0-RFC.md?plain=1#L284) | [`invalid_file_bytes_and_yaml_report_usage_errors_to_stderr`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L90) · [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L648) · [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L173) |
| [42 对应审批及信任失败](../docs/v1.0.0-RFC.md?plain=1#L285) | [`detached_session_requiring_approval_fails_with_42_without_executing_tool`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L462) · [`new_and_modified_project_hooks_require_explicit_trust`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L201) |
| [124 对应预算及 Provider 超时，单次工具超时可继续](../docs/v1.0.0-RFC.md?plain=1#L286) | [`max_turns_stops_before_another_request_and_success_on_last_turn_is_allowed`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L135) · [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L437) · [`provider_timeout_maps_to_124_without_a_retry`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L155) · [`test_real_shell_timeout_returns_an_error_tool_result_and_inference_continues`](../tests/test_shell_pipeline.py#L527) |
| [INT 清理后退出 130](../docs/v1.0.0-RFC.md?plain=1#L287) | [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L12) · [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L343) · [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L399) · [#35](https://github.com/Luthics/os-agent/issues/35) |
| [TERM 清理后退出 143](../docs/v1.0.0-RFC.md?plain=1#L288) | [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L12) |

## §7.1

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [msh 使用独立进程组、超时回收](../docs/v1.0.0-RFC.md?plain=1#L294) | [`test_msh_timeout_signals_the_whole_group_and_grants_three_seconds_before_kill`](../tests/test_shell_pipeline.py#L331) |
| [六个变量强制生效、工具目录进入 PATH](../docs/v1.0.0-RFC.md?plain=1#L296) | [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L220) · [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L149) |
| [8 KiB NUL 边界与全流 UTF-8 校验](../docs/v1.0.0-RFC.md?plain=1#L297) | [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L34) · [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L242) |
| [探测 MIME、保存原始字节、路径和 0600](../docs/v1.0.0-RFC.md?plain=1#L299) | [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L242) · [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L34) |
| [二进制输出单行说明](../docs/v1.0.0-RFC.md?plain=1#L300) | [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L242) |
| [CSI 终结字节、OSC 终止方式和回车覆盖](../docs/v1.0.0-RFC.md?plain=1#L301) | [`ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites`](../crates/msh/tests/rfc_shell.rs#L65) |
| [默认双阈值、环境覆盖、首尾 20 行和完整清洗日志](../docs/v1.0.0-RFC.md?plain=1#L302) | [`unconfigured_msh_folds_only_above_200_lines_or_8192_bytes`](../crates/msh/tests/rfc_shell.rs#L280) · [`cli_applies_environment_limits_to_cleaned_lines_and_bytes_before_saving_output`](../crates/msh/tests/rfc_shell.rs#L169) · [`line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text`](../crates/msh/tests/rfc_shell.rs#L93) · [#15](https://github.com/Luthics/os-agent/issues/15) · [`byte_limit_independent_from_line_limit_preserves_multibyte_file`](../crates/msh/tests/rfc_shell.rs#L134) |
| [默认 120 秒、覆盖、TERM/KILL、124 工具结果](../docs/v1.0.0-RFC.md?plain=1#L303) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) · [`test_msh_timeout_signals_the_whole_group_and_grants_three_seconds_before_kill`](../tests/test_shell_pipeline.py#L331) · [`test_real_shell_timeout_returns_an_error_tool_result_and_inference_continues`](../tests/test_shell_pipeline.py#L527) · [#29](https://github.com/Luthics/os-agent/issues/29) · [`timeout_reports_124_before_a_finite_longer_command_completes`](../crates/msh/tests/rfc_shell.rs#L267) · [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L163) |

## §7.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [原生工具和图片管道的字节一致性](../docs/v1.0.0-RFC.md?plain=1#L307) | [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) |
| [fd 6 的 JSONL 跨读取边界与多帧顺序、64 KB 边界](../docs/v1.0.0-RFC.md?plain=1#L308) | [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L115) · [`two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L188) · [`test_aclr_and_agent_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L77) · [#14](https://github.com/Luthics/os-agent/issues/14) · [`fold_control_frames_stay_within_64_kib_even_after_json_escaping`](../crates/agent-tools/tests/rfc_control.rs#L78) · [`ctl_frames_round_trip`](../crates/os-agent-proto/src/ctl.rs#L31) |
| [图像尺寸、方向、stdin、PDF 选页和 PNG 权限](../docs/v1.0.0-RFC.md?plain=1#L316) | [`aview_resizes_portrait_landscape_and_square_images_only_when_needed`](../crates/agent-tools/tests/rfc_control.rs#L130) · [`aview_preserves_image_content_and_orientation_in_the_published_png`](../crates/agent-tools/tests/rfc_control.rs#L168) · [`test_single_file_and_stdin_render_the_selected_pages`](../tests/test_pdf_pages.py#L51) · [`aview_file_input_and_stdin_agree_on_image_dimensions`](../crates/agent-tools/tests/rfc_control.rs#L153) · [`undecodable_input_or_invalid_options_do_not_emit_media`](../crates/agent-tools/tests/rfc_control.rs#L212) |
| [compact 和 fold 的字段与序列化](../docs/v1.0.0-RFC.md?plain=1#L317) | [`aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping`](../crates/agent-tools/tests/rfc_control.rs#L40) · [`invalid_aclr_argv_never_submits_a_frame`](../crates/agent-tools/tests/rfc_control.rs#L57) |

## §8.1

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [XDG_CONFIG_HOME 与 HOME 回退](../docs/v1.0.0-RFC.md?plain=1#L328) | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L994) |
| [XDG_STATE_HOME 与 HOME 回退](../docs/v1.0.0-RFC.md?plain=1#L329) | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L994) |
| [XDG_RUNTIME_DIR、按 UID 回退及权限](../docs/v1.0.0-RFC.md?plain=1#L330) | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L994) · [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L99) · [`each_missing_or_non_executable_private_binary_is_identified`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L31) |
| [最近的 .agent、.git 和 HOME 边界](../docs/v1.0.0-RFC.md?plain=1#L331) | [`project_search_stops_at_nearest_configuration_git_or_home_boundary`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L465) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L786) |

## §8.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [defaults 字段和配置层优先级](../docs/v1.0.0-RFC.md?plain=1#L336) | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) · [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L116) · [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L648) |
| [白名单、命令首词与多词条目](../docs/v1.0.0-RFC.md?plain=1#L349) | [`deny_and_fail_on_deny_differ_in_exit_status_tool_reply_and_side_effects`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L89) · [`always_approval_is_scoped_to_executable_and_pid_even_with_the_same_wal`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L400) · [#9](https://github.com/Luthics/os-agent/issues/9) |
| [Provider 解析、base URL、超时和凭据来源](../docs/v1.0.0-RFC.md?plain=1#L352) | [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L520) · [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L928) |
| [所选 Provider 的 params 浅合并](../docs/v1.0.0-RFC.md?plain=1#L359) | [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L851) |
| [内置及外部压缩器选取](../docs/v1.0.0-RFC.md?plain=1#L362) | [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L526) · [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L350) |

## §8.3

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [ASCII 顺序、项目同名覆盖、两种掩码及内置优先级](../docs/v1.0.0-RFC.md?plain=1#L375) | [`pre_context_hooks_merge_in_ascii_order_with_override_and_two_masks`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L163) · [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L489) |
| [trust 记录真实路径和 SHA-256，包括项目 Provider 和压缩器](../docs/v1.0.0-RFC.md?plain=1#L376) | [`new_and_modified_project_hooks_require_explicit_trust`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L201) · [`project_provider_and_compactor_each_require_trust_after_a_content_change`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L232) |
| [信任变更时报告路径及 42](../docs/v1.0.0-RFC.md?plain=1#L382) | [`new_and_modified_project_hooks_require_explicit_trust`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L201) · [`project_provider_and_compactor_each_require_trust_after_a_content_change`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L232) |

## §9.1

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [IR 无 model，目标模型由环境注入](../docs/v1.0.0-RFC.md?plain=1#L392) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/os-agent-proto/tests/rfc_ir.rs#L35) · [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L520) · [`selected_model_changes_the_provider_environment_without_entering_the_ir`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L1052) |
| [扁平消息字段、附件引用、按行解析](../docs/v1.0.0-RFC.md?plain=1#L393) | [`golden_ir_keeps_frozen_text_extensions_and_attachment_references_without_opening_files`](../crates/os-agent-proto/tests/rfc_ir.rs#L155) · [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/os-agent-proto/tests/rfc_ir.rs#L216) |
| [唯一 header 及必填字段](../docs/v1.0.0-RFC.md?plain=1#L394) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/os-agent-proto/tests/rfc_ir.rs#L35) · [`ir_jsonl_header_and_messages_round_trip`](../crates/os-agent-proto/src/ir.rs#L352) |
| [上游调用前拒绝不兼容版本](../docs/v1.0.0-RFC.md?plain=1#L395) | [`unsupported_ir_version_and_missing_media_fail_before_http`](../crates/provider-openai-responses/tests/rfc_responses.rs#L278) |
| [tools 必填，空表在上游省略](../docs/v1.0.0-RFC.md?plain=1#L397) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/os-agent-proto/tests/rfc_ir.rs#L35) · [`maps_request_and_streams_text_reasoning_usage_and_done`](../crates/provider-openai-responses/tests/rfc_responses.rs#L131) |
| [省略、零和显式上限的上游映射](../docs/v1.0.0-RFC.md?plain=1#L398) | [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L347) · [`maps_request_and_streams_text_reasoning_usage_and_done`](../crates/provider-openai-responses/tests/rfc_responses.rs#L131) |
| [params 的 wire 类型、透传与上游映射](../docs/v1.0.0-RFC.md?plain=1#L399) | [`meta_and_header_options_have_their_declared_wire_types`](../crates/os-agent-proto/tests/rfc_ir.rs#L67) · [`maps_request_and_streams_text_reasoning_usage_and_done`](../crates/provider-openai-responses/tests/rfc_responses.rs#L131) |
| [meta 的 cwd、stderr_mode、预算](../docs/v1.0.0-RFC.md?plain=1#L400) | [`meta_and_header_options_have_their_declared_wire_types`](../crates/os-agent-proto/tests/rfc_ir.rs#L67) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L786) |
| [扩展字段的解码及钩子透传](../docs/v1.0.0-RFC.md?plain=1#L401) | [`golden_ir_keeps_frozen_text_extensions_and_attachment_references_without_opening_files`](../crates/os-agent-proto/tests/rfc_ir.rs#L155) · [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L10) |
| [首条系统文本字节保真与尾部系统消息映射](../docs/v1.0.0-RFC.md?plain=1#L403) | [`runtime_update_stays_in_chronological_input_and_frozen_instructions_keep_their_bytes`](../crates/provider-openai-responses/tests/rfc_responses.rs#L175) |
| [user 必需字段及可选媒体](../docs/v1.0.0-RFC.md?plain=1#L404) | [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/os-agent-proto/tests/rfc_ir.rs#L216) · [`adapter_reads_each_declared_media_type_from_disk_in_user_and_tool_messages`](../crates/provider-openai-responses/tests/rfc_responses.rs#L361) |
| [assistant 文本或工具调用非空，opaque 与 arguments 类型](../docs/v1.0.0-RFC.md?plain=1#L405) | [`assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state`](../crates/os-agent-proto/tests/rfc_ir.rs#L251) · [`validated_ir_only_serializes_objects_for_arguments_and_provider_opaque`](../crates/os-agent-proto/tests/rfc_ir.rs#L102) |
| [tool 标识、结果和附件](../docs/v1.0.0-RFC.md?plain=1#L406) | [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/os-agent-proto/tests/rfc_ir.rs#L216) · [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/os-agent-proto/tests/rfc_ir.rs#L178) · [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L115) |
| [附件 MIME 与绝对路径约束](../docs/v1.0.0-RFC.md?plain=1#L408) | [`tool_control_frames_cannot_publish_invalid_media_references_to_wal_or_provider_ir`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L24) · [`adapter_reads_each_declared_media_type_from_disk_in_user_and_tool_messages`](../crates/provider-openai-responses/tests/rfc_responses.rs#L361) · [#16](https://github.com/Luthics/os-agent/issues/16) |
| [请求前读取媒体，缺失或不可读时永久失败](../docs/v1.0.0-RFC.md?plain=1#L409) | [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) · [`unsupported_ir_version_and_missing_media_fail_before_http`](../crates/provider-openai-responses/tests/rfc_responses.rs#L278) · [`unreadable_media_emits_a_permanent_error_before_dispatch`](../crates/provider-openai-responses/tests/rfc_responses.rs#L327) |
| [checkpoint 映射为新冻结提示词与 user 摘要](../docs/v1.0.0-RFC.md?plain=1#L411) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) · [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L503) |
| [fold 对齐后插入 user 摘要](../docs/v1.0.0-RFC.md?plain=1#L412) | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L297) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L347) |
| [尾部 runtime_update 及上游角色映射](../docs/v1.0.0-RFC.md?plain=1#L413) | [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L215) · [`runtime_update_stays_in_chronological_input_and_frozen_instructions_keep_their_bytes`](../crates/provider-openai-responses/tests/rfc_responses.rs#L175) |
| [header、工具一一配对且连续、编号顺序与共享锚点](../docs/v1.0.0-RFC.md?plain=1#L414) | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/os-agent-proto/tests/rfc_ir.rs#L35) · [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/os-agent-proto/tests/rfc_ir.rs#L178) · [`a_single_tool_result_cannot_close_two_calls_with_the_same_id`](../crates/os-agent-proto/tests/rfc_ir.rs#L206) · [`ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared`](../crates/os-agent-proto/tests/rfc_ir.rs#L270) |

## §9.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [有效 model、base URL、timeout 与密钥来源](../docs/v1.0.0-RFC.md?plain=1#L429) | [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L520) · [#4](https://github.com/Luthics/os-agent/issues/4) |
| [Provider 收到完整 IR 及 EOF](../docs/v1.0.0-RFC.md?plain=1#L430) | [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L10) · [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L520) · [`selected_model_changes_the_provider_environment_without_entering_the_ir`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L1052) |
| [增量、交错工具调用、计费、opaque 和终止事件](../docs/v1.0.0-RFC.md?plain=1#L431) | [`provider_events_round_trip`](../crates/os-agent-proto/src/provider.rs#L111) · [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L221) · [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L254) · [`repeated_sse_notifications_emit_one_complete_tool_call`](../crates/provider-openai-responses/tests/rfc_responses.rs#L214) |
| [WAL、Trace、on-exit 五字段一致，请求 IR 剥离用量](../docs/v1.0.0-RFC.md?plain=1#L447) | [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L26) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) |
| [总输入输出包含缓存与推理明细](../docs/v1.0.0-RFC.md?plain=1#L448) | [`response_usage_preserves_inclusive_totals_and_available_details`](../crates/provider-openai-responses/tests/rfc_responses.rs#L425) · [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L26) |
| [可选明细、uint64 类型及零值处理](../docs/v1.0.0-RFC.md?plain=1#L449) | [`each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values`](../crates/os-agent-proto/tests/rfc_provider.rs#L7) · [`usage_requires_both_totals_even_when_all_three_details_are_present`](../crates/os-agent-proto/tests/rfc_provider.rs#L56) · [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L95) · [#22](https://github.com/Luthics/os-agent/issues/22) |
| [run 含压缩，区分跨会话历史消耗](../docs/v1.0.0-RFC.md?plain=1#L450) | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) · [`retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L135) · [#8](https://github.com/Luthics/os-agent/issues/8) |
| [同 Provider 原样回传，切换时清除](../docs/v1.0.0-RFC.md?plain=1#L451) | [`opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L161) · [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L254) · [#6](https://github.com/Luthics/os-agent/issues/6) |
| [适配器单次请求、429/5xx、核心三次退避与 Trace](../docs/v1.0.0-RFC.md?plain=1#L452) | [`transient_http_errors_are_retryable_but_adapter_never_retries_itself`](../crates/provider-openai-responses/tests/rfc_responses.rs#L242) · [`retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L21) · [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L313) |
| [缺 done、非零退出、超时及 stderr](../docs/v1.0.0-RFC.md?plain=1#L453) | [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L144) · [`provider_nonzero_after_done_keeps_the_failed_inbox_task_unacknowledged`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L274) · [`provider_timeout_maps_to_124_without_a_retry`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L155) · [`nonretryable_error_preserves_provider_stderr_without_another_attempt`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L133) · [`missing_terminal_event_is_not_reported_as_success`](../crates/provider-openai-responses/tests/rfc_responses.rs#L258) · [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L195) · [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L163) · [#33](https://github.com/Luthics/os-agent/issues/33) |

## §9.3

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [pre-context 连续过滤 IR，失败向 fd 2 报告并停止](../docs/v1.0.0-RFC.md?plain=1#L457) | [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L10) · [`pre_context_failure_forwards_stderr_and_stops_before_inference`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L81) |
| [post-tool 改写结果，失败停止后续工具](../docs/v1.0.0-RFC.md?plain=1#L458) | [`post_tool_filters_the_model_result_and_on_exit_failure_keeps_primary_status`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L309) · [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L107) |
| [on-exit 用量、退出状态、5 秒超时与错误隔离](../docs/v1.0.0-RFC.md?plain=1#L459) | [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) · [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L153) |

## §9.4

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [20-skills 内置、同名覆写与禁用](../docs/v1.0.0-RFC.md?plain=1#L463) | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L489) |
| [六层搜索顺序和同名覆盖](../docs/v1.0.0-RFC.md?plain=1#L465) | [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L190) |
| [SKILL.md 生成转义 XML 元数据，正文不进首轮](../docs/v1.0.0-RFC.md?plain=1#L466) | [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L206) · [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L190) |
| [按索引路径经 msh 读取正文](../docs/v1.0.0-RFC.md?plain=1#L474) | [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L206) |

## §9.5

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [fd 3、socket、断开或停止读取、凭据隔离](../docs/v1.0.0-RFC.md?plain=1#L478) | [`disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L216) · [`test_inference_can_finish_with_an_open_trace_consumer_that_stops_reading`](../tests/test_shell_pipeline.py#L295) · [`regular_fd3_separates_events_deltas_and_wire_and_omits_provider_key`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L118) · [#11](https://github.com/Luthics/os-agent/issues/11) |
| [ts、pid、ppid、agent、v 与 type](../docs/v1.0.0-RFC.md?plain=1#L479) | [`test_trace_envelope_identifies_the_live_process_and_effective_agent_name`](../tests/test_shell_pipeline.py#L127) |
| [请求、响应、工具、折叠、压缩、重试与退出事件](../docs/v1.0.0-RFC.md?plain=1#L484) | [`regular_fd3_separates_events_deltas_and_wire_and_omits_provider_key`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L118) · [`events_distinguish_tool_approval_from_execution_and_record_an_applied_fold`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L16) · [`provider_failure_after_partial_text_retries_with_unchanged_ir_and_emits_trace_evidence`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L424) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) · [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L107) · [#32](https://github.com/Luthics/os-agent/issues/32) |
| [增量及时抵达 fd 3](../docs/v1.0.0-RFC.md?plain=1#L485) | [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L221) |
| [wire 包含原始 IR、帧和工具字节，只写私有 0600 文件](../docs/v1.0.0-RFC.md?plain=1#L486) | [`wire_trace_only_writes_full_ir_into_a_private_file`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L147) · [`journal_receives_events_on_fd3_fallback_and_never_receives_wire_ir`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L175) · [`test_wire_file_has_raw_ansi_and_model_receives_cleaned_text`](../tests/test_shell_pipeline.py#L534) |

## §9.6

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [文件名组成索引，正文不进 prompt](../docs/v1.0.0-RFC.md?plain=1#L492) | [`frozen_context_lists_skill_metadata_and_tip_paths_but_leaves_bodies_on_disk`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L361) · [`background_tip_is_indexed_in_ascii_order_and_can_be_overridden_or_masked_individually`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L377) |
| [首次释放、版本更新和状态目录不可写时回退](../docs/v1.0.0-RFC.md?plain=1#L494) | [`tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L253) · [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L292) |
| [隐藏项、同名覆盖、逐篇屏蔽和空索引](../docs/v1.0.0-RFC.md?plain=1#L497) | [`background_tip_is_indexed_in_ascii_order_and_can_be_overridden_or_masked_individually`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L377) · [`an_empty_merged_tips_index_costs_no_prompt_bytes`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L343) · [`frozen_context_lists_skill_metadata_and_tip_paths_but_leaves_bodies_on_disk`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L361) |
| [规范绝对路径与 ASCII 去重排序](../docs/v1.0.0-RFC.md?plain=1#L498) | [`background_tip_is_indexed_in_ascii_order_and_can_be_overridden_or_masked_individually`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L377) · [`frozen_context_lists_skill_metadata_and_tip_paths_but_leaves_bodies_on_disk`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L361) |
| [整块覆写、两类禁用及磁盘副作用](../docs/v1.0.0-RFC.md?plain=1#L513) | [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L292) · [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L489) |

## §10.2

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [Linux、macOS 的 POSIX 基础行为（WSL2 暂缓）](../docs/v1.0.0-RFC.md?plain=1#L526) | [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) |
| [父进程异常退出后回收 Provider 和 msh](../docs/v1.0.0-RFC.md?plain=1#L528) | [`abrupt_parent_exit_stops_the_active_provider_or_shell_child`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L210) |
| [可用和不支持的内核、警告及 strict 退出](../docs/v1.0.0-RFC.md?plain=1#L529) | [`supported_landlock_allows_writes_to_the_three_roots_and_keeps_other_fixture_files_readable`](../crates/os-agent-core/tests/rfc_runtime/platform.rs#L30) · [`strict_requires_landlock_and_best_effort_warns_when_the_kernel_lacks_it`](../crates/os-agent-core/tests/rfc_runtime/platform.rs#L7) · [`test_unavailable_landlock_warns_or_fails_before_inference`](../tests/test_linux_fallback.py#L85) · [`probes_platform_capabilities`](../crates/os-agent-core/src/platform/mod.rs#L87) |

## §10.3

| 条款与验收点 | 测试或待裁决问题 |
| --- | --- |
| [临时 HOME 安装与重装、FHS 和 shell rc，发布入口待定](../docs/v1.0.0-RFC.md?plain=1#L533) | [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L90) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) · [#7](https://github.com/Luthics/os-agent/issues/7) |
| [混合故障时的自检执行顺序与诊断](../docs/v1.0.0-RFC.md?plain=1#L534) | [`test_doctor_checks_missing_msh_before_diagnosing_invalid_toml`](../tests/test_linux_fallback.py#L111) · [#30](https://github.com/Luthics/os-agent/issues/30) |
| [五处文件及执行位、自定义默认 Provider 解析](../docs/v1.0.0-RFC.md?plain=1#L536) | [`each_missing_or_non_executable_private_binary_is_identified`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L31) · [`doctor_and_the_runtime_can_use_a_configured_default_provider_on_path`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L122) · [#25](https://github.com/Luthics/os-agent/issues/25) |
| [全局与项目配置、凭据和免鉴权模式](../docs/v1.0.0-RFC.md?plain=1#L537) | [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L99) · [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L70) · [#4](https://github.com/Luthics/os-agent/issues/4) · [#31](https://github.com/Luthics/os-agent/issues/31) |
| [匿名管道往返与故障诊断](../docs/v1.0.0-RFC.md?plain=1#L538) | [`test_doctor_detects_broken_pipes_and_ineffective_or_unavailable_flock`](../tests/test_linux_fallback.py#L133) |
| [运行目录、flock 故障和错误双重授锁](../docs/v1.0.0-RFC.md?plain=1#L539) | [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L99) · [`test_doctor_detects_broken_pipes_and_ineffective_or_unavailable_flock`](../tests/test_linux_fallback.py#L133) |
