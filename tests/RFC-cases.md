# v1.1 RFC 逐测试索引

每行对应一个测试函数，链接分别指向测试声明和规范原文。参数化用例共用所在函数的条款。

v1.0 的 198 个用例保存在[上一版测试索引](https://github.com/Luthics/anix/blob/45e55a9f16f6bead6a933c67051b94b0acb753a0/tests/RFC-cases.md)。

运行 `python3 scripts/rfc-test-index.py` 更新此文件。CI 使用 `--check` 校验。

## `crates/anix-core/src/cli.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`parses_script_and_args_with_double_dash`](../crates/anix-core/src/cli.rs#L209) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) |
| [`parses_direct_and_quoted_script_args_without_double_dash`](../crates/anix-core/src/cli.rs#L227) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) |
| [`trust_subcommand_does_not_conflict_with_script_args`](../crates/anix-core/src/cli.rs#L249) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§8.3](../docs/v1.1.0-RFC.md?plain=1#L508) |

## `crates/anix-core/src/platform/mod.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`probes_platform_capabilities`](../crates/anix-core/src/platform/mod.rs#L119) | [§10.2](../docs/v1.1.0-RFC.md?plain=1#L698) |

## `crates/anix-core/src/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage`](../crates/anix-core/src/wal.rs#L715) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |

## `crates/anix-core/tests/rfc_3_2/cli.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`crlf_script_reaches_the_provider_as_the_intended_system_message`](../crates/anix-core/tests/rfc_3_2/cli.rs#L44) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position`](../crates/anix-core/tests/rfc_3_2/cli.rs#L67) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`invalid_file_bytes_and_yaml_report_usage_errors_to_stderr`](../crates/anix-core/tests/rfc_3_2/cli.rs#L90) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`yaml_block_scalar_chomping_survives_the_frontmatter_boundary`](../crates/anix-core/tests/rfc_3_2/cli.rs#L107) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`duplicate_yaml_keys_are_invalid_even_inside_open_maps`](../crates/anix-core/tests/rfc_3_2/cli.rs#L150) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`yaml_aliases_can_supply_typed_provider_params_from_an_extension_field`](../crates/anix-core/tests/rfc_3_2/cli.rs#L163) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`vision_requires_a_yaml_boolean_and_leaves_provider_params_open`](../crates/anix-core/tests/rfc_3_2/cli.rs#L181) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |

## `crates/anix-core/tests/rfc_3_2/encoding.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`all_256_single_bytes_inside_a_plain_prompt`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L11) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |
| [`valid_multibyte_boundaries_survive_file_decoding`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L37) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |
| [`malformed_sequences_are_rejected_in_every_script_region`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L64) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |
| [`eof_at_each_interior_byte_of_a_utf8_scalar`](../crates/anix-core/tests/rfc_3_2/encoding.rs#L104) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |

## `crates/anix-core/tests/rfc_3_2/grammar.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`layout_matrix_for_line_ending_shebang_header_body_and_eof`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L9) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |
| [`only_a_first_line_beginning_with_hash_bang_is_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L46) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |
| [`opening_delimiter_requires_the_designated_line_and_exact_marker`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L69) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |
| [`first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L92) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |
| [`yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L115) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) |
| [`incomplete_delimiters_and_malformed_yaml_fail_at_the_header`](../crates/anix-core/tests/rfc_3_2/grammar.rs#L145) | [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) |

## `crates/anix-core/tests/rfc_runtime/compaction.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L30) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L99) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) |
| [`retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L139) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L198) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L321) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L374) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`fresh_window_counts_utf8_bytes_in_groups_of_three`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L443) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) |
| [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L459) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L489) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) |
| [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L548) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L610) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) |
| [`external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L870) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`empty_or_failed_auto_summary_keeps_the_subwindow_and_reports_a_tool_error`](../crates/anix-core/tests/rfc_runtime/compaction.rs#L900) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |

## `crates/anix-core/tests/rfc_runtime/configuration.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`agent_name_has_exactly_the_documented_ascii_alphabet`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L11) | [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L34) | [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) |
| [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L91) | [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) |
| [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L116) | [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L181) | [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) · [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) |
| [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L347) | [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L386) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L431) | [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) |
| [`project_search_stops_at_nearest_configuration_git_or_home_boundary`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L465) | [§8.1](../docs/v1.1.0-RFC.md?plain=1#L450) |
| [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L516) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L571) | [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§5.1](../docs/v1.1.0-RFC.md?plain=1#L225) |
| [`invalid_resolved_environment_values_exit_two_before_contacting_the_provider`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L618) | [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L646) | [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L691) | [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`cli_params_preserve_json_literals_and_the_exact_fallback_string`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L722) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`malformed_cli_param_stops_before_provider_or_wal_creation`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L759) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L784) | [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§8.1](../docs/v1.1.0-RFC.md?plain=1#L450) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L849) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L920) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`each_xdg_directory_resolves_independently_when_other_overrides_are_absent`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L986) | [§8.1](../docs/v1.1.0-RFC.md?plain=1#L450) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§8.3](../docs/v1.1.0-RFC.md?plain=1#L508) |
| [`selected_model_changes_the_provider_environment_without_entering_the_ir`](../crates/anix-core/tests/rfc_runtime/configuration.rs#L1045) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-core/tests/rfc_runtime/control.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/anix-core/tests/rfc_runtime/control.rs#L24) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order`](../crates/anix-core/tests/rfc_runtime/control.rs#L93) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |

## `crates/anix-core/tests/rfc_runtime/fd.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment`](../crates/anix-core/tests/rfc_runtime/fd.rs#L61) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input`](../crates/anix-core/tests/rfc_runtime/fd.rs#L94) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider`](../crates/anix-core/tests/rfc_runtime/fd.rs#L136) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |

## `crates/anix-core/tests/rfc_runtime/hooks.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L10) | [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`pre_context_failure_forwards_stderr_and_stops_before_inference`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L81) | [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) |
| [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L107) | [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L155) | [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L195) | [§9.4](../docs/v1.1.0-RFC.md?plain=1#L621) |
| [`tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L258) | [§1.4](../docs/v1.1.0-RFC.md?plain=1#L33) · [§8.1](../docs/v1.1.0-RFC.md?plain=1#L450) · [§9.6](../docs/v1.1.0-RFC.md?plain=1#L657) |
| [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L297) | [§9.6](../docs/v1.1.0-RFC.md?plain=1#L657) |
| [`an_empty_merged_tips_index_costs_no_prompt_bytes`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L345) | [§9.6](../docs/v1.1.0-RFC.md?plain=1#L657) |
| [`mcp_guide_follows_anix_xdg_and_project_precedence_without_disclosing_tool_schemas`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L376) | [§1.4](../docs/v1.1.0-RFC.md?plain=1#L33) · [§8.1](../docs/v1.1.0-RFC.md?plain=1#L450) · [§9.6](../docs/v1.1.0-RFC.md?plain=1#L657) |
| [`runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L467) | [§4.4](../docs/v1.1.0-RFC.md?plain=1#L208) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project`](../crates/anix-core/tests/rfc_runtime/hooks.rs#L524) | [§4.4](../docs/v1.1.0-RFC.md?plain=1#L208) · [§8.3](../docs/v1.1.0-RFC.md?plain=1#L508) · [§9.3](../docs/v1.1.0-RFC.md?plain=1#L615) · [§9.4](../docs/v1.1.0-RFC.md?plain=1#L621) · [§9.6](../docs/v1.1.0-RFC.md?plain=1#L657) |

## `crates/anix-core/tests/rfc_runtime/inbox.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L19) | [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`a_publisher_can_queue_work_while_the_initial_cli_task_is_running`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L120) | [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L179) | [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`oneshot_nonzero_after_done_discards_uncommitted_calls_and_preserves_later_jobs`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L265) | [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication`](../crates/anix-core/tests/rfc_runtime/inbox.rs#L336) | [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.3](../docs/v1.1.0-RFC.md?plain=1#L380) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |

## `crates/anix-core/tests/rfc_runtime/io.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/anix-core/tests/rfc_runtime/io.rs#L8) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`args_stdin_and_sparse_extra_descriptors_are_ordered_and_trace_is_separate`](../crates/anix-core/tests/rfc_runtime/io.rs#L35) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/anix-core/tests/rfc_runtime/io.rs#L87) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§4.3](../docs/v1.1.0-RFC.md?plain=1#L194) |
| [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/anix-core/tests/rfc_runtime/io.rs#L149) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§4.3](../docs/v1.1.0-RFC.md?plain=1#L194) |
| [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/anix-core/tests/rfc_runtime/io.rs#L186) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) |
| [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/anix-core/tests/rfc_runtime/io.rs#L207) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§4.3](../docs/v1.1.0-RFC.md?plain=1#L194) |
| [`shell_environment_contains_only_observability_effective_approval_and_routed_fds`](../crates/anix-core/tests/rfc_runtime/io.rs#L280) | [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§4.3](../docs/v1.1.0-RFC.md?plain=1#L194) |

## `crates/anix-core/tests/rfc_runtime/provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr`](../crates/anix-core/tests/rfc_runtime/provider.rs#L20) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`nonretryable_error_preserves_provider_stderr_without_another_attempt`](../crates/anix-core/tests/rfc_runtime/provider.rs#L136) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/anix-core/tests/rfc_runtime/provider.rs#L147) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`provider_timeout_maps_to_124_without_a_retry`](../crates/anix-core/tests/rfc_runtime/provider.rs#L158) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch`](../crates/anix-core/tests/rfc_runtime/provider.rs#L164) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |

## `crates/anix-core/tests/rfc_runtime/signals.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/anix-core/tests/rfc_runtime/signals.rs#L12) | [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/anix-core/tests/rfc_runtime/signals.rs#L50) | [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) |
| [`usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference`](../crates/anix-core/tests/rfc_runtime/signals.rs#L117) | [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) |
| [`interruption_waits_three_seconds_then_stops_a_provider_that_handles_term_without_exiting`](../crates/anix-core/tests/rfc_runtime/signals.rs#L170) | [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`abrupt_parent_exit_stops_the_active_provider_or_shell_child`](../crates/anix-core/tests/rfc_runtime/signals.rs#L210) | [§10.2](../docs/v1.1.0-RFC.md?plain=1#L698) |

## `crates/anix-core/tests/rfc_runtime/vision.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`vision_resolves_from_cli_env_script_selected_provider_then_defaults`](../crates/anix-core/tests/rfc_runtime/vision.rs#L10) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.3](../docs/v1.1.0-RFC.md?plain=1#L111) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§8.2](../docs/v1.1.0-RFC.md?plain=1#L457) |
| [`native_vision_keeps_the_attachment_through_resume_and_never_calls_a_bridge`](../crates/anix-core/tests/rfc_runtime/vision.rs#L161) | [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`no_native_vision_without_a_bridge_returns_the_documented_error_through_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L179) | [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`bridge_gets_one_utf8_json_line_then_eof_and_its_description_survives_resume`](../crates/anix-core/tests/rfc_runtime/vision.rs#L189) | [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`bridge_nonzero_exit_and_timeout_both_return_stderr_as_a_recoverable_tool_error`](../crates/anix-core/tests/rfc_runtime/vision.rs#L216) | [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |

## `crates/anix-core/tests/rfc_runtime/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`legacy_wal_without_vision_or_fold_usage_resumes_without_rewriting_the_prefix`](../crates/anix-core/tests/rfc_runtime/wal.rs#L23) | [§1.4](../docs/v1.1.0-RFC.md?plain=1#L33) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`repeated_resume_appends_one_vision_update_per_change_and_reuses_frozen_text`](../crates/anix-core/tests/rfc_runtime/wal.rs#L59) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |

## `crates/anix-core/tests/rfc_runtime/wal_replay.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`every_partial_byte_write_of_a_final_record_fails_without_modifying_history`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L35) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`resuming_a_complete_last_object_without_lf_never_joins_two_wal_records`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L59) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L109) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L137) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L173) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |
| [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L196) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L215) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L365) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L396) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L438) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`initial_section_interruption_never_dispatches_without_a_frozen_system_message`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L515) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/anix-core/tests/rfc_runtime/wal_replay.rs#L569) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |

## `crates/anix-core/tests/v1_1_e2e.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`trace_fd_explicit_only_no_implicit_fd3_and_cycle_events`](../crates/anix-core/tests/v1_1_e2e.rs#L161) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`external_approval_decider_trust_gate_and_exit_75`](../crates/anix-core/tests/v1_1_e2e.rs#L293) | [§5.2](../docs/v1.1.0-RFC.md?plain=1#L234) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§8.3](../docs/v1.1.0-RFC.md?plain=1#L508) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`vision_false_blocks_wal_pollution_and_vision_bridge_translates_and_mcp_tip_present`](../crates/anix-core/tests/v1_1_e2e.rs#L462) | [§4.4](../docs/v1.1.0-RFC.md?plain=1#L208) · [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.6](../docs/v1.1.0-RFC.md?plain=1#L657) |
| [`aclr_auto_summary_tail_fold_and_coprocess_inbox_lifecycle`](../crates/anix-core/tests/v1_1_e2e.rs#L596) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |

## `crates/anix-proto/src/ctl.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`ctl_frames_round_trip`](../crates/anix-proto/src/ctl.rs#L36) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |

## `crates/anix-proto/src/ir.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`ir_jsonl_header_and_messages_round_trip`](../crates/anix-proto/src/ir.rs#L360) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |

## `crates/anix-proto/src/provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`provider_events_round_trip`](../crates/anix-proto/src/provider.rs#L231) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-proto/src/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`wal_events_round_trip`](../crates/anix-proto/src/wal.rs#L97) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |

## `crates/anix-proto/tests/rfc_ir.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/anix-proto/tests/rfc_ir.rs#L36) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`meta_and_header_options_have_their_declared_wire_types`](../crates/anix-proto/tests/rfc_ir.rs#L68) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`validated_ir_only_serializes_objects_for_arguments_and_provider_opaque`](../crates/anix-proto/tests/rfc_ir.rs#L103) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references`](../crates/anix-proto/tests/rfc_ir.rs#L156) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/anix-proto/tests/rfc_ir.rs#L185) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`a_single_tool_result_cannot_close_two_calls_with_the_same_id`](../crates/anix-proto/tests/rfc_ir.rs#L213) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/anix-proto/tests/rfc_ir.rs#L223) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state`](../crates/anix-proto/tests/rfc_ir.rs#L258) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |
| [`ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared`](../crates/anix-proto/tests/rfc_ir.rs#L277) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) |
| [`coprocess_envelope_wraps_the_same_ir_without_moving_model_into_history`](../crates/anix-proto/tests/rfc_ir.rs#L297) | [§1.4](../docs/v1.1.0-RFC.md?plain=1#L33) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-proto/tests/rfc_provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values`](../crates/anix-proto/tests/rfc_provider.rs#L7) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`usage_requires_both_totals_even_when_all_three_details_are_present`](../crates/anix-proto/tests/rfc_provider.rs#L56) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-providers/src/anthropic.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`resolves_anthropic_endpoint_urls`](../crates/anix-providers/src/anthropic.rs#L938) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`coalesces_adjacent_user_and_tool_results_and_replays_signed_thinking`](../crates/anix-providers/src/anthropic.rs#L955) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`sse_normalizes_inclusive_usage_captures_thinking_signature_and_tool_use`](../crates/anix-providers/src/anthropic.rs#L1059) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`sse_overloaded_error_is_retryable`](../crates/anix-providers/src/anthropic.rs#L1138) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-providers/src/lib.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`resolves_dialect_from_argv0_symlink_and_version`](../crates/anix-providers/src/lib.rs#L218) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`resolves_dialect_from_subcommand_flag_and_env`](../crates/anix-providers/src/lib.rs#L238) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-providers/src/openai_compat.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`endpoint_url_appends_chat_completions_once`](../crates/anix-providers/src/openai_compat.rs#L884) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`translates_frozen_system_degraded_runtime_update_tools_and_params`](../crates/anix-providers/src/openai_compat.rs#L897) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`omits_tools_when_empty_and_supports_disabling_stream_options`](../crates/anix-providers/src/openai_compat.rs#L942) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`replays_opaque_reasoning_and_flushes_tool_media_after_batch`](../crates/anix-providers/src/openai_compat.rs#L964) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`sse_streams_reasoning_text_and_trailing_usage_after_finish_reason`](../crates/anix-providers/src/openai_compat.rs#L1038) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`sse_accumulates_multi_chunk_tool_calls_and_extra_content`](../crates/anix-providers/src/openai_compat.rs#L1086) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`sse_clean_eof_after_finish_reason_without_done_and_truncated_error`](../crates/anix-providers/src/openai_compat.rs#L1136) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`non_streaming_json_fallback_parses_message_and_inline_error`](../crates/anix-providers/src/openai_compat.rs#L1159) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-providers/src/openai_responses.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`builds_responses_request_with_frozen_instructions_and_reasoning_effort`](../crates/anix-providers/src/openai_responses.rs#L588) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`parses_responses_sse_text_tool_and_usage`](../crates/anix-providers/src/openai_responses.rs#L621) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-providers/tests/e2e_mock.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`version_flag_works_for_base_binary_and_busybox_symlinks`](../crates/anix-providers/tests/e2e_mock.rs#L201) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`unsupported_ir_version_exits_2`](../crates/anix-providers/tests/e2e_mock.rs#L231) | [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`busybox_symlink_openai_responses_end_to_end`](../crates/anix-providers/tests/e2e_mock.rs#L250) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`busybox_symlink_openai_compat_end_to_end_and_unauthenticated_local`](../crates/anix-providers/tests/e2e_mock.rs#L288) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`busybox_symlink_anthropic_end_to_end_with_inclusive_usage_and_headers`](../crates/anix-providers/tests/e2e_mock.rs#L327) | [§2](../docs/v1.1.0-RFC.md?plain=1#L44) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`http_429_and_529_map_to_retryable_errors_and_400_is_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L393) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`coprocess_mode_serves_multiple_requests_survives_error_and_exits_0_on_eof`](../crates/anix-providers/tests/e2e_mock.rs#L457) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`http_400_image_rejection_maps_to_unsupported_media_non_retryable`](../crates/anix-providers/tests/e2e_mock.rs#L614) | [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `crates/anix-tools/tests/rfc_control.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L40) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) · [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) |
| [`aclr_without_a_summary_omits_the_field_at_each_valid_turn_id_boundary`](../crates/anix-tools/tests/rfc_control.rs#L57) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |
| [`invalid_aclr_argv_never_submits_a_frame`](../crates/anix-tools/tests/rfc_control.rs#L67) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |
| [`fold_control_frames_stay_within_64_kib_even_after_json_escaping`](../crates/anix-tools/tests/rfc_control.rs#L86) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |
| [`aview_resizes_portrait_landscape_and_square_images_only_when_needed`](../crates/anix-tools/tests/rfc_control.rs#L138) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |
| [`aview_file_input_and_stdin_agree_on_image_dimensions`](../crates/anix-tools/tests/rfc_control.rs#L161) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |
| [`aview_preserves_image_content_and_orientation_in_the_published_png`](../crates/anix-tools/tests/rfc_control.rs#L176) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |
| [`undecodable_input_or_invalid_options_do_not_emit_media`](../crates/anix-tools/tests/rfc_control.rs#L220) | [§7.2](../docs/v1.1.0-RFC.md?plain=1#L428) |

## `crates/msh/tests/rfc_shell.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L34) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites`](../crates/msh/tests/rfc_shell.rs#L65) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text`](../crates/msh/tests/rfc_shell.rs#L106) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`byte_limit_independent_from_line_limit_preserves_multibyte_file`](../crates/msh/tests/rfc_shell.rs#L142) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`cli_applies_environment_limits_to_cleaned_lines_and_bytes_before_saving_output`](../crates/msh/tests/rfc_shell.rs#L177) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L219) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L241) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`timeout_reports_124_before_a_finite_longer_command_completes`](../crates/msh/tests/rfc_shell.rs#L266) | [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |
| [`unconfigured_msh_folds_only_above_200_lines_or_8192_bytes`](../crates/msh/tests/rfc_shell.rs#L279) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) |

## `tests/test_provider_dialects.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_frozen_prefix_and_runtime_update_remain_in_order_on_each_upstream`](../tests/test_provider_dialects.py#L131) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_user_and_tool_images_reach_upstream_with_declared_mime_and_exact_bytes`](../tests/test_provider_dialects.py#L167) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_coprocess_reopens_replaced_media_and_recovers_after_unlink`](../tests/test_provider_dialects.py#L194) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_oneshot_unreadable_and_missing_paths_fail_before_dispatch`](../tests/test_provider_dialects.py#L217) | [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_upstream_media_rejection_is_distinct_from_auth_and_transient_failures`](../tests/test_provider_dialects.py#L235) | [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_partial_text_then_upstream_eof_yields_error_and_coprocess_accepts_next_request`](../tests/test_provider_dialects.py#L262) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_unsupported_ir_version_is_diagnosed_before_http_in_both_modes`](../tests/test_provider_dialects.py#L291) | [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_fragmented_tool_arguments_produce_one_call_with_the_original_id`](../tests/test_provider_dialects.py#L307) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_inclusive_usage_preserves_wide_counts_and_available_cache_details`](../tests/test_provider_dialects.py#L349) | [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |

## `tests/test_provider_streaming.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_coprocess_overrides_apply_to_one_frame_then_environment_defaults_resume`](../tests/test_provider_streaming.py#L147) | [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L243) | [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_msh_timeout_is_delivered_to_the_next_provider_turn_as_a_tool_error`](../tests/test_provider_streaming.py#L275) | [§6.1](../docs/v1.1.0-RFC.md?plain=1#L315) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L304) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L330) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L363) | [§4.3](../docs/v1.1.0-RFC.md?plain=1#L194) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L422) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_executable_script_on_path_and_by_relative_name_obeys_tty_eof_rules`](../tests/test_provider_streaming.py#L456) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.2](../docs/v1.1.0-RFC.md?plain=1#L85) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) |
| [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L506) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L563) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.3](../docs/v1.1.0-RFC.md?plain=1#L380) · [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_inbox_commits_one_message_then_interrupts_the_next_in_the_same_coprocess`](../tests/test_provider_streaming.py#L604) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
