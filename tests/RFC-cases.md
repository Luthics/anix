# v1.2.0 RFC 逐测试索引

每行对应一个测试函数，链接分别指向测试声明和规范原文。参数化用例共用所在函数的条款。

按规范原句查找断言或相关 issue，见[条款验收表](RFC-requirements.md)。

历史快照见 [v1.1.2 的 277 个用例](https://github.com/Luthics/anix/blob/e4fb614f314625ae282bd2fcb9b7e80312514946/tests/RFC-cases.md)、[v1.1.1 的 262 个用例](https://github.com/Luthics/anix/blob/0b11a3c60218b9313f9e81a806c9a471906766eb/tests/RFC-cases.md)。v1.0 的 198 个用例保存在[v1.0 测试索引](https://github.com/Luthics/anix/blob/45e55a9f16f6bead6a933c67051b94b0acb753a0/tests/RFC-cases.md)。

运行 `python3 scripts/rfc-test-index.py` 更新此文件。CI 使用 `--check` 校验。

## `crates/anix-core/src/cli.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`parses_script_and_args_with_double_dash`](../crates/anix-core/src/cli.rs#L220) | [§3.1](../docs/RFC.md?plain=1#L69) |
| [`parses_direct_and_quoted_script_args_without_double_dash`](../crates/anix-core/src/cli.rs#L238) | [§3.1](../docs/RFC.md?plain=1#L69) |
| [`trust_subcommand_does_not_conflict_with_script_args`](../crates/anix-core/src/cli.rs#L261) | [§3.1](../docs/RFC.md?plain=1#L69) · [§8.3](../docs/RFC.md?plain=1#L546) |

## `crates/anix-core/src/platform/mod.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`probes_platform_capabilities`](../crates/anix-core/src/platform/mod.rs#L119) | [§10.2](../docs/RFC.md?plain=1#L747) |

## `crates/anix-core/src/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage`](../crates/anix-core/src/wal.rs#L748) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`replay_restores_session_id_across_checkpoint_and_derives_stable_legacy_fallback`](../crates/anix-core/src/wal.rs#L859) | [§6.2](../docs/RFC.md?plain=1#L358) |

## `crates/anix-core/tests/rfc_3_2/cli.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`crlf_script_reaches_the_provider_as_the_intended_system_message`](../crates/anix-core/tests/rfc_3_2/cli.rs#L44) | [§3.2](../docs/RFC.md?plain=1#L93) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position`](../crates/anix-core/tests/rfc_3_2/cli.rs#L67) | [§3.2](../docs/RFC.md?plain=1#L93) · [§3.3](../docs/RFC.md?plain=1#L119) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`invalid_file_bytes_and_yaml_report_usage_errors_to_stderr`](../crates/anix-core/tests/rfc_3_2/cli.rs#L90) | [§3.2](../docs/RFC.md?plain=1#L93) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`yaml_block_scalar_chomping_survives_the_frontmatter_boundary`](../crates/anix-core/tests/rfc_3_2/cli.rs#L107) | [§3.2](../docs/RFC.md?plain=1#L93) · [§3.3](../docs/RFC.md?plain=1#L119) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`duplicate_yaml_keys_are_invalid_even_inside_open_maps`](../crates/anix-core/tests/rfc_3_2/cli.rs#L150) | [§3.2](../docs/RFC.md?plain=1#L93) · [§3.3](../docs/RFC.md?plain=1#L119) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`yaml_aliases_can_supply_typed_provider_params_from_an_extension_field`](../crates/anix-core/tests/rfc_3_2/cli.rs#L163) | [§3.2](../docs/RFC.md?plain=1#L93) · [§3.3](../docs/RFC.md?plain=1#L119) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`vision_requires_a_yaml_boolean_and_leaves_provider_params_open`](../crates/anix-core/tests/rfc_3_2/cli.rs#L181) | [§3.2](../docs/RFC.md?plain=1#L93) · [§3.3](../docs/RFC.md?plain=1#L119) · [§5.5](../docs/RFC.md?plain=1#L295) · [§6.5](../docs/RFC.md?plain=1#L415) |

## `crates/anix-core/tests/rfc_3_2/encoding.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`all_256_single_bytes_inside_a_plain_prompt`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L11) | [§3.2](../docs/RFC.md?plain=1#L93) |
| [`valid_multibyte_boundaries_survive_file_decoding`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L37) | [§3.2](../docs/RFC.md?plain=1#L93) |
| [`malformed_sequences_are_rejected_in_every_script_region`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L64) | [§3.2](../docs/RFC.md?plain=1#L93) |
| [`eof_at_each_interior_byte_of_a_utf8_scalar`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L104) | [§3.2](../docs/RFC.md?plain=1#L93) |

## `crates/anix-core/tests/rfc_3_2/grammar.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`layout_matrix_for_line_ending_shebang_header_body_and_eof`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L9) | [§3.2](../docs/RFC.md?plain=1#L93) |
| [`only_a_first_line_beginning_with_hash_bang_is_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L46) | [§3.2](../docs/RFC.md?plain=1#L93) |
| [`opening_delimiter_requires_the_designated_line_and_exact_marker`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L69) | [§3.2](../docs/RFC.md?plain=1#L93) |
| [`first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L92) | [§3.2](../docs/RFC.md?plain=1#L93) |
| [`yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L115) | [§3.2](../docs/RFC.md?plain=1#L93) · [§3.3](../docs/RFC.md?plain=1#L119) |
| [`incomplete_delimiters_and_malformed_yaml_fail_at_the_header`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L145) | [§3.2](../docs/RFC.md?plain=1#L93) |

## `crates/anix-core/tests/rfc_runtime/compaction.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L30) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.3](../docs/RFC.md?plain=1#L662) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L99) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.3](../docs/RFC.md?plain=1#L662) |
| [`retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L139) | [§9.2](../docs/RFC.md?plain=1#L606) · [§6.1](../docs/RFC.md?plain=1#L331) · [§9.3](../docs/RFC.md?plain=1#L662) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L198) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.3](../docs/RFC.md?plain=1#L662) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L321) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L374) | [§6.1](../docs/RFC.md?plain=1#L331) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`fresh_window_counts_utf8_bytes_in_groups_of_three`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L443) | [§6.1](../docs/RFC.md?plain=1#L331) |
| [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L459) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L489) | [§6.1](../docs/RFC.md?plain=1#L331) |
| [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L548) | [§6.1](../docs/RFC.md?plain=1#L331) · [§8.2](../docs/RFC.md?plain=1#L492) |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.3](../docs/RFC.md?plain=1#L662) |
| [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L872) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`empty_or_failed_auto_summary_keeps_the_subwindow_and_reports_a_tool_error`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L902) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) |

## `crates/anix-core/tests/rfc_runtime/configuration.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`agent_name_has_exactly_the_documented_ascii_alphabet`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L11) | [§3.3](../docs/RFC.md?plain=1#L119) |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L34) | [§3.3](../docs/RFC.md?plain=1#L119) |
| [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L91) | [§3.3](../docs/RFC.md?plain=1#L119) · [§6.1](../docs/RFC.md?plain=1#L331) |
| [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L116) | [§3.4](../docs/RFC.md?plain=1#L138) · [§8.2](../docs/RFC.md?plain=1#L492) |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) | [§3.3](../docs/RFC.md?plain=1#L119) · [§3.4](../docs/RFC.md?plain=1#L138) · [§7.1](../docs/RFC.md?plain=1#L432) · [§8.2](../docs/RFC.md?plain=1#L492) |
| [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L351) | [§3.3](../docs/RFC.md?plain=1#L119) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L390) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.4](../docs/RFC.md?plain=1#L138) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L435) | [§3.4](../docs/RFC.md?plain=1#L138) |
| [`project_search_stops_at_nearest_configuration_git_or_home_boundary`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L469) | [§8.1](../docs/RFC.md?plain=1#L485) |
| [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L520) | [§2](../docs/RFC.md?plain=1#L51) · [§3.4](../docs/RFC.md?plain=1#L138) · [§8.2](../docs/RFC.md?plain=1#L492) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L575) | [§3.4](../docs/RFC.md?plain=1#L138) · [§5.1](../docs/RFC.md?plain=1#L241) |
| [`invalid_resolved_environment_values_exit_two_before_contacting_the_provider`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L623) | [§3.4](../docs/RFC.md?plain=1#L138) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L651) | [§8.2](../docs/RFC.md?plain=1#L492) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L698) | [§3.3](../docs/RFC.md?plain=1#L119) · [§3.2](../docs/RFC.md?plain=1#L93) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`cli_params_preserve_json_literals_and_the_exact_fallback_string`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L732) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.4](../docs/RFC.md?plain=1#L138) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`malformed_cli_param_stops_before_provider_or_wal_creation`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L769) | [§3.1](../docs/RFC.md?plain=1#L69) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L794) | [§3.3](../docs/RFC.md?plain=1#L119) · [§3.4](../docs/RFC.md?plain=1#L138) · [§6.2](../docs/RFC.md?plain=1#L358) · [§8.1](../docs/RFC.md?plain=1#L485) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L859) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.3](../docs/RFC.md?plain=1#L119) · [§3.4](../docs/RFC.md?plain=1#L138) · [§8.2](../docs/RFC.md?plain=1#L492) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L937) | [§2](../docs/RFC.md?plain=1#L51) · [§8.2](../docs/RFC.md?plain=1#L492) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1003) | [§8.1](../docs/RFC.md?plain=1#L485) · [§4.1](../docs/RFC.md?plain=1#L167) · [§8.3](../docs/RFC.md?plain=1#L546) |
| [`selected_model_changes_the_provider_environment_without_entering_the_ir`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1062) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-core/tests/rfc_runtime/control.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L26) | [§7.2](../docs/RFC.md?plain=1#L458) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order`](../crates/anix-core/tests/rfc_runtime/control.rs#L95) | [§7.2](../docs/RFC.md?plain=1#L458) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) |

## `crates/anix-core/tests/rfc_runtime/doctor.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_missing_or_non_executable_private_binary_is_identified`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L31) | [§10.3](../docs/RFC.md?plain=1#L754) · [§2](../docs/RFC.md?plain=1#L51) · [§8.1](../docs/RFC.md?plain=1#L485) |
| [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L70) | [§10.3](../docs/RFC.md?plain=1#L754) |
| [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L99) | [§10.3](../docs/RFC.md?plain=1#L754) |
| [`doctor_checks_the_selected_project_provider_and_its_credentials`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L122) | [§8.1](../docs/RFC.md?plain=1#L485) · [§8.2](../docs/RFC.md?plain=1#L492) · [§10.3](../docs/RFC.md?plain=1#L754) |
| [`doctor_reports_missing_msh_before_a_malformed_global_config`](../crates/anix-core/tests/rfc_runtime/doctor.rs#L154) | [§10.3](../docs/RFC.md?plain=1#L754) |

## `crates/anix-core/tests/rfc_runtime/fd.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment`](../crates/anix-core/tests/rfc_runtime/fd.rs#L61) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.4](../docs/RFC.md?plain=1#L138) · [§4.1](../docs/RFC.md?plain=1#L167) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input`](../crates/anix-core/tests/rfc_runtime/fd.rs#L94) | [§3.1](../docs/RFC.md?plain=1#L69) · [§4.1](../docs/RFC.md?plain=1#L167) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider`](../crates/anix-core/tests/rfc_runtime/fd.rs#L136) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.4](../docs/RFC.md?plain=1#L138) · [§4.1](../docs/RFC.md?plain=1#L167) · [§6.5](../docs/RFC.md?plain=1#L415) |

## `crates/anix-core/tests/rfc_runtime/hooks.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L10) | [§9.3](../docs/RFC.md?plain=1#L662) · [§9.1](../docs/RFC.md?plain=1#L568) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`pre_context_failure_forwards_stderr_and_stops_before_inference`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L81) | [§9.3](../docs/RFC.md?plain=1#L662) |
| [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L107) | [§9.3](../docs/RFC.md?plain=1#L662) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L155) | [§6.3](../docs/RFC.md?plain=1#L397) · [§9.3](../docs/RFC.md?plain=1#L662) · [§9.5](../docs/RFC.md?plain=1#L681) · [§10.2](../docs/RFC.md?plain=1#L747) |
| [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L195) | [§9.4](../docs/RFC.md?plain=1#L668) |
| [`tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L258) | [§1.4](../docs/RFC.md?plain=1#L33) · [§8.1](../docs/RFC.md?plain=1#L485) · [§9.6](../docs/RFC.md?plain=1#L706) |
| [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L297) | [§9.6](../docs/RFC.md?plain=1#L706) |
| [`an_empty_merged_tips_index_costs_no_prompt_bytes`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L345) | [§9.6](../docs/RFC.md?plain=1#L706) |
| [`mcp_guide_follows_anix_xdg_and_project_precedence_without_disclosing_tool_schemas`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L376) | [§1.4](../docs/RFC.md?plain=1#L33) · [§8.1](../docs/RFC.md?plain=1#L485) · [§9.6](../docs/RFC.md?plain=1#L706) |
| [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L467) | [§4.4](../docs/RFC.md?plain=1#L222) · [§4.1](../docs/RFC.md?plain=1#L167) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L530) | [§4.4](../docs/RFC.md?plain=1#L222) · [§8.3](../docs/RFC.md?plain=1#L546) · [§9.3](../docs/RFC.md?plain=1#L662) · [§9.4](../docs/RFC.md?plain=1#L668) · [§9.6](../docs/RFC.md?plain=1#L706) |
| [`post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L591) | [§9.3](../docs/RFC.md?plain=1#L662) · [§9.1](../docs/RFC.md?plain=1#L568) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§4.1](../docs/RFC.md?plain=1#L167) · [§7.1](../docs/RFC.md?plain=1#L432) |

## `crates/anix-core/tests/rfc_runtime/inbox.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L19) | [§4.2](../docs/RFC.md?plain=1#L193) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`a_publisher_can_queue_work_while_the_initial_cli_task_is_running`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L120) | [§4.2](../docs/RFC.md?plain=1#L193) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L179) | [§4.2](../docs/RFC.md?plain=1#L193) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`oneshot_nonzero_after_done_discards_uncommitted_calls_and_preserves_later_jobs`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L265) | [§4.2](../docs/RFC.md?plain=1#L193) · [§6.2](../docs/RFC.md?plain=1#L358) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L336) | [§4.2](../docs/RFC.md?plain=1#L193) · [§6.3](../docs/RFC.md?plain=1#L397) · [§9.5](../docs/RFC.md?plain=1#L681) |

## `crates/anix-core/tests/rfc_runtime/io.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/anix-core/tests/rfc_runtime/io.rs#L8) | [§3.1](../docs/RFC.md?plain=1#L69) · [§4.1](../docs/RFC.md?plain=1#L167) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`args_stdin_and_sparse_extra_descriptors_are_ordered_and_trace_is_separate`](../crates/anix-core/tests/rfc_runtime/io.rs#L35) | [§4.1](../docs/RFC.md?plain=1#L167) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L87) | [§4.1](../docs/RFC.md?plain=1#L167) · [§4.3](../docs/RFC.md?plain=1#L208) |
| [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L149) | [§2](../docs/RFC.md?plain=1#L51) · [§3.4](../docs/RFC.md?plain=1#L138) · [§4.3](../docs/RFC.md?plain=1#L208) |
| [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/anix-core/tests/rfc_runtime/io.rs#L195) | [§3.1](../docs/RFC.md?plain=1#L69) · [§4.1](../docs/RFC.md?plain=1#L167) |
| [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L216) | [§4.1](../docs/RFC.md?plain=1#L167) · [§4.3](../docs/RFC.md?plain=1#L208) |
| [`shell_environment_contains_only_observability_effective_approval_and_routed_fds`](../crates/anix-core/tests/rfc_runtime/io.rs#L289) | [§3.4](../docs/RFC.md?plain=1#L138) · [§4.3](../docs/RFC.md?plain=1#L208) |
| [`unterminated_deltas_are_separated_from_tool_status_diagnostics_and_usage`](../crates/anix-core/tests/rfc_runtime/io.rs#L401) | [§4.1](../docs/RFC.md?plain=1#L167) · [§4.3](../docs/RFC.md?plain=1#L208) |

## `crates/anix-core/tests/rfc_runtime/provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr`](../crates/anix-core/tests/rfc_runtime/provider.rs#L20) | [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`nonretryable_error_preserves_provider_stderr_without_another_attempt`](../crates/anix-core/tests/rfc_runtime/provider.rs#L136) | [§9.2](../docs/RFC.md?plain=1#L606) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/anix-core/tests/rfc_runtime/provider.rs#L147) | [§9.2](../docs/RFC.md?plain=1#L606) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`provider_timeout_maps_to_124_without_a_retry`](../crates/anix-core/tests/rfc_runtime/provider.rs#L158) | [§9.2](../docs/RFC.md?plain=1#L606) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch`](../crates/anix-core/tests/rfc_runtime/provider.rs#L164) | [§9.2](../docs/RFC.md?plain=1#L606) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`max_turns_allows_completion_on_the_last_request_and_stops_unfinished_work`](../crates/anix-core/tests/rfc_runtime/provider.rs#L234) | [§3.3](../docs/RFC.md?plain=1#L119) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |

## `crates/anix-core/tests/rfc_runtime/session.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`stateless_runs_get_distinct_uuid_v4s_while_all_provider_calls_in_a_run_share_one`](../crates/anix-core/tests/rfc_runtime/session.rs#L100) | [§1.4](../docs/RFC.md?plain=1#L33) · [§3.4](../docs/RFC.md?plain=1#L138) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`persisted_id_survives_builtin_compaction_checkpoint_alias_and_subsequent_resume`](../crates/anix-core/tests/rfc_runtime/session.rs#L122) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`legacy_wal_uses_a_stable_canonical_path_id_across_aliases_and_restarts`](../crates/anix-core/tests/rfc_runtime/session.rs#L161) | [§1.4](../docs/RFC.md?plain=1#L33) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope`](../crates/anix-core/tests/rfc_runtime/session.rs#L195) | [§1.4](../docs/RFC.md?plain=1#L33) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`nested_anix_gets_its_own_id_and_the_parent_resumes_with_its_original_id`](../crates/anix-core/tests/rfc_runtime/session.rs#L224) | [§1.4](../docs/RFC.md?plain=1#L33) · [§3.4](../docs/RFC.md?plain=1#L138) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`inbox_keeps_one_session_id_for_both_messages_in_a_single_coprocess`](../crates/anix-core/tests/rfc_runtime/session.rs#L276) | [§4.2](../docs/RFC.md?plain=1#L193) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`first_checkpoint_in_a_legacy_wal_persists_the_fallback_id_even_if_the_wal_moves`](../crates/anix-core/tests/rfc_runtime/session.rs#L313) | [§1.4](../docs/RFC.md?plain=1#L33) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`empty_context_overrides_a_saved_wal_and_keeps_one_ephemeral_id_through_compaction`](../crates/anix-core/tests/rfc_runtime/session.rs#L349) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.4](../docs/RFC.md?plain=1#L138) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`restarting_a_coprocess_between_turns_preserves_session_and_tool_history`](../crates/anix-core/tests/rfc_runtime/session.rs#L386) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |

## `crates/anix-core/tests/rfc_runtime/signals.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/anix-core/tests/rfc_runtime/signals.rs#L12) | [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L50) | [§4.2](../docs/RFC.md?plain=1#L193) · [§6.2](../docs/RFC.md?plain=1#L358) · [§6.4](../docs/RFC.md?plain=1#L405) |
| [`usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference`](../crates/anix-core/tests/rfc_runtime/signals.rs#L117) | [§6.4](../docs/RFC.md?plain=1#L405) · [§6.1](../docs/RFC.md?plain=1#L331) |
| [`sigterm_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L170) | [§6.3](../docs/RFC.md?plain=1#L397) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§10.2](../docs/RFC.md?plain=1#L747) |
| [`sigint_bounds_cleanup_of_a_provider_that_acknowledges_term`](../crates/anix-core/tests/rfc_runtime/signals.rs#L176) | [§6.3](../docs/RFC.md?plain=1#L397) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§10.2](../docs/RFC.md?plain=1#L747) |
| [`abrupt_parent_exit_stops_the_active_provider_or_shell_child`](../crates/anix-core/tests/rfc_runtime/signals.rs#L220) | [§10.2](../docs/RFC.md?plain=1#L747) |

## `crates/anix-core/tests/rfc_runtime/trace.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`events_distinguish_tool_approval_from_execution_and_record_an_applied_fold`](../crates/anix-core/tests/rfc_runtime/trace.rs#L16) | [§5.1](../docs/RFC.md?plain=1#L241) · [§6.1](../docs/RFC.md?plain=1#L331) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`explicit_fd_separates_events_deltas_and_wire_and_omits_provider_key`](../crates/anix-core/tests/rfc_runtime/trace.rs#L147) | [§9.5](../docs/RFC.md?plain=1#L681) · [§4.1](../docs/RFC.md?plain=1#L167) |
| [`journal_without_an_explicit_fd_receives_lifecycle_events_and_no_wire_records`](../crates/anix-core/tests/rfc_runtime/trace.rs#L201) | [§4.1](../docs/RFC.md?plain=1#L167) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status`](../crates/anix-core/tests/rfc_runtime/trace.rs#L237) | [§9.5](../docs/RFC.md?plain=1#L681) |
| [`explicit_trace_fd_takes_precedence_over_an_available_journal`](../crates/anix-core/tests/rfc_runtime/trace.rs#L289) | [§4.1](../docs/RFC.md?plain=1#L167) · [§9.5](../docs/RFC.md?plain=1#L681) |

## `crates/anix-core/tests/rfc_runtime/vision.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`vision_resolves_from_cli_env_script_selected_provider_then_defaults`](../crates/anix-core/tests/rfc_runtime/vision.rs#L10) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.3](../docs/RFC.md?plain=1#L119) · [§3.4](../docs/RFC.md?plain=1#L138) · [§5.5](../docs/RFC.md?plain=1#L295) · [§6.2](../docs/RFC.md?plain=1#L358) · [§8.2](../docs/RFC.md?plain=1#L492) |
| [`native_vision_keeps_the_attachment_through_resume_and_never_calls_a_bridge`](../crates/anix-core/tests/rfc_runtime/vision.rs#L163) | [§5.5](../docs/RFC.md?plain=1#L295) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`no_native_vision_without_a_bridge_returns_the_documented_error_through_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L181) | [§5.5](../docs/RFC.md?plain=1#L295) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`bridge_gets_one_utf8_json_line_then_eof_and_its_description_survives_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L191) | [§5.5](../docs/RFC.md?plain=1#L295) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`bridge_nonzero_exit_and_timeout_both_return_stderr_as_a_recoverable_tool_error`](../crates/anix-core/tests/rfc_runtime/vision.rs#L218) | [§5.5](../docs/RFC.md?plain=1#L295) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) |

## `crates/anix-core/tests/rfc_runtime/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`legacy_wal_without_vision_or_fold_usage_resumes_without_rewriting_the_prefix`](../crates/anix-core/tests/rfc_runtime/wal.rs#L26) | [§1.4](../docs/RFC.md?plain=1#L33) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`repeated_resume_appends_one_vision_update_per_change_and_reuses_frozen_text`](../crates/anix-core/tests/rfc_runtime/wal.rs#L62) | [§3.1](../docs/RFC.md?plain=1#L69) · [§5.5](../docs/RFC.md?plain=1#L295) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |

## `crates/anix-core/tests/rfc_runtime/wal_replay.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`every_partial_byte_write_of_a_final_record_fails_without_modifying_history`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L35) | [§6.2](../docs/RFC.md?plain=1#L358) |
| [`resuming_a_complete_last_object_without_lf_never_joins_two_wal_records`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L59) | [§6.2](../docs/RFC.md?plain=1#L358) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L109) | [§6.2](../docs/RFC.md?plain=1#L358) |
| [`overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L137) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L173) | [§6.2](../docs/RFC.md?plain=1#L358) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L196) | [§6.2](../docs/RFC.md?plain=1#L358) |
| [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L215) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L396) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L438) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`initial_section_interruption_never_dispatches_without_a_frozen_system_message`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L515) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L569) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`each_combination_of_stderr_approval_and_vision_drift_creates_one_tail_update`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L635) | [§4.4](../docs/RFC.md?plain=1#L222) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |

## `crates/anix-core/tests/v1_1_e2e.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`trace_fd_explicit_only_no_implicit_fd3_and_cycle_events`](../crates/anix-core/tests/v1_1_e2e.rs#L164) | [§4.1](../docs/RFC.md?plain=1#L167) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L296) | [§5.2](../docs/RFC.md?plain=1#L250) · [§6.5](../docs/RFC.md?plain=1#L415) · [§8.3](../docs/RFC.md?plain=1#L546) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`vision_false_blocks_wal_pollution_and_vision_bridge_translates_and_mcp_tip_present`](../crates/anix-core/tests/v1_1_e2e.rs#L465) | [§4.4](../docs/RFC.md?plain=1#L222) · [§5.5](../docs/RFC.md?plain=1#L295) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.6](../docs/RFC.md?plain=1#L706) |
| [`aclr_auto_summary_tail_fold_and_coprocess_inbox_lifecycle`](../crates/anix-core/tests/v1_1_e2e.rs#L599) | [§4.1](../docs/RFC.md?plain=1#L167) · [§4.2](../docs/RFC.md?plain=1#L193) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_v1_1_1_session_id_persists_across_wal_resume_checkpoint_and_isolates_msh`](../crates/anix-core/tests/v1_1_e2e.rs#L808) | [§3.4](../docs/RFC.md?plain=1#L138) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_v1_1_2_msh_per_call_timeout_yield_bg_notification_and_kill_on_timeout`](../crates/anix-core/tests/v1_1_e2e.rs#L1032) | [§6.3](../docs/RFC.md?plain=1#L397) · [§6.5](../docs/RFC.md?plain=1#L415) · [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) · [§8.2](../docs/RFC.md?plain=1#L492) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_v1_2_0_orthogonal_output_contracts_and_shell_none`](../crates/anix-core/tests/v1_1_e2e.rs#L1233) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.3](../docs/RFC.md?plain=1#L119) · [§3.4](../docs/RFC.md?plain=1#L138) · [§4.1](../docs/RFC.md?plain=1#L167) · [§4.3](../docs/RFC.md?plain=1#L208) · [§4.4](../docs/RFC.md?plain=1#L222) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.5](../docs/RFC.md?plain=1#L681) |

## `crates/anix-proto/src/ctl.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`ctl_frames_round_trip`](../crates/anix-proto/src/ctl.rs#L46) | [§7.2](../docs/RFC.md?plain=1#L458) |

## `crates/anix-proto/src/ir.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`ir_jsonl_header_and_messages_round_trip`](../crates/anix-proto/src/ir.rs#L397) | [§9.1](../docs/RFC.md?plain=1#L568) |

## `crates/anix-proto/src/provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`provider_events_round_trip`](../crates/anix-proto/src/provider.rs#L242) | [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-proto/src/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`wal_events_round_trip`](../crates/anix-proto/src/wal.rs#L105) | [§6.2](../docs/RFC.md?plain=1#L358) |

## `crates/anix-proto/tests/rfc_ir.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L36) | [§9.1](../docs/RFC.md?plain=1#L568) |
| [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L68) | [§9.1](../docs/RFC.md?plain=1#L568) |
| [`validated_ir_only_serializes_objects_for_arguments_and_provider_opaque`](../crates/anix-proto/tests/rfc_ir.rs#L103) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L156) | [§9.1](../docs/RFC.md?plain=1#L568) |
| [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L185) | [§9.1](../docs/RFC.md?plain=1#L568) |
| [`a_single_tool_result_cannot_close_two_calls_with_the_same_id`](../crates/anix-proto/tests/rfc_ir.rs#L213) | [§9.1](../docs/RFC.md?plain=1#L568) |
| [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L223) | [§9.1](../docs/RFC.md?plain=1#L568) |
| [`assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state`](../crates/anix-proto/tests/rfc_ir.rs#L258) | [§9.1](../docs/RFC.md?plain=1#L568) |
| [`ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared`](../crates/anix-proto/tests/rfc_ir.rs#L277) | [§9.1](../docs/RFC.md?plain=1#L568) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`coprocess_envelope_wraps_the_same_ir_without_moving_model_into_history`](../crates/anix-proto/tests/rfc_ir.rs#L297) | [§1.4](../docs/RFC.md?plain=1#L33) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-proto/tests/rfc_provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values`](../crates/anix-proto/tests/rfc_provider.rs#L7) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`usage_requires_both_totals_even_when_all_three_details_are_present`](../crates/anix-proto/tests/rfc_provider.rs#L56) | [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-providers/src/anthropic.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`resolves_anthropic_endpoint_urls`](../crates/anix-providers/src/anthropic.rs#L942) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`coalesces_adjacent_user_and_tool_results_and_replays_signed_thinking`](../crates/anix-providers/src/anthropic.rs#L959) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`sse_normalizes_inclusive_usage_captures_thinking_signature_and_tool_use`](../crates/anix-providers/src/anthropic.rs#L1063) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`sse_overloaded_error_is_retryable`](../crates/anix-providers/src/anthropic.rs#L1142) | [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-providers/src/lib.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`resolves_dialect_from_argv0_symlink_and_version`](../crates/anix-providers/src/lib.rs#L232) | [§2](../docs/RFC.md?plain=1#L51) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`resolves_dialect_from_subcommand_flag_and_env`](../crates/anix-providers/src/lib.rs#L252) | [§2](../docs/RFC.md?plain=1#L51) · [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-providers/src/openai_compat.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`endpoint_url_appends_chat_completions_once`](../crates/anix-providers/src/openai_compat.rs#L926) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`translates_frozen_system_degraded_runtime_update_tools_and_params`](../crates/anix-providers/src/openai_compat.rs#L939) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`omits_tools_when_empty_and_supports_disabling_stream_options`](../crates/anix-providers/src/openai_compat.rs#L984) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`replays_opaque_reasoning_and_flushes_tool_media_after_batch`](../crates/anix-providers/src/openai_compat.rs#L1006) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`sse_streams_reasoning_text_and_trailing_usage_after_finish_reason`](../crates/anix-providers/src/openai_compat.rs#L1080) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`sse_accumulates_multi_chunk_tool_calls_and_extra_content`](../crates/anix-providers/src/openai_compat.rs#L1128) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`sse_clean_eof_after_finish_reason_without_done_and_truncated_error`](../crates/anix-providers/src/openai_compat.rs#L1178) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`non_streaming_json_fallback_parses_message_and_inline_error`](../crates/anix-providers/src/openai_compat.rs#L1201) | [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-providers/src/openai_responses.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L634) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`parses_responses_sse_text_tool_and_usage`](../crates/anix-providers/src/openai_responses.rs#L668) | [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-providers/tests/e2e_mock.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`version_flag_works_for_base_binary_and_busybox_symlinks`](../crates/anix-providers/tests/e2e_mock.rs#L202) | [§2](../docs/RFC.md?plain=1#L51) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`unsupported_ir_version_exits_2`](../crates/anix-providers/tests/e2e_mock.rs#L232) | [§6.5](../docs/RFC.md?plain=1#L415) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`busybox_symlink_openai_responses_end_to_end`](../crates/anix-providers/tests/e2e_mock.rs#L251) | [§2](../docs/RFC.md?plain=1#L51) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`busybox_symlink_openai_compat_end_to_end_and_unauthenticated_local`](../crates/anix-providers/tests/e2e_mock.rs#L289) | [§2](../docs/RFC.md?plain=1#L51) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`busybox_symlink_anthropic_end_to_end_with_inclusive_usage_and_headers`](../crates/anix-providers/tests/e2e_mock.rs#L328) | [§2](../docs/RFC.md?plain=1#L51) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`http_429_and_529_map_to_retryable_errors_and_400_is_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L394) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`coprocess_mode_serves_multiple_requests_survives_error_and_exits_0_on_eof`](../crates/anix-providers/tests/e2e_mock.rs#L458) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`http_400_image_rejection_maps_to_unsupported_media_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L615) | [§5.5](../docs/RFC.md?plain=1#L295) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`session_affinity_headers_and_body_mapping_across_all_dialects`](../crates/anix-providers/tests/e2e_mock.rs#L674) | [§9.2](../docs/RFC.md?plain=1#L606) |

## `crates/anix-tools/tests/rfc_control.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L50) | [§7.2](../docs/RFC.md?plain=1#L458) · [§6.1](../docs/RFC.md?plain=1#L331) |
| [`aclr_without_a_summary_omits_the_field_at_each_valid_turn_id_boundary`](../crates/anix-tools/tests/rfc_control.rs#L67) | [§6.1](../docs/RFC.md?plain=1#L331) · [§7.2](../docs/RFC.md?plain=1#L458) |
| [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L77) | [§6.1](../docs/RFC.md?plain=1#L331) · [§7.2](../docs/RFC.md?plain=1#L458) |
| [`fold_control_frames_stay_within_64_kib_even_after_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L96) | [§7.2](../docs/RFC.md?plain=1#L458) |
| [`aview_resizes_portrait_landscape_and_square_images_only_when_needed`](../crates/anix-tools/tests/rfc_control.rs#L148) | [§7.2](../docs/RFC.md?plain=1#L458) |
| [`aview_file_input_and_stdin_agree_on_image_dimensions`](../crates/anix-tools/tests/rfc_control.rs#L171) | [§7.2](../docs/RFC.md?plain=1#L458) |
| [`aview_preserves_image_content_and_orientation_in_the_published_png`](../crates/anix-tools/tests/rfc_control.rs#L186) | [§7.2](../docs/RFC.md?plain=1#L458) |
| [`undecodable_input_or_invalid_options_do_not_emit_media`](../crates/anix-tools/tests/rfc_control.rs#L230) | [§7.2](../docs/RFC.md?plain=1#L458) |
| [`aview_selects_pdf_pages_with_their_actual_geometry_and_colors`](../crates/anix-tools/tests/rfc_control.rs#L245) | [§7.2](../docs/RFC.md?plain=1#L458) |
| [`aview_rejects_invalid_pdf_page_expressions_without_publishing_attachments`](../crates/anix-tools/tests/rfc_control.rs#L314) | [§7.2](../docs/RFC.md?plain=1#L458) |

## `crates/msh/tests/rfc_shell.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L34) | [§7.1](../docs/RFC.md?plain=1#L432) |
| [`ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites`](../crates/msh/tests/rfc_shell.rs#L65) | [§7.1](../docs/RFC.md?plain=1#L432) |
| [`line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text`](../crates/msh/tests/rfc_shell.rs#L106) | [§7.1](../docs/RFC.md?plain=1#L432) |
| [`byte_limit_independent_from_line_limit_preserves_multibyte_file`](../crates/msh/tests/rfc_shell.rs#L142) | [§7.1](../docs/RFC.md?plain=1#L432) |
| [`cli_applies_environment_limits_to_cleaned_lines_and_bytes_before_saving_output`](../crates/msh/tests/rfc_shell.rs#L177) | [§7.1](../docs/RFC.md?plain=1#L432) |
| [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L219) | [§7.1](../docs/RFC.md?plain=1#L432) |
| [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L241) | [§7.1](../docs/RFC.md?plain=1#L432) |
| [`timeout_reports_124_before_a_finite_longer_command_completes`](../crates/msh/tests/rfc_shell.rs#L266) | [§6.5](../docs/RFC.md?plain=1#L415) · [§7.1](../docs/RFC.md?plain=1#L432) |
| [`unconfigured_msh_folds_only_above_200_lines_or_8192_bytes`](../crates/msh/tests/rfc_shell.rs#L280) | [§7.1](../docs/RFC.md?plain=1#L432) |

## `tests/test_approval.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_tty_yes_and_no_use_the_terminal_while_stdin_remains_model_input`](../tests/test_approval.py#L131) | [§4.1](../docs/RFC.md?plain=1#L167) · [§5.1](../docs/RFC.md?plain=1#L241) · [§5.3](../docs/RFC.md?plain=1#L282) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_always_applies_to_one_executable_and_resets_on_wal_resume`](../tests/test_approval.py#L156) | [§5.1](../docs/RFC.md?plain=1#L241) · [§5.2](../docs/RFC.md?plain=1#L250) · [§5.3](../docs/RFC.md?plain=1#L282) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`test_editor_receives_private_original_and_only_the_edited_command_runs`](../tests/test_approval.py#L167) | [§5.3](../docs/RFC.md?plain=1#L282) |
| [`test_failed_or_empty_edit_removes_the_file_and_runs_neither_command`](../tests/test_approval.py#L194) | [§5.3](../docs/RFC.md?plain=1#L282) |
| [`test_yes_authorizes_only_one_call_in_a_batch`](../tests/test_approval.py#L206) | [§5.1](../docs/RFC.md?plain=1#L241) · [§5.3](../docs/RFC.md?plain=1#L282) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`test_no_controlling_terminal_exits_75_with_command_and_no_tool_execution`](../tests/test_approval.py#L219) | [§5.2](../docs/RFC.md?plain=1#L250) · [§5.4](../docs/RFC.md?plain=1#L291) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_external_escalation_hands_the_decision_to_the_terminal`](../tests/test_approval.py#L234) | [§4.1](../docs/RFC.md?plain=1#L167) · [§5.1](../docs/RFC.md?plain=1#L241) · [§5.2](../docs/RFC.md?plain=1#L250) · [§5.3](../docs/RFC.md?plain=1#L282) |
| [`test_external_decider_receives_one_utf8_frame_with_the_real_caller_identity`](../tests/test_approval.py#L247) | [§5.2](../docs/RFC.md?plain=1#L250) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_external_denial_trims_reason_or_supplies_default_and_continues`](../tests/test_approval.py#L265) | [§5.2](../docs/RFC.md?plain=1#L250) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_external_error_and_timeout_surface_stderr_and_end_the_cycle_with_75`](../tests/test_approval.py#L285) | [§5.2](../docs/RFC.md?plain=1#L250) · [§6.3](../docs/RFC.md?plain=1#L397) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_unavailable_external_decider_exits_75_and_records_the_failed_approval`](../tests/test_approval.py#L307) | [§5.2](../docs/RFC.md?plain=1#L250) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_auto_deny_and_allowlisted_commands_skip_the_external_decider`](../tests/test_approval.py#L319) | [§5.1](../docs/RFC.md?plain=1#L241) · [§5.2](../docs/RFC.md?plain=1#L250) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_ctrl_c_at_the_approval_prompt_returns_to_the_same_shell`](../tests/test_approval.py#L345) | [§5.3](../docs/RFC.md?plain=1#L282) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_interrupt_during_builtin_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L353) | [§5.2](../docs/RFC.md?plain=1#L250) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_interrupt_during_external_decision_identifies_the_decider_in_trace`](../tests/test_approval.py#L379) | [§5.2](../docs/RFC.md?plain=1#L250) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.5](../docs/RFC.md?plain=1#L681) |

## `tests/test_distribution.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L121) | [§2](../docs/RFC.md?plain=1#L51) · [§10.3](../docs/RFC.md?plain=1#L754) |
| [`test_doctor_detects_unavailable_pipes_and_unusable_file_locking`](../tests/test_distribution.py#L164) | [§4.1](../docs/RFC.md?plain=1#L167) · [§10.3](../docs/RFC.md?plain=1#L754) |
| [`test_usage_line_resets_dim_before_the_final_newline_on_a_terminal`](../tests/test_distribution.py#L218) | [§4.1](../docs/RFC.md?plain=1#L167) |
| [`test_installed_bash_script_accepts_args_on_path_and_by_relative_name_without_tty_eof`](../tests/test_distribution.py#L248) | [§2](../docs/RFC.md?plain=1#L51) · [§3.1](../docs/RFC.md?plain=1#L69) · [§4.1](../docs/RFC.md?plain=1#L167) · [§9.2](../docs/RFC.md?plain=1#L606) · [§10.3](../docs/RFC.md?plain=1#L754) |
| [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L272) | [§2](../docs/RFC.md?plain=1#L51) · [§3.1](../docs/RFC.md?plain=1#L69) · [§4.1](../docs/RFC.md?plain=1#L167) · [§4.4](../docs/RFC.md?plain=1#L222) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.6](../docs/RFC.md?plain=1#L706) · [§10.3](../docs/RFC.md?plain=1#L754) |

## `tests/test_msh_jobs.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_immediate_handoff_closes_side_channels_and_publishes_the_real_exit`](../tests/test_msh_jobs.py#L87) | [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) |
| [`test_positive_deadline_returns_a_clean_bounded_tail_and_keeps_the_complete_log`](../tests/test_msh_jobs.py#L116) | [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) |
| [`test_foreground_completion_returns_real_status_and_removes_job_files`](../tests/test_msh_jobs.py#L140) | [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) |
| [`test_foreground_int_and_term_clean_up_in_both_timeout_modes`](../tests/test_msh_jobs.py#L149) | [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§7.1](../docs/RFC.md?plain=1#L432) |
| [`test_zero_with_explicit_kill_waits_one_second_and_leaves_no_job`](../tests/test_msh_jobs.py#L180) | [§6.5](../docs/RFC.md?plain=1#L415) · [§7.1](../docs/RFC.md?plain=1#L432) |
| [`test_long_commands_execute_their_suffix_and_handoff_with_a_single_bounded_frame`](../tests/test_msh_jobs.py#L191) | [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) |
| [`test_msh_tool_schema_and_per_call_options_do_not_leak_into_the_next_call`](../tests/test_msh_jobs.py#L252) | [§7.1](../docs/RFC.md?plain=1#L432) · [§8.2](../docs/RFC.md?plain=1#L492) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`test_single_call_never_exceeds_a_configured_maximum_below_the_default_wait`](../tests/test_msh_jobs.py#L267) | [§7.1](../docs/RFC.md?plain=1#L432) · [§8.2](../docs/RFC.md?plain=1#L492) |
| [`test_environment_maximum_overrides_toml_for_each_explicit_request`](../tests/test_msh_jobs.py#L273) | [§7.1](../docs/RFC.md?plain=1#L432) · [§8.2](../docs/RFC.md?plain=1#L492) |
| [`test_background_completion_is_reported_once_before_inference_and_replays_from_wal`](../tests/test_msh_jobs.py#L280) | [§6.2](../docs/RFC.md?plain=1#L358) · [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_positive_timeout_dispatches_the_next_request_while_the_job_is_alive`](../tests/test_msh_jobs.py#L284) | [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_terminal_title_and_overwritten_progress_are_cleaned_from_the_completion_notice`](../tests/test_msh_jobs.py#L288) | [§6.2](../docs/RFC.md?plain=1#L358) · [§7.1](../docs/RFC.md?plain=1#L432) · [§7.2](../docs/RFC.md?plain=1#L458) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`test_finished_group_without_an_exit_file_reports_null_exactly_once`](../tests/test_msh_jobs.py#L342) | [§7.2](../docs/RFC.md?plain=1#L458) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_hard_kill_grace_waits_for_a_living_descendant_without_repeated_polling`](../tests/test_msh_jobs.py#L386) | [§6.3](../docs/RFC.md?plain=1#L397) · [§7.1](../docs/RFC.md?plain=1#L432) |

## `tests/test_provider_dialects.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L111) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L147) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_coprocess_reopens_replaced_media_and_recovers_after_unlink`](../tests/test_provider_dialects.py#L174) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_oneshot_unreadable_and_missing_paths_fail_before_dispatch`](../tests/test_provider_dialects.py#L197) | [§6.5](../docs/RFC.md?plain=1#L415) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_upstream_media_rejection_is_distinct_from_auth_and_transient_failures`](../tests/test_provider_dialects.py#L215) | [§5.5](../docs/RFC.md?plain=1#L295) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_partial_text_then_upstream_eof_yields_error_and_coprocess_accepts_next_request`](../tests/test_provider_dialects.py#L242) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_unsupported_ir_version_is_diagnosed_before_http_in_both_modes`](../tests/test_provider_dialects.py#L271) | [§6.5](../docs/RFC.md?plain=1#L415) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_fragmented_tool_arguments_produce_one_call_with_the_original_id`](../tests/test_provider_dialects.py#L287) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_inclusive_usage_preserves_wide_counts_and_available_cache_details`](../tests/test_provider_dialects.py#L329) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_wal_session_reaches_http_across_cli_restarts_and_provider_switches`](../tests/test_provider_dialects.py#L355) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_session_routing_works_in_oneshot_and_per_envelope_in_coprocess`](../tests/test_provider_dialects.py#L407) | [§1.4](../docs/RFC.md?plain=1#L33) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_header_rename_and_disable_apply_only_to_the_current_request`](../tests/test_provider_dialects.py#L433) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_responses_cache_key_has_native_string_boolean_and_null_semantics`](../tests/test_provider_dialects.py#L456) | [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_chat_cache_fields_are_independent_opt_ins_and_anthropic_keeps_them_out_of_the_body`](../tests/test_provider_dialects.py#L474) | [§9.2](../docs/RFC.md?plain=1#L606) |

## `tests/test_provider_streaming.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_coprocess_overrides_apply_to_one_frame_then_environment_defaults_resume`](../tests/test_provider_streaming.py#L102) | [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_wire_trace_keeps_raw_msh_output_while_the_model_receives_cleaned_text`](../tests/test_provider_streaming.py#L198) | [§4.3](../docs/RFC.md?plain=1#L208) · [§7.1](../docs/RFC.md?plain=1#L432) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L230) | [§6.5](../docs/RFC.md?plain=1#L415) · [§7.1](../docs/RFC.md?plain=1#L432) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L262) | [§6.1](../docs/RFC.md?plain=1#L331) · [§6.5](../docs/RFC.md?plain=1#L415) · [§7.1](../docs/RFC.md?plain=1#L432) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L291) | [§7.1](../docs/RFC.md?plain=1#L432) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L317) | [§4.1](../docs/RFC.md?plain=1#L167) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L350) | [§4.3](../docs/RFC.md?plain=1#L208) · [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) · [§9.2](../docs/RFC.md?plain=1#L606) |
| [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L409) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_executable_script_on_path_and_by_relative_name_obeys_tty_eof_rules`](../tests/test_provider_streaming.py#L442) | [§3.1](../docs/RFC.md?plain=1#L69) · [§3.2](../docs/RFC.md?plain=1#L93) · [§4.1](../docs/RFC.md?plain=1#L167) · [§4.2](../docs/RFC.md?plain=1#L193) |
| [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L492) | [§3.1](../docs/RFC.md?plain=1#L69) · [§4.1](../docs/RFC.md?plain=1#L167) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L549) | [§4.1](../docs/RFC.md?plain=1#L167) · [§6.3](../docs/RFC.md?plain=1#L397) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |
| [`test_inbox_commits_one_message_then_interrupts_the_next_in_the_same_coprocess`](../tests/test_provider_streaming.py#L589) | [§4.1](../docs/RFC.md?plain=1#L167) · [§4.2](../docs/RFC.md?plain=1#L193) · [§6.2](../docs/RFC.md?plain=1#L358) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.2](../docs/RFC.md?plain=1#L606) · [§9.5](../docs/RFC.md?plain=1#L681) |

## `tests/test_shell_pipeline.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_aclr_and_anix_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L72) | [§7.2](../docs/RFC.md?plain=1#L458) · [§6.1](../docs/RFC.md?plain=1#L331) · [§6.2](../docs/RFC.md?plain=1#L358) |
| [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L122) | [§6.2](../docs/RFC.md?plain=1#L358) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`test_archived_wal_growth_keeps_resume_reads_bounded_at_the_latest_checkpoint`](../tests/test_shell_pipeline.py#L167) | [§6.2](../docs/RFC.md?plain=1#L358) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L221) | [§9.4](../docs/RFC.md?plain=1#L668) · [§7.1](../docs/RFC.md?plain=1#L432) |
| [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L294) | [§3.1](../docs/RFC.md?plain=1#L69) · [§4.1](../docs/RFC.md?plain=1#L167) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`test_cancelling_input_does_not_need_eof_or_dispatch_the_partial_message`](../tests/test_shell_pipeline.py#L320) | [§4.1](../docs/RFC.md?plain=1#L167) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) |
| [`test_msh_timeout_signals_the_whole_group_and_grants_two_seconds_before_kill`](../tests/test_shell_pipeline.py#L333) | [§7.1](../docs/RFC.md?plain=1#L432) · [§6.4](../docs/RFC.md?plain=1#L405) |
| [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L371) | [§6.2](../docs/RFC.md?plain=1#L358) · [§6.4](../docs/RFC.md?plain=1#L405) · [§6.5](../docs/RFC.md?plain=1#L415) · [§9.1](../docs/RFC.md?plain=1#L568) |
| [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L424) | [§4.1](../docs/RFC.md?plain=1#L167) · [§4.3](../docs/RFC.md?plain=1#L208) |
| [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L464) | [§4.1](../docs/RFC.md?plain=1#L167) · [§4.3](../docs/RFC.md?plain=1#L208) |
| [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L529) | [§2](../docs/RFC.md?plain=1#L51) |
| [`test_nested_anix_uses_own_model_wal_and_budget_and_inherits_cli_auto`](../tests/test_shell_pipeline.py#L593) | [§3.4](../docs/RFC.md?plain=1#L138) · [§5.1](../docs/RFC.md?plain=1#L241) |
| [`test_nested_anix_inherits_cli_deny_over_its_own_auto_frontmatter`](../tests/test_shell_pipeline.py#L597) | [§3.4](../docs/RFC.md?plain=1#L138) · [§5.1](../docs/RFC.md?plain=1#L241) |
