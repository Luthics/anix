# v1.2.0 RFC 条款验收表

编号的前半部分是 RFC 章节，后半部分是本仓库的稳定验收编号。源码注释与清单独立维护，CI 要求两边声明同一批测试。

在源码测试声明前写 `RFC §编号 — 本用例的输入或操作及预期结果。`。编辑 [rfc-requirements.tsv](rfc-requirements.tsv) 时保留已有编号，为新行为分配新编号。清单中的 `!测试名` 还要求在 Python 断言正上方写 `# RFC-ASSERT §编号`。构建二进制后，运行 `python3 scripts/check-rfc-assertions.py` 验证每处标记的断言都已执行。

CI 逐行检查 §3.3、§4.1、§4.3、§6.4、§6.5 的五张表，每行至少关联一项可执行测试。单列 issue 的验收项仍待规范裁决，TODO 表示还有待补断言。执行结果见对应提交的 CI。按测试查找见[逐测试索引](RFC-cases.md)。

## §1.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§1.2-01](../docs/RFC.md?plain=1#L21) | 安装后的 .agent 脚本以 shebang 从 PATH 或相对路径直接启动 | [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L251) |

## §1.4

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§1.4-01](../docs/RFC.md?plain=1#L37) | 旧命令仍可解析，默认超时已从杀组并返回 124 改为返回 0 并移交后台任务 | [`test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log`](../tests/test_msh_jobs.py#L118) · [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L264) · [#59](https://github.com/Luthics/anix/issues/59) |

## §2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§2-01](../docs/RFC.md?plain=1#L62) | PATH 中的同名 msh 可覆盖私有安装位置，显式路径仍优先 | [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L576) |
| [§2-02](../docs/RFC.md?plain=1#L62) | Provider 的命令名依次查找 PATH 和私有目录，显式路径直接执行 | [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L963) · [#28](https://github.com/Luthics/anix/issues/28) |
| [§2-03](../docs/RFC.md?plain=1#L63) | 子进程优先解析私有工具 | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L154) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) |
| [§2-04](../docs/RFC.md?plain=1#L59) | 参考 Provider 按入口选择对应 HTTP 方言，其本地模拟响应可转换为协议事件 | [`resolves_anthropic_endpoint_urls`](../crates/anix-providers/src/anthropic.rs#L942) · [`resolves_dialect_from_argv0_symlink_and_version`](../crates/anix-providers/src/lib.rs#L232) · [`resolves_dialect_from_subcommand_flag_and_env`](../crates/anix-providers/src/lib.rs#L252) · [`endpoint_url_appends_chat_completions_once`](../crates/anix-providers/src/openai_compat.rs#L926) · [`version_flag_works_for_base_binary_and_busybox_symlinks`](../crates/anix-providers/tests/e2e_mock.rs#L202) · [`busybox_symlink_openai_responses_end_to_end`](../crates/anix-providers/tests/e2e_mock.rs#L251) · [`busybox_symlink_openai_compat_end_to_end_and_unauthenticated_local`](../crates/anix-providers/tests/e2e_mock.rs#L294) · [`busybox_symlink_anthropic_end_to_end_with_inclusive_usage_and_headers`](../crates/anix-providers/tests/e2e_mock.rs#L333) |
| [§2-05](../docs/RFC.md?plain=1#L53) | x86_64 musl 发行包的五个二进制均可执行，没有 PT_INTERP 或 DT_NEEDED | [`test_every_musl_release_executable_has_no_interpreter_or_needed_libraries`](../tests/test_static_release.py#L32) |

## §3.1

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§3.1-01](../docs/RFC.md?plain=1#L72) | 直接传参、可选分隔符及以短横线开头的参数 | [`parses_script_and_args_with_double_dash`](../crates/anix-core/src/cli.rs#L220) · [`parses_direct_and_quoted_script_args_without_double_dash`](../crates/anix-core/src/cli.rs#L238) · [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/anix-core/tests/rfc_runtime/io.rs#L200) |
| [§3.1-02](../docs/RFC.md?plain=1#L73) | trust 子命令与普通脚本参数分别解析 | [`trust_subcommand_does_not_conflict_with_script_args`](../crates/anix-core/src/cli.rs#L261) |
| [§3.1-03](../docs/RFC.md?plain=1#L74) | 自检与凭据诊断 | [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L71) · [#31](https://github.com/Luthics/anix/issues/31) |
| [§3.1-04](../docs/RFC.md?plain=1#L75) | 版本与 ir_version | [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/anix-core/tests/rfc_runtime/io.rs#L8) |
| [§3.1-05](../docs/RFC.md?plain=1#L81) | 空 context 和显式 WAL 路径 | [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L451) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) |
| [§3.1-06](../docs/RFC.md?plain=1#L82) | Provider、模型、shell 选择和覆盖 | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) · [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L963) |
| [§3.1-07](../docs/RFC.md?plain=1#L85) | 轮数、上下文、单轮输出和压缩参数 | [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L96) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) · [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L364) |
| [§3.1-08](../docs/RFC.md?plain=1#L86) | 审批枚举及简写 | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L595) · [#27](https://github.com/Luthics/anix/issues/27) |
| [§3.1-09](../docs/RFC.md?plain=1#L87) | 重复键、JSON 值、原字符串与格式错误 | [`cli_params_preserve_json_literals_and_the_exact_fallback_string`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L752) · [`malformed_cli_param_stops_before_provider_or_wal_creation`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L789) · [#27](https://github.com/Luthics/anix/issues/27) |
| [§3.1-10](../docs/RFC.md?plain=1#L88) | quiet 与显式 stderr 模式 | [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L474) · [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) · [#27](https://github.com/Luthics/anix/issues/27) |
| [§3.1-11](../docs/RFC.md?plain=1#L91) | 默认闲置时限和有限等待的计时起点 | [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) · [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L182) |
| [§3.1-12](../docs/RFC.md?plain=1#L83) | 对照 §3.1 的封闭选项集合、§3.3 的枚举和真实 CLI help；新增、缺失或多余取值均可定位 | [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) [断言 L75](../tests/test_rfc_tables.py#L75) |

## §3.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§3.2-01](../docs/RFC.md?plain=1#L95) | UTF-8 字节、截断、非法序列、LF 与 CRLF | [`crlf_script_reaches_the_provider_as_the_intended_system_message`](../crates/anix-core/tests/rfc_3_2/cli.rs#L44) · [`all_256_single_bytes_inside_a_plain_prompt`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L11) · [`valid_multibyte_boundaries_survive_file_decoding`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L37) · [`malformed_sequences_are_rejected_in_every_script_region`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L64) · [`eof_at_each_interior_byte_of_a_utf8_scalar`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L104) · [`layout_matrix_for_line_ending_shebang_header_body_and_eof`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L9) |
| [§3.2-02](../docs/RFC.md?plain=1#L96) | 仅去掉首行 shebang | [`only_a_first_line_beginning_with_hash_bang_is_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L46) |
| [§3.2-03](../docs/RFC.md?plain=1#L97) | 分隔行、提示词空行及 YAML 1.2 | [`yaml_block_scalar_chomping_survives_the_frontmatter_boundary`](../crates/anix-core/tests/rfc_3_2/cli.rs#L107) · [`duplicate_yaml_keys_are_invalid_even_inside_open_maps`](../crates/anix-core/tests/rfc_3_2/cli.rs#L150) · [`opening_delimiter_requires_the_designated_line_and_exact_marker`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L69) · [`first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L92) · [`yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L115) · [`incomplete_delimiters_and_malformed_yaml_fail_at_the_header`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L145) · [#1](https://github.com/Luthics/anix/issues/1) · [#23](https://github.com/Luthics/anix/issues/23) |
| [§3.2-04](../docs/RFC.md?plain=1#L98) | 未知字段和 x-* 扩展 | [`unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position`](../crates/anix-core/tests/rfc_3_2/cli.rs#L67) · [`yaml_aliases_can_supply_typed_provider_params_from_an_extension_field`](../crates/anix-core/tests/rfc_3_2/cli.rs#L163) |

## §3.3

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§3.3-01](../docs/RFC.md?plain=1#L129) | 最后一轮完成返回 0，仍需推理时返回 124，均不再请求 Provider | [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L235) |
| [§3.3-02](../docs/RFC.md?plain=1#L123) | 显式 name 的字符集，默认文件名含非法字符时的处理待定 | [`agent_name_has_exactly_the_documented_ascii_alphabet`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L11) · [#26](https://github.com/Luthics/anix/issues/26) |
| [§3.3-03](../docs/RFC.md?plain=1#L124) | Provider 名必须已定义 | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) · [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L538) |
| [§3.3-04](../docs/RFC.md?plain=1#L125) | model 类型、透传与默认 | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) · [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L122) |
| [§3.3-05](../docs/RFC.md?plain=1#L127) | shell 路径或命令名 | [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L576) |
| [§3.3-06](../docs/RFC.md?plain=1#L130) | max_tokens 范围及硬上限退出 | [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L472) · [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) |
| [§3.3-07](../docs/RFC.md?plain=1#L131) | 单轮输出必须小于上下文预算，0 时省略 | [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L364) · [`builds_responses_request_with_frozen_developer_prefix_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L644) |
| [§3.3-08](../docs/RFC.md?plain=1#L132) | 比例向下取整、绝对阈值与禁用值 | [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L96) · [`absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L718) · [#5](https://github.com/Luthics/anix/issues/5) · [#24](https://github.com/Luthics/anix/issues/24) |
| [§3.3-09](../docs/RFC.md?plain=1#L133) | approval 枚举及默认值 | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) · [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) [断言 L77](../tests/test_rfc_tables.py#L77) · [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) [断言 L100](../tests/test_rfc_tables.py#L100) [断言 L109](../tests/test_rfc_tables.py#L109) |
| [§3.3-10](../docs/RFC.md?plain=1#L134) | stderr 枚举及默认值 | [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L38) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) · [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) [断言 L77](../tests/test_rfc_tables.py#L77) · [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) [断言 L100](../tests/test_rfc_tables.py#L100) [断言 L109](../tests/test_rfc_tables.py#L109) |
| [§3.3-11](../docs/RFC.md?plain=1#L135) | context 按 cwd 解析 | [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L451) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) |
| [§3.3-12](../docs/RFC.md?plain=1#L136) | params 的扩展键与浅合并 | [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L404) · [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L884) |
| [§3.3-13](../docs/RFC.md?plain=1#L126) | vision 只接受 YAML 布尔值，params.vision 仍可作为普通字符串透传 | [`vision_requires_a_yaml_boolean_and_leaves_provider_params_open`](../crates/anix-core/tests/rfc_3_2/cli.rs#L181) |
| [§3.3-14](../docs/RFC.md?plain=1#L128) | output 枚举与默认值在 CLI、Frontmatter 及首条 WAL 配置中一致 | [`test_cli_enum_domains_equal_rfc_option_and_frontmatter_sets`](../tests/test_rfc_tables.py#L57) [断言 L77](../tests/test_rfc_tables.py#L77) · [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) [断言 L100](../tests/test_rfc_tables.py#L100) [断言 L109](../tests/test_rfc_tables.py#L109) |

## §3.4

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§3.4-01](../docs/RFC.md?plain=1#L156) | 嵌套子进程继承实际生效的 auto 或 deny，模型、WAL 和轮数预算独立 | [`test_nested_anix_uses_own_model_wal_and_budget_and_inherits_cli_auto`](../tests/test_shell_pipeline.py#L640) · [`test_nested_anix_inherits_cli_deny_over_its_own_auto_frontmatter`](../tests/test_shell_pipeline.py#L644) |
| [§3.4-02](../docs/RFC.md?plain=1#L141) | 优先级各层的 32 种子集 | [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L122) · [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L192) |
| [§3.4-03](../docs/RFC.md?plain=1#L147) | 环境变量、布尔别名及非法值 | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L595) · [`invalid_resolved_environment_values_exit_two_before_contacting_the_provider`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L643) |
| [§3.4-04](../docs/RFC.md?plain=1#L148) | 显式空串覆盖会话路径 | [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L451) |
| [§3.4-05](../docs/RFC.md?plain=1#L154) | 清除父实例状态、保留观测变量与普通环境变量 | [`shell_environment_contains_only_observability_effective_approval_and_routed_fds`](../crates/anix-core/tests/rfc_runtime/io.rs#L295) |
| [§3.4-06](../docs/RFC.md?plain=1#L157) | fd 4–6 的环境映射 | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L154) |
| [§3.4-07](../docs/RFC.md?plain=1#L159) | 跨配置层保留无冲突键、高层整体替换同名子对象 | [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L404) · [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L884) |
| [§3.4-08](../docs/RFC.md?plain=1#L153) | 脚本正文引用未挂载的 ANIX_OUT 时给出迁移提示 | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§3.4-09](../docs/RFC.md?plain=1#L151) | 无 shell 或禁止执行时请求工具直通，启动失败且不写 WAL | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§3.4-10](../docs/RFC.md?plain=1#L150) | 未知 output、approval 或 stderr 在 CLI 与 Frontmatter 两条路径都返回 2，WAL 和 Provider 请求均不存在 | [`test_effective_enum_defaults_choices_and_rejections_match_rfc`](../tests/test_rfc_tables.py#L83) [断言 L115](../tests/test_rfc_tables.py#L115) [断言 L118](../tests/test_rfc_tables.py#L118) |
| [§3.4-11](../docs/RFC.md?plain=1#L152) | 配置无法解析的 shell 时返回 2，目标 WAL 不被创建 | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) · [`test_shell_resolution_requires_an_executable_file_before_any_provider_request`](../tests/test_output_contract.py#L74) [断言 L92](../tests/test_output_contract.py#L92) |
| [§3.4-12](../docs/RFC.md?plain=1#L141) | output 的五个覆盖层各自可缺省，最终 stdout、WAL 快照和请求 IR 采用同一优先级 | [`test_output_resolves_every_subset_of_cli_env_script_project_and_global`](../tests/test_output_contract.py#L17) [断言 L50](../tests/test_output_contract.py#L50) |
| [§3.4-13](../docs/RFC.md?plain=1#L153) | 两种 shell 能力、三种 output 和三种 approval 组合分别核实退出状态、诊断、WAL 与 Provider 请求，包括正文引用 ANIX_OUT 的情形 | [`test_output_startup_matrix_warns_or_stops_before_wal_and_provider`](../tests/test_output_contract.py#L54) [断言 L69](../tests/test_output_contract.py#L69) |

## §4.1

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§4.1-01](../docs/RFC.md?plain=1#L171) | 有参数的可执行脚本通过 PATH 或相对路径启动，TTY 保持打开即可推理 | [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L251) |
| [§4.1-02](../docs/RFC.md?plain=1#L171) | 无参数且 stdin 为终端时等待实际 EOF，再将输入交给 Provider | [`test_executable_script_on_path_and_by_relative_name_obeys_tty_eof_rules`](../tests/test_provider_streaming.py#L445) · [#34](https://github.com/Luthics/anix/issues/34) |
| [§4.1-03](../docs/RFC.md?plain=1#L171) | 带参数启动，Provider 首帧前是否必须显示等待提示 | [#36](https://github.com/Luthics/anix/issues/36) |
| [§4.1-04](../docs/RFC.md?plain=1#L172) | output=explicit 时重定向与真实管道只转发显式 Payload，保留原始字节 | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) · [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) |
| [§4.1-05](../docs/RFC.md?plain=1#L181) | 末尾无换行的模型文本与使用量行分行显示 | [`trace_fd_explicit_only_no_implicit_fd3_and_cycle_events`](../crates/anix-core/tests/v1_1_e2e.rs#L164) |
| [§4.1-06](../docs/RFC.md?plain=1#L181) | 流式正文和思考帧在工具状态、诊断侧信道及使用量汇总处的换行，显式模式只输出诊断 | [`unterminated_deltas_are_separated_from_tool_status_diagnostics_and_usage`](../crates/anix-core/tests/rfc_runtime/io.rs#L407) |
| [§4.1-07](../docs/RFC.md?plain=1#L182) | 在真实 TTY 上精确检查 Dim 使用量行在最后一个换行前重置 ANSI 样式 | [`test_usage_line_resets_dim_before_the_final_newline_on_a_terminal`](../tests/test_distribution.py#L219) |
| [§4.1-08](../docs/RFC.md?plain=1#L175) | 可读写的显式 Trace FD 不进入静态输入 | [`each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input`](../crates/anix-core/tests/rfc_runtime/fd.rs#L94) |
| [§4.1-09](../docs/RFC.md?plain=1#L174) | 显式 FD 校验失败时先返回 2，有效时 CLI 优先于环境配置 | [`only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment`](../crates/anix-core/tests/rfc_runtime/fd.rs#L61) · [`invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider`](../crates/anix-core/tests/rfc_runtime/fd.rs#L136) |
| [§4.1-10](../docs/RFC.md?plain=1#L171) | 真实管道有无脚本参数均等到 EOF，保留跨写入的 UTF-8 及内嵌 NUL | [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L333) |
| [§4.1-11](../docs/RFC.md?plain=1#L175) | 逐个通过真实管道连接 fd 4 至 9，读完后保留输入来源和完整正文 | [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L333) |
| [§4.1-12](../docs/RFC.md?plain=1#L172) | stdout 与 stderr 各自为终端或普通文件的四种组合，测试 auto 和 explicit | [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) |
| [§4.1-13](../docs/RFC.md?plain=1#L175) | fd 4–9 访问模式、拼接顺序与 EOF | [`args_stdin_and_sparse_extra_descriptors_are_ordered_and_trace_is_separate`](../crates/anix-core/tests/rfc_runtime/io.rs#L35) · [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L222) · [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L333) |
| [§4.1-14](../docs/RFC.md?plain=1#L185) | Provider、钩子、shell 不能继承已消费的文件 | [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L222) |
| [§4.1-15](../docs/RFC.md?plain=1#L171) | 管道输入含非法 UTF-8 时如何构造 JSON 用户消息 | [#12](https://github.com/Luthics/anix/issues/12) |
| [§4.1-16](../docs/RFC.md?plain=1#L172) | 默认只向管道交付最终回复，禁用工具时可串联两级脚本 | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§4.1-17](../docs/RFC.md?plain=1#L172) | explicit 逐字节交付工具输出，none 保持零字节并允许磁盘副作用 | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§4.1-18](../docs/RFC.md?plain=1#L172) | 带工具的轮次及失败轮次的部分文本进入诊断流，stdout 只交付成功终轮的回复 | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§4.1-19](../docs/RFC.md?plain=1#L172) | 在 Provider 尚不能发出终结帧时，观察纯文本首段与启用 shell 的缓冲行为，覆盖两种 Provider 模式 | [`test_reply_visibility_before_done_in_oneshot_and_coprocess`](../tests/test_output_contract.py#L96) [断言 L149](../tests/test_output_contract.py#L149) [断言 L155](../tests/test_output_contract.py#L155) |
| [§4.1-20](../docs/RFC.md?plain=1#L172) | stdout 连接 PTY 和管道时分别核对空文本、非空文本、一个或多个尾部 LF | [`test_reply_newline_depends_on_stdout_tty_and_last_byte`](../tests/test_output_contract.py#L158) [断言 L183](../tests/test_output_contract.py#L183) |
| [§4.1-21](../docs/RFC.md?plain=1#L172) | 纯文本输出后遇到错误、中断或重试时，已交付字节的保留和重发语义待裁决 | [#61](https://github.com/Luthics/anix/issues/61) |
| [§4.1-22](../docs/RFC.md?plain=1#L173) | 比较真实终端和文件中的 auto 与 explicit 诊断流，核对工具诊断、思考及 usage 可见性 | [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) |

## §4.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§4.2-01](../docs/RFC.md?plain=1#L200) | 当前工具批次闭合后才取下一条已发布消息 | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L21) |
| [§4.2-02](../docs/RFC.md?plain=1#L202) | 成功消息先提交，下一条被中断时仍可识别已完成的轮次 | [`aclr_auto_summary_tail_fold_and_coprocess_inbox_lifecycle`](../crates/anix-core/tests/v1_1_e2e.rs#L600) · [`test_inbox_commits_one_message_then_interrupts_the_next_in_the_same_coprocess`](../tests/test_provider_streaming.py#L592) · [#37](https://github.com/Luthics/anix/issues/37) |
| [§4.2-03](../docs/RFC.md?plain=1#L205) | Linux 空闲时只发一次 idle，新任务结束后重新待命 | [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L340) |
| [§4.2-04](../docs/RFC.md?plain=1#L197) | 新建四个子目录及根目录的权限与就绪时间 | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L21) · [`a_publisher_can_queue_work_while_the_initial_cli_task_is_running`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L122) · [#21](https://github.com/Luthics/anix/issues/21) |
| [§4.2-05](../docs/RFC.md?plain=1#L198) | 多发送方只发布完整文件 | [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L21) |
| [§4.2-06](../docs/RFC.md?plain=1#L206) | 上一任务结束后才开始计算闲置超时并释放 WAL 锁 | [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L182) |
| [§4.2-07](../docs/RFC.md?plain=1#L201) | 进程重启时如何处理已领取且仍留在 cur/ 的消息 | [#2](https://github.com/Luthics/anix/issues/2) |

## §4.3

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§4.3-01](../docs/RFC.md?plain=1#L214) | output=explicit 时真实 msh 子进程的标准输入指向空设备，fd 4、5、6 都是只写匿名管道 | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) |
| [§4.3-02](../docs/RFC.md?plain=1#L216) | 多次打开输出管道，超过 1 MiB 的二进制 Payload 逐字节完整转发 | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) |
| [§4.3-03](../docs/RFC.md?plain=1#L217) | 诊断管道可独立传输超过 1 MiB 的二进制数据 | [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) |
| [§4.3-04](../docs/RFC.md?plain=1#L218) | 控制管道写端与实际 JSONL 帧 | [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L28) · [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L467) |
| [§4.3-05](../docs/RFC.md?plain=1#L220) | 连续工具重定向到普通文件时顺序追加 | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) |
| [§4.3-06](../docs/RFC.md?plain=1#L216) | reply 模式下真实 shell 观察到 fd 4 关闭且 ANIX_OUT 缺失 | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§4.3-07](../docs/RFC.md?plain=1#L216) | 三个 output 模式下 fd 4 的开闭与 ANIX_OUT 一致，fd 5、6 保持只写匿名管道 | [`test_output_mode_controls_payload_fd_without_changing_diagnostic_or_control_pipes`](../tests/test_shell_pipeline.py#L73) [断言 L102](../tests/test_shell_pipeline.py#L102) |
| [§4.3-08](../docs/RFC.md?plain=1#L215) | msh 子进程的 stdout 和 stderr 均进入模型工具结果，外部 quiet 诊断流保持干净 | [`test_output_mode_controls_payload_fd_without_changing_diagnostic_or_control_pipes`](../tests/test_shell_pipeline.py#L73) [断言 L97](../tests/test_shell_pipeline.py#L97) |

## §4.4

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§4.4-01](../docs/RFC.md?plain=1#L224) | 同名全局或已信任项目钩子可覆写和屏蔽 | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) |
| [§4.4-02](../docs/RFC.md?plain=1#L228) | output=explicit、stderr=auto 首轮提示中的交付通道和工具，实际 fd 2 可见性 | [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L474) · [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) |
| [§4.4-03](../docs/RFC.md?plain=1#L228) | output=explicit、stderr=explicit 的首轮契约、切换 stderr 后的冻结前缀及输出 | [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L474) · [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L510) |
| [§4.4-04](../docs/RFC.md?plain=1#L226) | shell=none 的系统提示声明无工具，省略 runtime_tips 与 available_skills | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |

## §5.1

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§5.1-01](../docs/RFC.md?plain=1#L243) | 自动放行和其环境变量别名，外部裁决器不被调用 | [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L595) · [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L327) |
| [§5.1-02](../docs/RFC.md?plain=1#L244) | deny 保留白名单，拦截时返回工具错误，开启 fail-on-deny 时退出 75 | [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L327) · [#9](https://github.com/Luthics/anix/issues/9) |
| [§5.1-03](../docs/RFC.md?plain=1#L246) | Always 对同一可执行程序生效，换进程读取同一 WAL 后需重新审批 | [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L158) |

## §5.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§5.2-01](../docs/RFC.md?plain=1#L264) | 外部裁决器读到一行 JSON、真实进程标识及 EOF | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L253) |
| [§5.2-02](../docs/RFC.md?plain=1#L274) | 外部返回 0，无控制终端也能执行一次已批准的命令 | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L253) |
| [§5.2-03](../docs/RFC.md?plain=1#L275) | 外部返回 1，裁剪拒绝理由、为空时给出默认值，继续推理 | [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L272) |
| [§5.2-04](../docs/RFC.md?plain=1#L276) | 返回 2 后交还终端裁决，无控制终端时退出 75 | [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L223) · [`test_external_escalation_hands_the_decision_to_the_terminal`](../tests/test_approval.py#L238) |
| [§5.2-05](../docs/RFC.md?plain=1#L277) | 其他非零或超时透出诊断，结束当前轮且不执行工具 | [`test_external_error_and_timeout_surface_stderr_and_end_the_cycle_with_75`](../tests/test_approval.py#L292) · [`test_unavailable_external_decider_exits_75_and_records_the_failed_approval`](../tests/test_approval.py#L314) |
| [§5.2-06](../docs/RFC.md?plain=1#L279) | 普通放行、拒绝及被信号中断时记录裁决来源 | [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L253) · [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L272) · [`test_interrupt_during_builtin_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L361) · [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L388) · [#50](https://github.com/Luthics/anix/issues/50) |
| [§5.2-07](../docs/RFC.md?plain=1#L279) | 退出码 2 转交人工、超时或启动失败时如何表示外部退出码 | [#51](https://github.com/Luthics/anix/issues/51) |

## §5.3

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§5.3-01](../docs/RFC.md?plain=1#L284) | 提示与回答走控制终端，管道 stdin 保留为模型输入 | [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L132) |
| [§5.3-02](../docs/RFC.md?plain=1#L286) | y 只批准一条，同批下一条可单独拒绝 | [`test_yes_authorizes_only_one_call_in_a_batch`](../tests/test_approval.py#L208) |
| [§5.3-03](../docs/RFC.md?plain=1#L287) | n 返回模型错误，支持空理由及 UTF-8 文本 | [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L132) |
| [§5.3-04](../docs/RFC.md?plain=1#L288) | a 的授权以可执行程序及 PID 为界 | [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L158) |
| [§5.3-05](../docs/RFC.md?plain=1#L289) | e 使用私有临时文件，执行编辑结果并清理，编辑取消或为空时两条命令均未执行 | [`test_editor_receives_private_original_and_only_the_edited_command_runs`](../tests/test_approval.py#L169) · [`test_failed_or_empty_edit_removes_the_file_and_runs_neither_command`](../tests/test_approval.py#L196) · [#10](https://github.com/Luthics/anix/issues/10) |

## §5.4

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§5.4-01](../docs/RFC.md?plain=1#L293) | 无控制终端时打印原命令、记录终止事件并退出 75 | [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L223) |

## §5.5

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§5.5-01](../docs/RFC.md?plain=1#L299) | 依次覆盖 CLI、环境、脚本、所选 Provider、全局默认和内置默认 | [`vision_resolves_from_cli_env_script_selected_provider_then_defaults`](../crates/anix-core/tests/rfc_runtime/vision.rs#L10) |
| [§5.5-02](../docs/RFC.md?plain=1#L304) | 无原生视觉且无桥接器时，工具报错且附件不进入恢复的历史 | [`no_native_vision_without_a_bridge_returns_the_documented_error_through_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L181) · [`vision_false_blocks_wal_pollution_and_vision_bridge_translates_and_mcp_tip_present`](../crates/anix-core/tests/v1_1_e2e.rs#L466) |
| [§5.5-03](../docs/RFC.md?plain=1#L313) | 桥接器接收图片信息和 EOF，恢复会话后保留转译描述 | [`bridge_gets_one_utf8_json_line_then_eof_and_its_description_survives_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L191) |
| [§5.5-04](../docs/RFC.md?plain=1#L318) | 桥接成功的正文边界、空输出及无效 UTF-8 的裁决 | [#43](https://github.com/Luthics/anix/issues/43) |
| [§5.5-05](../docs/RFC.md?plain=1#L319) | 视觉桥接非零退出或超时成为可恢复的工具错误 | [`bridge_nonzero_exit_and_timeout_both_return_stderr_as_a_recoverable_tool_error`](../crates/anix-core/tests/rfc_runtime/vision.rs#L218) |
| [§5.5-06](../docs/RFC.md?plain=1#L321) | 上游拒绝媒体时产生专属且不可重试的错误 | [`http_400_image_rejection_maps_to_unsupported_media_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L620) · [`test_upstream_media_rejection_is_distinct_from_auth_and_transient_failures`](../tests/test_provider_dialects.py#L220) |

## §6.1

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§6.1-01](../docs/RFC.md?plain=1#L351) | 从同批工具的消息锚点折叠时回退到对应 assistant | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) |
| [§6.1-02](../docs/RFC.md?plain=1#L333) | 折叠时沿用活跃轮次编号，耗尽 uint64 后的行为待裁决 | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) · [`running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L404) · [#18](https://github.com/Luthics/anix/issues/18) |
| [§6.1-03](../docs/RFC.md?plain=1#L336) | 最后存活回答的 usage 与后续字节估算 | [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L101) · [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L579) · [#5](https://github.com/Luthics/anix/issues/5) · [#20](https://github.com/Luthics/anix/issues/20) |
| [§6.1-04](../docs/RFC.md?plain=1#L337) | 首轮及 checkpoint 后估算、排除压缩器用量 | [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) · [`fresh_window_counts_utf8_bytes_in_groups_of_three`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L454) · [#5](https://github.com/Luthics/anix/issues/5) |
| [§6.1-05](../docs/RFC.md?plain=1#L338) | fold 后的水位回落 | [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L579) |
| [§6.1-06](../docs/RFC.md?plain=1#L339) | 等于硬上限允许、超过后停止 | [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L472) · [`auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L502) |
| [§6.1-07](../docs/RFC.md?plain=1#L342) | 显式 compact 写入新分节并刷新系统提示词 | [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) |
| [§6.1-08](../docs/RFC.md?plain=1#L343) | 有效范围、前缀保存、剪去发起调用的轮次 | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) · [#3](https://github.com/Luthics/anix/issues/3) |
| [§6.1-09](../docs/RFC.md?plain=1#L350) | 无参 aclr 输出用法并退出 2 | [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L78) |
| [§6.1-10](../docs/RFC.md?plain=1#L354) | 外部压缩器只收 IR、输出须为非空 UTF-8、故障退出 | [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L385) |
| [§6.1-11](../docs/RFC.md?plain=1#L355) | provider、model、提示词文件与内置回退 | [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) · [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L563) |
| [§6.1-12](../docs/RFC.md?plain=1#L356) | 压缩请求没有工具 | [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L385) · [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L563) |
| [§6.1-13](../docs/RFC.md?plain=1#L333) | 工具退出 0、7、124 时，当前 IR、落盘 WAL 和重新加载的 IR 都含正确的轮次、退出码与过滤后文本 | [`post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L602) |
| [§6.1-14](../docs/RFC.md?plain=1#L346) | 外部自动摘要器收到无工具的对齐子窗口，摘要指令在 IR 中的位置待裁定 | [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L894) · [#42](https://github.com/Luthics/anix/issues/42) |
| [§6.1-15](../docs/RFC.md?plain=1#L348) | 自动摘要失败或为空时保留旧窗口，向模型返回工具错误 | [`empty_or_failed_auto_summary_keeps_the_subwindow_and_reports_a_tool_error`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L924) |

## §6.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§6.2-01](../docs/RFC.md?plain=1#L369) | 新建会话在压缩、路径别名与恢复后仍使用原 session_id | [`persisted_id_survives_builtin_compaction_checkpoint_alias_and_subsequent_resume`](../crates/anix-core/tests/rfc_runtime/session.rs#L122) · [`test_v1_1_1_session_id_persists_across_wal_resume_checkpoint_and_isolates_msh`](../crates/anix-core/tests/v1_1_e2e.rs#L809) |
| [§6.2-02](../docs/RFC.md?plain=1#L369) | 无状态运行每进程生成 UUID v4，经过推理和压缩仍一致 | [`stateless_runs_get_distinct_uuid_v4s_while_all_provider_calls_in_a_run_share_one`](../crates/anix-core/tests/rfc_runtime/session.rs#L100) · [`empty_context_overrides_a_saved_wal_and_keeps_one_ephemeral_id_through_compaction`](../crates/anix-core/tests/rfc_runtime/session.rs#L350) |
| [§6.2-03](../docs/RFC.md?plain=1#L369) | 旧 WAL 的回退值经路径别名与重启稳定，写入 checkpoint 后随文件迁移 | [`replay_restores_session_id_across_checkpoint_and_derives_stable_legacy_fallback`](../crates/anix-core/src/wal.rs#L859) · [`legacy_wal_uses_a_stable_canonical_path_id_across_aliases_and_restarts`](../crates/anix-core/tests/rfc_runtime/session.rs#L161) · [`first_checkpoint_in_a_legacy_wal_persists_the_fallback_id_even_if_the_wal_moves`](../crates/anix-core/tests/rfc_runtime/session.rs#L314) |
| [§6.2-04](../docs/RFC.md?plain=1#L394) | 最新 checkpoint 提供的标识原样到达 Trace、环境和协进程信封 | [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L196) · [`inbox_keeps_one_session_id_for_both_messages_in_a_single_coprocess`](../crates/anix-core/tests/rfc_runtime/session.rs#L277) |
| [§6.2-05](../docs/RFC.md?plain=1#L366) | 从 session_init 和 checkpoint 恢复，stderr、approval、vision 的八种变更组合只在首次变化时产生一条尾部通知 | [`repeated_resume_appends_one_vision_update_per_change_and_reuses_frozen_text`](../crates/anix-core/tests/rfc_runtime/wal.rs#L62) · [`each_combination_of_stderr_approval_and_vision_drift_creates_one_tail_update`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L645) |
| [§6.2-06](../docs/RFC.md?plain=1#L366) | 旧 runtime_update 只提供部分字段时，缺失的契约配置要从哪里恢复 | [#17](https://github.com/Luthics/anix/issues/17) |
| [§6.2-07](../docs/RFC.md?plain=1#L389) | 文件增长受限时保留原有字节，只在尾行完整可回放时继续 | [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L158) |
| [§6.2-08](../docs/RFC.md?plain=1#L388) | 活跃窗口固定，归档从 1 MiB 增至 32 MiB，读取量维持在误差范围内且 Provider 收到相同字节 | [`checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L109) · [`test_archived_wal_growth_keeps_resume_reads_bounded_at_the_latest_checkpoint`](../tests/test_shell_pipeline.py#L203) |
| [§6.2-09](../docs/RFC.md?plain=1#L362) | 独占非阻塞锁、冲突时退出并诊断 | [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L198) |
| [§6.2-10](../docs/RFC.md?plain=1#L365) | session_init、冻结 system、恢复时复用 | [`legacy_wal_without_vision_or_fold_usage_resumes_without_rewriting_the_prefix`](../crates/anix-core/tests/rfc_runtime/wal.rs#L26) · [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L218) · [`initial_section_interruption_never_dispatches_without_a_frozen_system_message`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L524) · [#19](https://github.com/Luthics/anix/issues/19) |
| [§6.2-11](../docs/RFC.md?plain=1#L367) | 只在全量压缩时重新运行钩子 | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) · [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L447) · [#19](https://github.com/Luthics/anix/issues/19) |
| [§6.2-12](../docs/RFC.md?plain=1#L368) | WAL 每行提交、短写、五字段累计和跨分节恢复 | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) · [`wal_events_round_trip`](../crates/anix-proto/src/wal.rs#L105) · [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L158) · [#8](https://github.com/Luthics/anix/issues/8) |
| [§6.2-13](../docs/RFC.md?plain=1#L389) | 逐字节截断、损坏时保留原件 | [`every_partial_byte_write_of_a_final_record_fails_without_modifying_history`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L35) · [`resuming_a_complete_last_object_without_lf_never_joins_two_wal_records`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L59) · [#13](https://github.com/Luthics/anix/issues/13) |
| [§6.2-14](../docs/RFC.md?plain=1#L391) | 多次重叠 fold 的剪枝水位与顺序 | [`overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L137) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) |
| [§6.2-15](../docs/RFC.md?plain=1#L392) | 被剪枝回答的 usage 仍进入会话总量 | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) |
| [§6.2-16](../docs/RFC.md?plain=1#L393) | 回放剔除未闭合的工具调用，保留已提交的用量 | [`replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage`](../crates/anix-core/src/wal.rs#L748) · [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L411) |
| [§6.2-17](../docs/RFC.md?plain=1#L394) | 分节版本检查与前缀和 | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) · [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L174) |
| [§6.2-18](../docs/RFC.md?plain=1#L395) | 正序回放与前向参考模型逐轮对照 | [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) |
| [§6.2-19](../docs/RFC.md?plain=1#L366) | 恢复旧 WAL 时默认 reply 在尾部通知原 explicit 契约发生变化 | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§6.2-20](../docs/RFC.md?plain=1#L366) | 恢复时比较 output 和 shell 启用状态；换用另一个可执行 shell 不触发漂移，同时改变两项只追加一条 | [`test_resume_distinguishes_shell_capability_from_executable_path_and_output`](../tests/test_output_contract.py#L186) [断言 L206](../tests/test_output_contract.py#L206) [断言 L211](../tests/test_output_contract.py#L211) [断言 L215](../tests/test_output_contract.py#L215) |

## §6.3

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§6.3-01](../docs/RFC.md?plain=1#L399) | 无截止时间的 Linux 空闲循环持续睡眠，发布消息可以唤醒 | [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L340) |
| [§6.3-02](../docs/RFC.md?plain=1#L403) | 子进程清理与退出钩子的截止时间在正常和缺少 pidfd 的 Linux 上可检查 | [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L158) · [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L178) · [#48](https://github.com/Luthics/anix/issues/48) |
| [§6.3-03](../docs/RFC.md?plain=1#L399) | 强杀时组长退出、有限子进程仍在，Linux 宽限期的等待不应持续空转 | [`test_hard_kill_grace_waits_for_a_living_descendant_without_repeated_polling`](../tests/test_msh_jobs.py#L390) · [#57](https://github.com/Luthics/anix/issues/57) |

## §6.4

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§6.4-01](../docs/RFC.md?plain=1#L409) | Ctrl+C 在等待 Provider 或其关闭 stdout 后回到可继续执行管道的同一 shell | [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/anix-core/tests/rfc_runtime/signals.rs#L12) · [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L495) · [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L552) · [#35](https://github.com/Luthics/anix/issues/35) |
| [§6.4-02](../docs/RFC.md?plain=1#L409) | 在内置审批处只按一次 Ctrl+C，命令不执行，原 shell 恢复并读到退出码 130 | [`test_ctrl_c_at_the_approval_prompt_returns_to_the_same_shell`](../tests/test_approval.py#L353) · [#49](https://github.com/Luthics/anix/issues/49) |
| [§6.4-03](../docs/RFC.md?plain=1#L410) | 外部审批中收到 SIGTERM，退出 143 并记录中断 | [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L388) · [#50](https://github.com/Luthics/anix/issues/50) |
| [§6.4-04](../docs/RFC.md?plain=1#L409) | 工具输出一部分后中断，shell 后代收到 TERM、释放 WAL 锁，恢复时没有半截工具结果 | [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L411) |
| [§6.4-05](../docs/RFC.md?plain=1#L411) | 保持模式进程收到 STOP 后暂停，CONT 恢复后继续响应快照请求 | [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L51) |
| [§6.4-06](../docs/RFC.md?plain=1#L412) | USR1 在下次推理前执行一次无工具的上下文压缩 | [`usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference`](../crates/anix-core/tests/rfc_runtime/signals.rs#L118) |
| [§6.4-07](../docs/RFC.md?plain=1#L413) | USR2 输出快照路径，文件权限为 0600，内容与活跃 IR 相符 | [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L51) |

## §6.5

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§6.5-01](../docs/RFC.md?plain=1#L425) | 显式强杀超时返回模型错误，长时间运行的命令本身不耗掉下一次 Provider 请求的时限 | [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L231) · [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L264) |
| [§6.5-02](../docs/RFC.md?plain=1#L426) | 读 stdin 或额外输入管道期间取消，写端仍打开也能返回 shell 可见的 130 或 143 | [`test_cancelling_input_does_not_need_eof_or_dispatch_the_partial_message`](../tests/test_shell_pipeline.py#L359) |
| [§6.5-03](../docs/RFC.md?plain=1#L421) | 正常结束返回 0，普通文件中保留完整的显式输出 | [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L90) |
| [§6.5-04](../docs/RFC.md?plain=1#L422) | Provider 提前 EOF、WAL 文件增长失败和锁冲突分别以 1 结束 | [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/anix-core/tests/rfc_runtime/provider.rs#L147) · [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L198) · [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L158) |
| [§6.5-05](../docs/RFC.md?plain=1#L423) | 脚本编码错误、损坏的配置及不支持的 WAL 版本在推理前以 2 结束 | [`invalid_file_bytes_and_yaml_report_usage_errors_to_stderr`](../crates/anix-core/tests/rfc_3_2/cli.rs#L90) · [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L671) · [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L174) |
| [§6.5-06](../docs/RFC.md?plain=1#L424) | 无控制终端需人工审批及未信任的项目裁决器以 75 结束 | [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L297) · [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L223) |
| [§6.5-07](../docs/RFC.md?plain=1#L425) | 最后一轮结束仍有工作、首轮超过硬上限或 Provider 超时均以 124 结束 | [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L472) · [`provider_timeout_maps_to_124_without_a_retry`](../crates/anix-core/tests/rfc_runtime/provider.rs#L158) · [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L235) |
| [§6.5-08](../docs/RFC.md?plain=1#L425) | 默认超时返回后台句柄并继续任务，显式强杀返回工具错误并继续推理 | [`test_v1_1_2_msh_per_call_timeout_yield_bg_notification_and_kill_on_timeout`](../crates/anix-core/tests/v1_1_e2e.rs#L1033) |
| [§6.5-09](../docs/RFC.md?plain=1#L426) | 在 Provider 正等待时发送 SIGINT 或 SIGTERM，退出状态同时符合 RFC 示例和 128+N | [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) [断言 L158](../tests/test_rfc_tables.py#L158) [断言 L161](../tests/test_rfc_tables.py#L161) |
| [§6.5-10](../docs/RFC.md?plain=1#L421) | 一次普通模型回复成功结束，进程状态码等于 EX_OK | [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) [断言 L161](../tests/test_rfc_tables.py#L161) |
| [§6.5-11](../docs/RFC.md?plain=1#L422) | 模拟 Provider 返回不可重试的错误，进程状态码等于 GENERAL_ERR | [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) [断言 L161](../tests/test_rfc_tables.py#L161) |
| [§6.5-12](../docs/RFC.md?plain=1#L423) | 传入未声明的命令行选项，进程状态码等于 EX_USAGE | [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) [断言 L161](../tests/test_rfc_tables.py#L161) |
| [§6.5-13](../docs/RFC.md?plain=1#L424) | deny 拒绝工具且启用 --fail-on-deny 时，进程状态码等于 EX_TEMPFAIL | [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) [断言 L161](../tests/test_rfc_tables.py#L161) |
| [§6.5-14](../docs/RFC.md?plain=1#L425) | 工具轮结束且用尽 max_turns 时，进程状态码等于 EX_TIMEOUT | [`test_process_exit_statuses_match_every_rfc_table_row`](../tests/test_rfc_tables.py#L126) [断言 L161](../tests/test_rfc_tables.py#L161) |

## §7.1

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§7.1-01](../docs/RFC.md?plain=1#L455) | 独立 msh 显式强杀时向整组发 TERM，2 秒宽限后强制回收并返回 124 | [`test_msh_timeout_signals_the_whole_group_and_grants_two_seconds_before_kill`](../tests/test_shell_pipeline.py#L372) |
| [§7.1-02](../docs/RFC.md?plain=1#L436) | 六个变量强制生效、工具目录进入 PATH | [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L154) · [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L220) |
| [§7.1-03](../docs/RFC.md?plain=1#L437) | 8 KiB NUL 边界与全流 UTF-8 校验 | [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L35) · [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L244) |
| [§7.1-04](../docs/RFC.md?plain=1#L439) | 探测 MIME、保存原始字节、路径和 0600 | [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L35) · [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L244) |
| [§7.1-05](../docs/RFC.md?plain=1#L440) | 二进制输出单行说明 | [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L244) |
| [§7.1-06](../docs/RFC.md?plain=1#L441) | CSI 终结字节、OSC 终止方式和回车覆盖 | [`ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites`](../crates/msh/tests/rfc_shell.rs#L66) |
| [§7.1-07](../docs/RFC.md?plain=1#L442) | 默认双阈值、环境覆盖、首尾 20 行和完整清洗日志 | [`line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text`](../crates/msh/tests/rfc_shell.rs#L107) · [`byte_limit_independent_from_line_limit_preserves_multibyte_file`](../crates/msh/tests/rfc_shell.rs#L143) · [`cli_applies_environment_limits_to_cleaned_lines_and_bytes_before_saving_output`](../crates/msh/tests/rfc_shell.rs#L178) · [`unconfigured_msh_folds_only_above_200_lines_or_8192_bytes`](../crates/msh/tests/rfc_shell.rs#L283) · [#15](https://github.com/Luthics/anix/issues/15) |
| [§7.1-08](../docs/RFC.md?plain=1#L446) | 普通命令退出 7 时，用于 Provider 的 is_error 布尔值是否有规定 | [#55](https://github.com/Luthics/anix/issues/55) |
| [§7.1-09](../docs/RFC.md?plain=1#L444) | 独立 msh 的环境变量配置前台等待时长及显式强杀 | [`timeout_reports_124_before_a_finite_longer_command_completes`](../crates/msh/tests/rfc_shell.rs#L269) |
| [§7.1-10](../docs/RFC.md?plain=1#L444) | 单次请求依次覆盖缺省、零、正数、上限两侧与回到默认，自定义 shell 收到 -c、原命令、策略变量及任务目录 | [`test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call`](../tests/test_msh_jobs.py#L255) |
| [§7.1-11](../docs/RFC.md?plain=1#L446) | 前台成功与普通非零退出保留双流输出、透传真实退出码并清理任务文件 | [`test_foreground_completion_returns_real_status_and_removes_job_files`](../tests/test_msh_jobs.py#L142) |
| [§7.1-12](../docs/RFC.md?plain=1#L448) | 立即后台启动时 fd 4–6 和三个侧信道变量均已关闭或清除，进程组和会话隔离 | [`test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit`](../tests/test_msh_jobs.py#L88) |
| [§7.1-13](../docs/RFC.md?plain=1#L449) | 真实命令等待到期时仅返回清洗后的有限预览，后台继续执行并保留完整原始日志 | [`test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log`](../tests/test_msh_jobs.py#L118) |
| [§7.1-14](../docs/RFC.md?plain=1#L445) | 运行时目录下 jobs 为 0700、日志为 0600，后台退出前不存在最终状态文件，随后读到真实的非零退出码 | [`test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit`](../tests/test_msh_jobs.py#L88) |
| [§7.1-15](../docs/RFC.md?plain=1#L455) | 显式强杀且指定 0 秒时先等待一秒，输出已有文本与超时标记，清理任务文件 | [`test_zero_with_explicit_kill_waits_one_second_and_leaves_no_job`](../tests/test_msh_jobs.py#L182) |
| [§7.1-16](../docs/RFC.md?plain=1#L456) | 前台等待中分别发送 INT、TERM，两种超时策略都清理整组并返回 130、143 | [`test_foreground_int_and_term_clean_up_in_both_timeout_modes`](../tests/test_msh_jobs.py#L151) |
| [§7.1-17](../docs/RFC.md?plain=1#L454) | 正数等待到期后，后一轮推理可以在任务仍存活时运行，随后报告一次完成结果 | [`test_positive_timeout_dispatches_the_next_request_while_the_job_is_alive`](../tests/test_msh_jobs.py#L288) |

## §7.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§7.2-01](../docs/RFC.md?plain=1#L474) | 省略字段时工具发出无 summary 的 fold 帧，运行时生成摘要 | [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L894) · [`aclr_without_a_summary_omits_the_field_at_each_valid_turn_id_boundary`](../crates/anix-tools/tests/rfc_control.rs#L67) · [#41](https://github.com/Luthics/anix/issues/41) |
| [§7.2-02](../docs/RFC.md?plain=1#L461) | 按含 LF 的实际序列化长度逐个比较 aclr 发送端和内核接收端的判定 | [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L106) · [#14](https://github.com/Luthics/anix/issues/14) |
| [§7.2-03](../docs/RFC.md?plain=1#L467) | 四页文档的 15 种非空页集合，从文件、显式和隐式 stdin 输入，逐页核对 PNG 尺寸及色块 | [`aview_selects_pdf_pages_with_their_actual_geometry_and_colors`](../crates/anix-tools/tests/rfc_control.rs#L246) |
| [§7.2-04](../docs/RFC.md?plain=1#L467) | 页码语法错误时以非零状态退出且不发布附件 | [`aview_rejects_invalid_pdf_page_expressions_without_publishing_attachments`](../crates/anix-tools/tests/rfc_control.rs#L315) |
| [§7.2-05](../docs/RFC.md?plain=1#L467) | 重复或乱序页码的输出次序，以及合法页与越界页混用时能否先发布部分附件 | [#54](https://github.com/Luthics/anix/issues/54) |
| [§7.2-06](../docs/RFC.md?plain=1#L460) | 原生工具和图片管道的字节一致性 | [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) |
| [§7.2-07](../docs/RFC.md?plain=1#L461) | fd 6 的 JSONL 跨读取边界与多帧顺序、64 KB 边界 | [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L28) · [`two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order`](../crates/anix-core/tests/rfc_runtime/control.rs#L97) · [`ctl_frames_round_trip`](../crates/anix-proto/src/ctl.rs#L46) · [`fold_control_frames_stay_within_64_kib_even_after_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L97) · [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L106) · [#14](https://github.com/Luthics/anix/issues/14) · [#40](https://github.com/Luthics/anix/issues/40) |
| [§7.2-08](../docs/RFC.md?plain=1#L467) | 不同方向与宽高比图片的尺寸、内容、路径和权限 | [`aview_resizes_portrait_landscape_and_square_images_only_when_needed`](../crates/anix-tools/tests/rfc_control.rs#L149) · [`aview_file_input_and_stdin_agree_on_image_dimensions`](../crates/anix-tools/tests/rfc_control.rs#L172) · [`aview_preserves_image_content_and_orientation_in_the_published_png`](../crates/anix-tools/tests/rfc_control.rs#L187) · [`undecodable_input_or_invalid_options_do_not_emit_media`](../crates/anix-tools/tests/rfc_control.rs#L231) |
| [§7.2-09](../docs/RFC.md?plain=1#L468) | compact 和 fold 的字段与序列化 | [`aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L50) · [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L78) |
| [§7.2-10](../docs/RFC.md?plain=1#L475) | 511 至 66,000 字节的命令都执行到末尾，fd 6 只产生一条有效且有界的 JSONL 帧 | [`test_long_commands_execute_their_suffix_and_handoff_with_a_single_bounded_frame`](../tests/test_msh_jobs.py#L193) |
| [§7.2-11](../docs/RFC.md?plain=1#L475) | 长命令在移交帧和 Trace 的 cmd 字段中是否可以缩短、如何标识预览 | [#58](https://github.com/Luthics/anix/issues/58) |
| [§7.2-12](../docs/RFC.md?plain=1#L479) | 后台任务结束后仅在下一次请求前通知一次，冻结前缀、WAL 和重启恢复结果可核对 | [`test_background_completion_is_reported_once_before_inference_and_replays_from_wal`](../tests/test_msh_jobs.py#L284) |
| [§7.2-13](../docs/RFC.md?plain=1#L479) | 终端标题和已覆盖的进度文本应从后台通知中清除，在 WAL 及后续 Provider 请求中使用清洗后的文本 | [`test_terminal_title_and_overwritten_progress_are_cleaned_from_the_completion_notice`](../tests/test_msh_jobs.py#L292) · [#60](https://github.com/Luthics/anix/issues/60) |

## §8.1

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§8.1-01](../docs/RFC.md?plain=1#L487) | XDG_CONFIG_HOME 与 HOME 回退 | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1031) |
| [§8.1-02](../docs/RFC.md?plain=1#L488) | XDG_STATE_HOME 与 HOME 回退 | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1031) |
| [§8.1-03](../docs/RFC.md?plain=1#L489) | XDG_RUNTIME_DIR、按 UID 回退及权限 | [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1031) · [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L100) |
| [§8.1-04](../docs/RFC.md?plain=1#L490) | 从 cwd 向上寻找最近的 .anix，并在 .git 或 HOME 边界停止 | [`project_search_stops_at_nearest_configuration_git_or_home_boundary`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L485) · [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) |

## §8.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§8.2-01](../docs/RFC.md?plain=1#L532) | 所选 Provider 的 params 浅合并 | [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L884) |
| [§8.2-02](../docs/RFC.md?plain=1#L535) | 内置及外部压缩器选取 | [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L385) · [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L563) |
| [§8.2-03](../docs/RFC.md?plain=1#L506) | 只将硬上限配置为 2 时，每个显式请求仍应裁剪到 2，即使默认等待为 120 | [`test_single_call_never_exceeds_a_configured_maximum_below_the_default_wait`](../tests/test_msh_jobs.py#L270) · [#56](https://github.com/Luthics/anix/issues/56) |
| [§8.2-04](../docs/RFC.md?plain=1#L505) | 环境覆盖 TOML 的默认等待与单次上限，单次覆盖后恢复默认等待 | [`test_environment_maximum_overrides_toml_for_each_explicit_request`](../tests/test_msh_jobs.py#L276) |

## §8.3

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§8.3-01](../docs/RFC.md?plain=1#L548) | 三个内置 pre-context 钩子依次由全局及项目同名文件覆写，也可被空文件或空设备软链屏蔽 | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) |
| [§8.3-02](../docs/RFC.md?plain=1#L550) | 项目级裁决器首次运行需取得信任，再次启动按裁决处理工具 | [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L297) |

## §9.1

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§9.1-01](../docs/RFC.md?plain=1#L572) | IR 不含 model，oneshot 由环境注入，coprocess 改用请求信封 | [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L538) · [`selected_model_changes_the_provider_environment_without_entering_the_ir`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1090) · [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) · [`coprocess_envelope_wraps_the_same_ir_without_moving_model_into_history`](../crates/anix-proto/tests/rfc_ir.rs#L305) |
| [§9.1-02](../docs/RFC.md?plain=1#L573) | 扁平消息字段、附件引用、按行解析 | [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L161) · [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L231) |
| [§9.1-03](../docs/RFC.md?plain=1#L576) | header 必须位于首行，必填字段及其类型受约束 | [`ir_jsonl_header_and_messages_round_trip`](../crates/anix-proto/src/ir.rs#L397) · [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) |
| [§9.1-04](../docs/RFC.md?plain=1#L575) | 不支持的 IR 版本在 HTTP 请求前被检测 | [`unsupported_ir_version_exits_2`](../crates/anix-providers/tests/e2e_mock.rs#L232) · [`test_unsupported_ir_version_is_diagnosed_before_http_in_both_modes`](../tests/test_provider_dialects.py#L276) · [#44](https://github.com/Luthics/anix/issues/44) |
| [§9.1-05](../docs/RFC.md?plain=1#L577) | IR 中 tools 必填，Chat Completions 请求省略空工具集 | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) · [`omits_tools_when_empty_and_supports_disabling_stream_options`](../crates/anix-providers/src/openai_compat.rs#L984) |
| [§9.1-06](../docs/RFC.md?plain=1#L578) | IR 输出上限的省略和显式设置，以及上游 Responses 映射 | [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L364) · [`builds_responses_request_with_frozen_developer_prefix_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L644) |
| [§9.1-07](../docs/RFC.md?plain=1#L579) | params 的 wire 类型及 Responses 参数映射 | [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L72) · [`builds_responses_request_with_frozen_developer_prefix_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L644) |
| [§9.1-08](../docs/RFC.md?plain=1#L580) | meta 的 cwd、stderr_mode、预算 | [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L817) · [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L72) |
| [§9.1-09](../docs/RFC.md?plain=1#L581) | 扩展字段的解码及钩子透传 | [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L11) · [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L161) |
| [§9.1-10](../docs/RFC.md?plain=1#L583) | 冻结系统前缀按各方言映射并保持字节，Responses 使用首条 developer 消息 | [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L112) · [#65](https://github.com/Luthics/anix/issues/65) |
| [§9.1-11](../docs/RFC.md?plain=1#L584) | user 必需字段及可选媒体 | [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L231) · [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L152) |
| [§9.1-12](../docs/RFC.md?plain=1#L585) | assistant 文本或工具调用非空，opaque 与 arguments 类型 | [`validated_ir_only_serializes_objects_for_arguments_and_provider_opaque`](../crates/anix-proto/tests/rfc_ir.rs#L107) · [`assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state`](../crates/anix-proto/tests/rfc_ir.rs#L266) |
| [§9.1-13](../docs/RFC.md?plain=1#L586) | tool 标识、结果和附件 | [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L28) · [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L191) · [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L231) |
| [§9.1-14](../docs/RFC.md?plain=1#L588) | 四种有效媒体类型均从磁盘读取，无效媒体引用的报错方式待裁决 | [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L152) · [#16](https://github.com/Luthics/anix/issues/16) |
| [§9.1-15](../docs/RFC.md?plain=1#L589) | 请求前读取媒体，缺失或不可读时永久失败 | [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L278) · [`test_coprocess_reopens_replaced_media_and_recovers_after_unlink`](../tests/test_provider_dialects.py#L179) · [`test_oneshot_unreadable_and_missing_paths_fail_before_dispatch`](../tests/test_provider_dialects.py#L202) |
| [§9.1-16](../docs/RFC.md?plain=1#L591) | checkpoint 映射为新冻结提示词与 user 摘要 | [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) · [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L447) |
| [§9.1-17](../docs/RFC.md?plain=1#L592) | fold 对齐后插入 user 摘要 | [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L330) · [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L373) |
| [§9.1-18](../docs/RFC.md?plain=1#L593) | 尾部 runtime_update 及上游角色映射 | [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L218) · [`translates_frozen_system_degraded_runtime_update_tools_and_params`](../crates/anix-providers/src/openai_compat.rs#L939) · [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L112) |
| [§9.1-19](../docs/RFC.md?plain=1#L594) | header、工具一一配对且连续、编号顺序与共享锚点 | [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L39) · [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L191) · [`a_single_tool_result_cannot_close_two_calls_with_the_same_id`](../crates/anix-proto/tests/rfc_ir.rs#L219) · [`ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared`](../crates/anix-proto/tests/rfc_ir.rs#L285) · [#39](https://github.com/Luthics/anix/issues/39) |
| [§9.1-20](../docs/RFC.md?plain=1#L599) | msh 的工具声明仅要求 cmd，timeout_sec 为非负整数，kill_on_timeout 为可选布尔值 | [`test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call`](../tests/test_msh_jobs.py#L255) |
| [§9.1-21](../docs/RFC.md?plain=1#L586) | 启用原生视觉时附件随 tool 消息写入 WAL，恢复后保留附件且不调用桥接器 | [`native_vision_keeps_the_attachment_through_resume_and_never_calls_a_bridge`](../crates/anix-core/tests/rfc_runtime/vision.rs#L163) |
| [§9.1-22](../docs/RFC.md?plain=1#L580) | 纯文本请求中 header.tools 为空，meta.output_mode 为 reply | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§9.1-23](../docs/RFC.md?plain=1#L583) | 同一协进程请求的冻结 developer、稳定历史和切换后省略的单轮指令分别核对 | [`test_responses_coprocess_keeps_the_frozen_prefix_and_scopes_dynamic_instructions`](../tests/test_provider_dialects.py#L507) [断言 L534](../tests/test_provider_dialects.py#L534) [断言 L542](../tests/test_provider_dialects.py#L542) |
| [§9.1-24](../docs/RFC.md?plain=1#L583) | 三次真实 CLI 使用一个 WAL，冻结前缀与持久化记录一致，单轮指令只出现在指定请求 | [`test_responses_wal_resume_retains_only_the_frozen_prompt_and_conversation`](../tests/test_provider_dialects.py#L551) [断言 L581](../tests/test_provider_dialects.py#L581) [断言 L587](../tests/test_provider_dialects.py#L587) |

## §9.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§9.2-01](../docs/RFC.md?plain=1#L612) | 父子会话各自路由，子进程完成后父会话保持原标识 | [`nested_anix_gets_its_own_id_and_the_parent_resumes_with_its_original_id`](../crates/anix-core/tests/rfc_runtime/session.rs#L225) |
| [§9.2-02](../docs/RFC.md?plain=1#L624) | 每帧路由覆盖只在本轮生效，省略时回到环境默认 | [`test_session_routing_works_in_oneshot_and_per_envelope_in_coprocess`](../tests/test_provider_dialects.py#L412) · [`test_coprocess_overrides_apply_to_one_frame_then_environment_defaults_resume`](../tests/test_provider_streaming.py#L102) |
| [§9.2-03](../docs/RFC.md?plain=1#L629) | 显式空串和含首尾空白的路由标识如何处理 | [#47](https://github.com/Luthics/anix/issues/47) |
| [§9.2-04](../docs/RFC.md?plain=1#L631) | 三种方言的默认路由头、改名和逐请求禁用 | [`session_affinity_headers_and_body_mapping_across_all_dialects`](../crates/anix-providers/tests/e2e_mock.rs#L679) · [`test_wal_session_reaches_http_across_cli_restarts_and_provider_switches`](../tests/test_provider_dialects.py#L360) · [`test_header_rename_and_disable_apply_only_to_the_current_request`](../tests/test_provider_dialects.py#L438) |
| [§9.2-05](../docs/RFC.md?plain=1#L632) | Responses 原生缓存键支持默认、字符串、布尔值及 null | [`test_responses_cache_key_has_native_string_boolean_and_null_semantics`](../tests/test_provider_dialects.py#L461) |
| [§9.2-06](../docs/RFC.md?plain=1#L633) | 兼容 Chat 端点的两个 Body 路由字段分别按需启用 | [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L480) |
| [§9.2-07](../docs/RFC.md?plain=1#L634) | Anthropic 只发送路由头，Body 保持协议格式 | [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L480) |
| [§9.2-08](../docs/RFC.md?plain=1#L660) | 慢速流入工具参数与已经开始执行的长命令位于不同的计时阶段 | [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L231) · [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L293) · [#33](https://github.com/Luthics/anix/issues/33) |
| [§9.2-09](../docs/RFC.md?plain=1#L612) | 有效 model、base URL、timeout 与密钥来源 | [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L538) · [#4](https://github.com/Luthics/anix/issues/4) |
| [§9.2-10](../docs/RFC.md?plain=1#L654) | WAL、Trace、on-exit 五字段一致，请求 IR 剥离用量 | [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L31) · [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) |
| [§9.2-11](../docs/RFC.md?plain=1#L655) | 总输入输出包含缓存与推理明细 | [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L31) · [`provider_events_round_trip`](../crates/anix-proto/src/provider.rs#L242) · [`sse_normalizes_inclusive_usage_captures_thinking_signature_and_tool_use`](../crates/anix-providers/src/anthropic.rs#L1063) · [`test_inclusive_usage_preserves_wide_counts_and_available_cache_details`](../tests/test_provider_dialects.py#L334) |
| [§9.2-12](../docs/RFC.md?plain=1#L656) | 可选明细、uint64 类型及零值处理 | [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L101) · [`each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values`](../crates/anix-proto/tests/rfc_provider.rs#L7) · [`usage_requires_both_totals_even_when_all_three_details_are_present`](../crates/anix-proto/tests/rfc_provider.rs#L56) · [#22](https://github.com/Luthics/anix/issues/22) |
| [§9.2-13](../docs/RFC.md?plain=1#L657) | run 含压缩，区分跨会话历史消耗 | [`retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L141) · [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L631) · [#8](https://github.com/Luthics/anix/issues/8) |
| [§9.2-14](../docs/RFC.md?plain=1#L658) | 同 Provider 原样回传，切换时清除 | [`opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch`](../crates/anix-core/tests/rfc_runtime/provider.rs#L164) · [`coalesces_adjacent_user_and_tool_results_and_replays_signed_thinking`](../crates/anix-providers/src/anthropic.rs#L959) · [`replays_opaque_reasoning_and_flushes_tool_media_after_batch`](../crates/anix-providers/src/openai_compat.rs#L1006) · [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L352) · [#6](https://github.com/Luthics/anix/issues/6) |
| [§9.2-15](../docs/RFC.md?plain=1#L659) | HTTP 暂态错误分类、核心三次退避与可观测事件 | [`retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr`](../crates/anix-core/tests/rfc_runtime/provider.rs#L20) · [`nonretryable_error_preserves_provider_stderr_without_another_attempt`](../crates/anix-core/tests/rfc_runtime/provider.rs#L136) · [`sse_overloaded_error_is_retryable`](../crates/anix-providers/src/anthropic.rs#L1142) · [`http_429_and_529_map_to_retryable_errors_and_400_is_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L399) · [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L412) · [#45](https://github.com/Luthics/anix/issues/45) |
| [§9.2-16](../docs/RFC.md?plain=1#L638) | oneshot 虽已输出 done 却非零退出时返回 1，不执行未提交的命令，后一条 Inbox 消息保留在 new/ | [`oneshot_nonzero_after_done_discards_uncommitted_calls_and_preserves_later_jobs`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L268) |
| [§9.2-17](../docs/RFC.md?plain=1#L624) | 协进程重新启动后，替代进程仍收到相同 session_id 和已提交的工具历史 | [`restarting_a_coprocess_between_turns_preserves_session_and_tool_history`](../crates/anix-core/tests/rfc_runtime/session.rs#L387) |
| [§9.2-18](../docs/RFC.md?plain=1#L625) | 流式及整包上游响应转换为逐行 Provider 事件，工具参数与终结状态可恢复 | [`sse_streams_reasoning_text_and_trailing_usage_after_finish_reason`](../crates/anix-providers/src/openai_compat.rs#L1080) · [`sse_accumulates_multi_chunk_tool_calls_and_extra_content`](../crates/anix-providers/src/openai_compat.rs#L1128) · [`sse_clean_eof_after_finish_reason_without_done_and_truncated_error`](../crates/anix-providers/src/openai_compat.rs#L1178) · [`non_streaming_json_fallback_parses_message_and_inline_error`](../crates/anix-providers/src/openai_compat.rs#L1201) · [`parses_responses_sse_text_tool_and_usage`](../crates/anix-providers/src/openai_responses.rs#L685) · [`coprocess_mode_serves_multiple_requests_survives_error_and_exits_0_on_eof`](../crates/anix-providers/tests/e2e_mock.rs#L463) · [`test_partial_text_then_upstream_eof_yields_error_and_coprocess_accepts_next_request`](../tests/test_provider_dialects.py#L247) · [`test_fragmented_tool_arguments_produce_one_call_with_the_original_id`](../tests/test_provider_dialects.py#L292) |

## §9.3

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§9.3-01](../docs/RFC.md?plain=1#L664) | pre-context 连续过滤 IR，失败向 fd 2 报告并停止 | [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L11) · [`pre_context_failure_forwards_stderr_and_stops_before_inference`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L82) |
| [§9.3-02](../docs/RFC.md?plain=1#L665) | 两个成功的 post-tool 钩子串联改写结果并持久化，钩子失败则中止后续工具 | [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L108) · [`post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L602) |
| [§9.3-03](../docs/RFC.md?plain=1#L666) | on-exit 用量、退出状态、5 秒超时与错误隔离 | [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L204) · [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L158) |
| [§9.3-04](../docs/RFC.md?plain=1#L666) | 无状态和恢复 WAL 后，退出钩子的状态码、轮次、五项进程与会话用量和上下文水位逐项符合预期 | [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) [断言 L168](../tests/test_cycle_accounting.py#L168) |
| [§9.3-05](../docs/RFC.md?plain=1#L666) | 钩子失败后仍执行下一钩子，两者报告相同，Trace 可定位故障而主状态与交付通道保持正确 | [`test_failed_exit_hook_preserves_reports_later_hooks_and_primary_status`](../tests/test_cycle_accounting.py#L179) [断言 L191](../tests/test_cycle_accounting.py#L191) [断言 L194](../tests/test_cycle_accounting.py#L194) [断言 L197](../tests/test_cycle_accounting.py#L197) |

## §9.4

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§9.4-01](../docs/RFC.md?plain=1#L679) | 模型从真实 XML 索引读取含特殊字符的技能路径，经 msh 按需获取完整正文 | [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L258) |
| [§9.4-02](../docs/RFC.md?plain=1#L670) | 20-skills 内置、同名覆写与禁用 | [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) |
| [§9.4-03](../docs/RFC.md?plain=1#L672) | 六层搜索顺序和同名覆盖 | [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L199) |
| [§9.4-04](../docs/RFC.md?plain=1#L673) | SKILL.md 生成转义 XML 元数据，正文不进首轮 | [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L199) · [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L258) |
| [§9.4-05](../docs/RFC.md?plain=1#L673) | 混合合法与缺字段的 SKILL.md，索引只包含 YAML 中声明的技能名、路径与描述 | [`test_skill_index_requires_both_frontmatter_fields_and_excludes_private_text`](../tests/test_discovery_indexes.py#L118) [断言 L135](../tests/test_discovery_indexes.py#L135) [断言 L137](../tests/test_discovery_indexes.py#L137) |
| [§9.4-06](../docs/RFC.md?plain=1#L679) | XML 解码后的技能路径应能打开原文件，覆盖目录名含空格、Tab、LF 和 CR | [`test_skill_xml_paths_reopen_files_with_posix_whitespace`](../tests/test_discovery_indexes.py#L140) · [#62](https://github.com/Luthics/anix/issues/62) |

## §9.5

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§9.5-01](../docs/RFC.md?plain=1#L686) | 采集端在第一条记录前或期间断开，Payload、stderr 和退出码保持一致 | [`disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status`](../crates/anix-core/tests/rfc_runtime/trace.rs#L237) |
| [§9.5-02](../docs/RFC.md?plain=1#L686) | 消费者保持连接但停止读取时的背压及恢复语义 | [#11](https://github.com/Luthics/anix/issues/11) |
| [§9.5-03](../docs/RFC.md?plain=1#L693) | 初始 Trace 与 WAL、Provider 环境和协进程信封可用同一标识关联 | [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L196) |
| [§9.5-04](../docs/RFC.md?plain=1#L704) | 模型只看清洗后文本，私有 wire 文件保留命令实际输出的 ANSI 字节 | [`test_wire_trace_keeps_raw_msh_output_while_the_model_receives_cleaned_text`](../tests/test_provider_streaming.py#L198) · [#52](https://github.com/Luthics/anix/issues/52) |
| [§9.5-05](../docs/RFC.md?plain=1#L702) | 协进程错误事件含字符串 provider_stderr。异步诊断是否完整、如何划分到对应请求待裁定 | [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L412) · [#38](https://github.com/Luthics/anix/issues/38) |
| [§9.5-06](../docs/RFC.md?plain=1#L692) | Trace 通用 error 事件的适用故障范围待明确 | [#32](https://github.com/Luthics/anix/issues/32) |
| [§9.5-07](../docs/RFC.md?plain=1#L699) | 检查已终止的任务组且没有退出码文件时，完成事件含显式 null，后续推理不重复通知 | [`test_finished_group_without_an_exit_file_reports_null_exactly_once`](../tests/test_msh_jobs.py#L346) |
| [§9.5-08](../docs/RFC.md?plain=1#L699) | started 和 finished 通过相同任务标识、PID、进程组和日志路径关联，并报告真实状态及经过的毫秒数 | [`test_background_completion_is_reported_once_before_inference_and_replays_from_wal`](../tests/test_msh_jobs.py#L284) |
| [§9.5-09](../docs/RFC.md?plain=1#L692) | 审批和实际执行分别发出事件，已应用的 fold 也写入 Trace | [`events_distinguish_tool_approval_from_execution_and_record_an_applied_fold`](../crates/anix-core/tests/rfc_runtime/trace.rs#L16) |
| [§9.5-10](../docs/RFC.md?plain=1#L691) | 对独立 FD 比较 events、deltas、wire 记录内容，三种级别均排除 Provider 密钥 | [`explicit_fd_separates_events_deltas_and_wire_and_omits_provider_key`](../crates/anix-core/tests/rfc_runtime/trace.rs#L147) |
| [§9.5-11](../docs/RFC.md?plain=1#L704) | 未指定显式 FD 时 journal 收到生命周期事件，wire 内容不会发往共享 socket | [`journal_without_an_explicit_fd_receives_lifecycle_events_and_no_wire_records`](../crates/anix-core/tests/rfc_runtime/trace.rs#L201) |
| [§9.5-12](../docs/RFC.md?plain=1#L685) | 同时存在显式 Trace FD 和 journal 时，记录写入指定 FD | [`explicit_trace_fd_takes_precedence_over_an_available_journal`](../crates/anix-core/tests/rfc_runtime/trace.rs#L289) |
| [§9.5-13](../docs/RFC.md?plain=1#L703) | 下一帧尚未发送时上一帧已在 Trace 中可见；完成前 WAL 没有 assistant，完成后才提交 | [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L319) |
| [§9.5-14](../docs/RFC.md?plain=1#L693) | run_start 记录本次真实生效的 shell=none 与 output=reply | [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1244) |
| [§9.5-15](../docs/RFC.md?plain=1#L695) | 从 RFC 读取完整原因枚举，逐个触发并核对 cycle_end、run_end 与进程状态，包括同码不同原因 | [`test_each_declared_cycle_end_reason_matches_the_cause_and_os_status`](../tests/test_cycle_accounting.py#L49) [断言 L82](../tests/test_cycle_accounting.py#L82) [断言 L87](../tests/test_cycle_accounting.py#L87) |
| [§9.5-16](../docs/RFC.md?plain=1#L694) | 初始输入与两封实际 Inbox 消息的起止事件配对、编号和来源一致，队列排空才进入 idle；start_turn_id 的精确锚点待裁决 | [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) [断言 L134](../tests/test_cycle_accounting.py#L134) [断言 L137](../tests/test_cycle_accounting.py#L137) [断言 L140](../tests/test_cycle_accounting.py#L140) · [#66](https://github.com/Luthics/anix/issues/66) |
| [§9.5-17](../docs/RFC.md?plain=1#L695) | 连续三个 cycle 分别核对轮次数、当轮及进程五字段用量、会话总量和上下文水位 | [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) [断言 L163](../tests/test_cycle_accounting.py#L163) |
| [§9.5-18](../docs/RFC.md?plain=1#L697) | 每次 llm_response 逐字段匹配该实际 Provider 请求的 usage | [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) [断言 L150](../tests/test_cycle_accounting.py#L150) |
| [§9.5-19](../docs/RFC.md?plain=1#L701) | 多轮 Inbox 闲置退出时核对运行期间总轮次、进程及会话总量、末轮水位和退出状态 | [`test_inbox_cycles_separate_per_request_per_cycle_process_and_session_totals`](../tests/test_cycle_accounting.py#L94) [断言 L170](../tests/test_cycle_accounting.py#L170) |

## §9.6

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§9.6-01](../docs/RFC.md?plain=1#L712) | MCP 指南按 anix 状态目录、全局和项目路径披露 | [`mcp_guide_follows_anix_xdg_and_project_precedence_without_disclosing_tool_schemas`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L381) |
| [§9.6-02](../docs/RFC.md?plain=1#L712) | 首次释放、版本更新和状态目录不可写时回退 | [`tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L262) · [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L302) |
| [§9.6-03](../docs/RFC.md?plain=1#L727) | 整块覆写、两类禁用及磁盘副作用 | [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L302) · [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L540) |
| [§9.6-04](../docs/RFC.md?plain=1#L715) | 屏蔽所有指南后不向系统提示词注入 tips 索引 | [`an_empty_merged_tips_index_costs_no_prompt_bytes`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L350) |
| [§9.6-05](../docs/RFC.md?plain=1#L712) | 从本版 RFC 读取完整指南名单，双向核对首次释放的文件和绝对路径索引，覆盖默认及自定义 XDG_STATE_HOME | [`test_declared_builtin_guides_match_materialized_files_and_the_first_request`](../tests/test_discovery_indexes.py#L49) [断言 L59](../tests/test_discovery_indexes.py#L59) [断言 L61](../tests/test_discovery_indexes.py#L61) |
| [§9.6-06](../docs/RFC.md?plain=1#L715) | 全局和项目各选四种状态，16 种组合逐一检查内置同名指南被替换、屏蔽或恢复 | [`test_tip_masks_overrides_and_sort_order_across_both_user_layers`](../tests/test_discovery_indexes.py#L67) [断言 L110](../tests/test_discovery_indexes.py#L110) |
| [§9.6-07](../docs/RFC.md?plain=1#L716) | 比较完整路径列表的顺序和唯一性，覆盖空格、大小写、排除项、配置根切换以及正文留在磁盘 | [`test_tip_masks_overrides_and_sort_order_across_both_user_layers`](../tests/test_discovery_indexes.py#L67) [断言 L113](../tests/test_discovery_indexes.py#L113) [断言 L115](../tests/test_discovery_indexes.py#L115) |
| [§9.6-08](../docs/RFC.md?plain=1#L716) | 合法文件名含 LF 时，逐行索引如何表示可供读取的完整路径待明确 | [#63](https://github.com/Luthics/anix/issues/63) |

## §10.2

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§10.2-01](../docs/RFC.md?plain=1#L751) | Linux 上主进程异常终止后，活跃的 Provider 或 shell 子进程也终止 | [`abrupt_parent_exit_stops_the_active_provider_or_shell_child`](../crates/anix-core/tests/rfc_runtime/signals.rs#L222) |
| [§10.2-02](../docs/RFC.md?plain=1#L751) | 缺少 pidfd 时，等待处理了 SIGTERM 的 Provider 和退出钩子仍须遵守各自期限 | [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L158) · [`sigterm_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L171) · [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L178) · [#48](https://github.com/Luthics/anix/issues/48) |
| [§10.2-03](../docs/RFC.md?plain=1#L752) | 探测内核 ABI 与可用性，启用后的写入范围与不支持时的 strict 行为仍待专项验证 | [`probes_platform_capabilities`](../crates/anix-core/src/platform/mod.rs#L119) · 待补专项断言 |

## §10.3

| 编号及 RFC 原文 | 可验收行为 | 测试、断言与问题 |
| --- | --- | --- |
| [§10.3-01](../docs/RFC.md?plain=1#L756) | 全新 HOME 的安装布局、rc 文件、保留用户修改及安装后启动 | [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L121) · [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L251) |
| [§10.3-02](../docs/RFC.md?plain=1#L758) | 同时缺少 msh 且全局 TOML 损坏时先报告第 1 步 | [`doctor_reports_missing_msh_before_a_malformed_global_config`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L155) · [#30](https://github.com/Luthics/anix/issues/30) |
| [§10.3-03](../docs/RFC.md?plain=1#L760) | 逐个移除私有二进制或其执行权限，doctor 指出故障路径 | [`each_missing_or_non_executable_private_binary_is_identified`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L31) |
| [§10.3-04](../docs/RFC.md?plain=1#L761) | 免鉴权配置、密钥回退与项目级默认 Provider 的诊断 | [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L71) · [`doctor_checks_the_selected_project_provider_and_its_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L123) |
| [§10.3-05](../docs/RFC.md?plain=1#L762) | doctor 发现无法创建匿名管道并给出修复提示 | [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L165) |
| [§10.3-06](../docs/RFC.md?plain=1#L763) | doctor 检测锁操作失败或失去互斥性 | [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L165) |
| [§10.3-07](../docs/RFC.md?plain=1#L756) | 发布安装器的版本选择和验证来源 | [#7](https://github.com/Luthics/anix/issues/7) |
| [§10.3-08](../docs/RFC.md?plain=1#L760) | 默认 Provider 位于用户 PATH 时，doctor 应检查哪些实际可执行文件 | [#25](https://github.com/Luthics/anix/issues/25) |
