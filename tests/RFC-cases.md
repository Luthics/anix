# RFC 逐测试索引

每行对应一个测试函数，链接分别指向测试声明和规范原文。参数化用例共用所在函数的条款。

运行 `python3 scripts/rfc-test-index.py` 更新此文件；CI 使用 `--check` 校验。

## `crates/agent-tools/tests/rfc_control.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping`](../crates/agent-tools/tests/rfc_control.rs#L40) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) |
| [`invalid_aclr_argv_never_submits_a_frame`](../crates/agent-tools/tests/rfc_control.rs#L57) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |
| [`fold_control_frames_stay_within_64_kib_even_after_json_escaping`](../crates/agent-tools/tests/rfc_control.rs#L78) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |
| [`aview_resizes_portrait_landscape_and_square_images_only_when_needed`](../crates/agent-tools/tests/rfc_control.rs#L130) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |
| [`aview_file_input_and_stdin_agree_on_image_dimensions`](../crates/agent-tools/tests/rfc_control.rs#L153) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |
| [`aview_preserves_image_content_and_orientation_in_the_published_png`](../crates/agent-tools/tests/rfc_control.rs#L168) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |
| [`undecodable_input_or_invalid_options_do_not_emit_media`](../crates/agent-tools/tests/rfc_control.rs#L212) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |

## `crates/msh/tests/rfc_shell.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream`](../crates/msh/tests/rfc_shell.rs#L34) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites`](../crates/msh/tests/rfc_shell.rs#L65) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text`](../crates/msh/tests/rfc_shell.rs#L93) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`byte_limit_independent_from_line_limit_preserves_multibyte_file`](../crates/msh/tests/rfc_shell.rs#L134) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`cli_applies_environment_limits_to_cleaned_lines_and_bytes_before_saving_output`](../crates/msh/tests/rfc_shell.rs#L169) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`cli_forces_noninteractive_environment_merges_streams_and_propagates_status`](../crates/msh/tests/rfc_shell.rs#L220) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`cli_suppresses_raw_bytes_before_ansi_filtering_or_folding`](../crates/msh/tests/rfc_shell.rs#L242) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`timeout_reports_124_before_a_finite_longer_command_completes`](../crates/msh/tests/rfc_shell.rs#L267) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |

## `crates/os-agent-core/src/cli.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`parses_script_and_args_with_double_dash`](../crates/os-agent-core/src/cli.rs#L197) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) |
| [`parses_direct_and_quoted_script_args_without_double_dash`](../crates/os-agent-core/src/cli.rs#L215) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) |
| [`trust_subcommand_does_not_conflict_with_script_args`](../crates/os-agent-core/src/cli.rs#L238) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§8.3](../docs/v1.0.0-RFC.md?plain=1#L373) |

## `crates/os-agent-core/src/platform/mod.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`probes_platform_capabilities`](../crates/os-agent-core/src/platform/mod.rs#L87) | [§10.2](../docs/v1.0.0-RFC.md?plain=1#L524) |

## `crates/os-agent-core/src/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage`](../crates/os-agent-core/src/wal.rs#L711) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |

## `crates/os-agent-core/tests/rfc_3_2/cli.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`crlf_script_reaches_the_provider_as_the_intended_system_message`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L44) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L67) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`invalid_file_bytes_and_yaml_report_usage_errors_to_stderr`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L90) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`yaml_block_scalar_chomping_survives_the_frontmatter_boundary`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L107) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`duplicate_yaml_keys_are_invalid_even_inside_open_maps`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L150) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`yaml_aliases_can_supply_typed_provider_params_from_an_extension_field`](../crates/os-agent-core/tests/rfc_3_2/cli.rs#L163) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |

## `crates/os-agent-core/tests/rfc_3_2/encoding.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`all_256_single_bytes_inside_a_plain_prompt`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L11) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |
| [`valid_multibyte_boundaries_survive_file_decoding`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L37) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |
| [`malformed_sequences_are_rejected_in_every_script_region`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L64) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |
| [`eof_at_each_interior_byte_of_a_utf8_scalar`](../crates/os-agent-core/tests/rfc_3_2/encoding.rs#L104) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |

## `crates/os-agent-core/tests/rfc_3_2/grammar.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`layout_matrix_for_line_ending_shebang_header_body_and_eof`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L9) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |
| [`only_a_first_line_beginning_with_hash_bang_is_removed`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L46) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |
| [`opening_delimiter_requires_the_designated_line_and_exact_marker`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L69) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |
| [`first_closing_delimiter_ends_yaml_and_only_blank_body_edges_are_removed`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L92) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |
| [`yaml_12_scalars_extensions_and_open_params_coexist_with_known_fields`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L115) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) |
| [`incomplete_delimiters_and_malformed_yaml_fail_at_the_header`](../crates/os-agent-core/tests/rfc_3_2/grammar.rs#L145) | [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) |

## `crates/os-agent-core/tests/rfc_runtime/compaction.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L26) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L95) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) |
| [`retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L135) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`full_compaction_and_resume_account_for_every_call_without_inflating_watermark`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L194) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L297) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L350) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`fresh_window_counts_utf8_bytes_in_groups_of_three`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L421) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) |
| [`hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L437) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L467) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) |
| [`builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L526) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) |
| [`two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes`](../crates/os-agent-core/tests/rfc_runtime/compaction.rs#L583) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) |

## `crates/os-agent-core/tests/rfc_runtime/configuration.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`agent_name_has_exactly_the_documented_ascii_alphabet`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L11) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) |
| [`declared_fields_validate_type_enums_and_uint32_boundaries`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L34) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) |
| [`threshold_fraction_floors_absolute_threshold_and_disable_cases`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L91) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) |
| [`every_subset_of_five_layers_selects_highest_priority_model_and_output_limit`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L116) | [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) |
| [`scalar_settings_resolve_for_every_subset_of_the_five_override_layers`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L181) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) |
| [`output_limit_is_strictly_below_effective_context_budget_zero_omits_it`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L345) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`params_merge_by_key_and_cli_values_preserve_json_types`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L384) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`empty_context_override_is_distinct_from_an_unset_variable`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L429) | [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) |
| [`project_search_stops_at_nearest_configuration_git_or_home_boundary`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L463) | [§8.1](../docs/v1.0.0-RFC.md?plain=1#L326) |
| [`provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L518) | [§2](../docs/v1.0.0-RFC.md?plain=1#L23) · [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L572) | [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§5.1](../docs/v1.0.0-RFC.md?plain=1#L183) |
| [`invalid_resolved_environment_values_exit_two_before_contacting_the_provider`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L619) | [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`global_and_project_config_reject_unknown_keys_before_any_inference`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L646) | [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L691) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§3.2](../docs/v1.0.0-RFC.md?plain=1#L60) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`cli_params_preserve_json_literals_and_the_exact_fallback_string`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L722) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`malformed_cli_param_stops_before_provider_or_wal_creation`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L759) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L784) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§8.1](../docs/v1.0.0-RFC.md?plain=1#L326) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`only_the_selected_providers_defaults_feed_the_shallow_params_merge`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L849) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`a_named_provider_searches_path_then_libexec_while_a_path_executes_directly`](../crates/os-agent-core/tests/rfc_runtime/configuration.rs#L926) | [§2](../docs/v1.0.0-RFC.md?plain=1#L23) · [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |

## `crates/os-agent-core/tests/rfc_runtime/control.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`tool_control_frames_cannot_publish_invalid_media_references_to_wal_or_provider_ir`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L24) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L115) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order`](../crates/os-agent-core/tests/rfc_runtime/control.rs#L188) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |

## `crates/os-agent-core/tests/rfc_runtime/doctor.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_missing_or_non_executable_private_binary_is_identified`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L31) | [§10.3](../docs/v1.0.0-RFC.md?plain=1#L531) · [§2](../docs/v1.0.0-RFC.md?plain=1#L23) |
| [`doctor_distinguishes_required_missing_empty_and_fallback_credentials`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L62) | [§10.3](../docs/v1.0.0-RFC.md?plain=1#L531) |
| [`invalid_global_or_project_toml_and_unusable_runtime_directory_report_failure`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L91) | [§10.3](../docs/v1.0.0-RFC.md?plain=1#L531) |
| [`doctor_and_the_runtime_can_use_a_configured_default_provider_on_path`](../crates/os-agent-core/tests/rfc_runtime/doctor.rs#L114) | [§2](../docs/v1.0.0-RFC.md?plain=1#L23) · [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§8.2](../docs/v1.0.0-RFC.md?plain=1#L333) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§10.3](../docs/v1.0.0-RFC.md?plain=1#L531) |

## `crates/os-agent-core/tests/rfc_runtime/hooks.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L10) | [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`pre_context_failure_forwards_stderr_and_stops_before_inference`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L81) | [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) |
| [`post_tool_failure_stops_before_later_hooks_tools_and_provider_turns`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L107) | [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) |
| [`on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L143) | [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L180) | [§9.4](../docs/v1.0.0-RFC.md?plain=1#L461) |
| [`tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L243) | [§9.6](../docs/v1.0.0-RFC.md?plain=1#L488) |
| [`masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L282) | [§9.6](../docs/v1.0.0-RFC.md?plain=1#L488) |
| [`an_empty_merged_tips_index_costs_no_prompt_bytes`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L333) | [§9.6](../docs/v1.0.0-RFC.md?plain=1#L488) |
| [`background_tip_is_indexed_in_ascii_order_and_can_be_overridden_or_masked_individually`](../crates/os-agent-core/tests/rfc_runtime/hooks.rs#L367) | [§9.6](../docs/v1.0.0-RFC.md?plain=1#L488) |

## `crates/os-agent-core/tests/rfc_runtime/inbox.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L19) | [§4.2](../docs/v1.0.0-RFC.md?plain=1#L142) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`a_publisher_can_queue_work_while_the_initial_cli_task_is_running`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L120) | [§4.2](../docs/v1.0.0-RFC.md?plain=1#L142) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L188) | [§4.2](../docs/v1.0.0-RFC.md?plain=1#L142) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`provider_nonzero_after_done_keeps_the_failed_inbox_task_unacknowledged`](../crates/os-agent-core/tests/rfc_runtime/inbox.rs#L274) | [§4.2](../docs/v1.0.0-RFC.md?plain=1#L142) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |

## `crates/os-agent-core/tests/rfc_runtime/io.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`cli_version_usage_and_success_have_distinct_stream_contracts`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L8) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`args_stdin_and_sparse_extra_descriptors_are_ordered_and_trace_is_separate`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L35) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L87) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§4.3](../docs/v1.0.0-RFC.md?plain=1#L156) |
| [`tool_receives_control_channels_tools_path_and_no_parent_model_or_wal`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L149) | [§2](../docs/v1.0.0-RFC.md?plain=1#L23) · [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§4.3](../docs/v1.0.0-RFC.md?plain=1#L156) |
| [`script_arguments_accept_an_optional_separator_and_preserve_option_like_literals`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L186) | [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) |
| [`every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L207) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§4.3](../docs/v1.0.0-RFC.md?plain=1#L156) |
| [`shell_environment_contains_only_observability_effective_approval_and_routed_fds`](../crates/os-agent-core/tests/rfc_runtime/io.rs#L280) | [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§4.3](../docs/v1.0.0-RFC.md?plain=1#L156) |

## `crates/os-agent-core/tests/rfc_runtime/lifecycle.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`inbox_observes_only_published_files_in_ascii_order_and_moves_them_after_commit`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L11) | [§4.2](../docs/v1.0.0-RFC.md?plain=1#L142) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`deny_and_fail_on_deny_differ_in_exit_status_tool_reply_and_side_effects`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L89) | [§5.1](../docs/v1.0.0-RFC.md?plain=1#L183) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`max_turns_stops_before_another_request_and_success_on_last_turn_is_allowed`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L135) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`pre_context_hooks_merge_in_ascii_order_with_override_and_two_masks`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L163) | [§8.3](../docs/v1.0.0-RFC.md?plain=1#L373) · [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) |
| [`new_and_modified_project_hooks_require_explicit_trust`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L194) | [§8.3](../docs/v1.0.0-RFC.md?plain=1#L373) |
| [`project_provider_and_compactor_each_require_trust_after_a_content_change`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L225) | [§8.3](../docs/v1.0.0-RFC.md?plain=1#L373) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) |
| [`post_tool_filters_the_model_result_and_on_exit_failure_keeps_primary_status`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L302) | [§9.3](../docs/v1.0.0-RFC.md?plain=1#L455) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`frozen_context_lists_skill_metadata_and_tip_paths_but_leaves_bodies_on_disk`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L352) | [§4.4](../docs/v1.0.0-RFC.md?plain=1#L172) · [§9.4](../docs/v1.0.0-RFC.md?plain=1#L461) · [§9.6](../docs/v1.0.0-RFC.md?plain=1#L488) |
| [`provider_failure_after_partial_text_retries_with_unchanged_ir_and_emits_trace_evidence`](../crates/os-agent-core/tests/rfc_runtime/lifecycle.rs#L415) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |

## `crates/os-agent-core/tests/rfc_runtime/platform.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`strict_requires_landlock_and_best_effort_warns_when_the_kernel_lacks_it`](../crates/os-agent-core/tests/rfc_runtime/platform.rs#L7) | [§10.2](../docs/v1.0.0-RFC.md?plain=1#L524) |
| [`supported_landlock_allows_writes_to_the_three_roots_and_keeps_other_fixture_files_readable`](../crates/os-agent-core/tests/rfc_runtime/platform.rs#L30) | [§10.2](../docs/v1.0.0-RFC.md?plain=1#L524) |

## `crates/os-agent-core/tests/rfc_runtime/provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L21) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`nonretryable_error_preserves_provider_stderr_without_another_attempt`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L133) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L144) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`provider_timeout_maps_to_124_without_a_retry`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L155) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch`](../crates/os-agent-core/tests/rfc_runtime/provider.rs#L161) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |

## `crates/os-agent-core/tests/rfc_runtime/signals.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L12) | [§6.3](../docs/v1.0.0-RFC.md?plain=1#L268) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L50) | [§4.2](../docs/v1.0.0-RFC.md?plain=1#L142) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§6.3](../docs/v1.0.0-RFC.md?plain=1#L268) |
| [`usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L117) | [§6.3](../docs/v1.0.0-RFC.md?plain=1#L268) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) |
| [`interruption_waits_three_seconds_then_stops_a_provider_that_handles_term_without_exiting`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L170) | [§6.3](../docs/v1.0.0-RFC.md?plain=1#L268) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`abrupt_parent_exit_stops_the_active_provider_or_shell_child`](../crates/os-agent-core/tests/rfc_runtime/signals.rs#L210) | [§10.2](../docs/v1.0.0-RFC.md?plain=1#L524) |

## `crates/os-agent-core/tests/rfc_runtime/terminal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`tty_stdin_uses_args_immediately_or_reads_input_through_eof`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L169) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) |
| [`approval_choices_use_dev_tty_and_preserve_piped_stdin`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L201) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§5.1](../docs/v1.0.0-RFC.md?plain=1#L183) · [§5.2](../docs/v1.0.0-RFC.md?plain=1#L189) |
| [`unsuccessful_or_empty_terminal_edit_runs_neither_original_nor_edited_command`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L294) | [§5.2](../docs/v1.0.0-RFC.md?plain=1#L189) |
| [`always_approval_is_scoped_to_executable_and_pid_even_with_the_same_wal`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L333) | [§5.1](../docs/v1.0.0-RFC.md?plain=1#L183) · [§5.2](../docs/v1.0.0-RFC.md?plain=1#L189) |
| [`detached_session_requiring_approval_fails_with_42_without_executing_tool`](../crates/os-agent-core/tests/rfc_runtime/terminal.rs#L395) | [§5.3](../docs/v1.0.0-RFC.md?plain=1#L198) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |

## `crates/os-agent-core/tests/rfc_runtime/trace.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`events_distinguish_tool_approval_from_execution_and_record_an_applied_fold`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L16) | [§5.1](../docs/v1.0.0-RFC.md?plain=1#L183) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`regular_fd3_separates_events_deltas_and_wire_and_omits_provider_key`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L118) | [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) · [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) |
| [`wire_trace_only_writes_full_ir_into_a_private_file`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L147) | [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`journal_receives_events_on_fd3_fallback_and_never_receives_wire_ir`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L175) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status`](../crates/os-agent-core/tests/rfc_runtime/trace.rs#L216) | [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |

## `crates/os-agent-core/tests/rfc_runtime/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`every_partial_byte_write_of_a_final_record_fails_without_modifying_history`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L35) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`resuming_a_complete_last_object_without_lf_never_joins_two_wal_records`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L59) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L109) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L137) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L173) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`nonblocking_wal_lock_excludes_second_process_and_is_released_on_close`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L196) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L215) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L347) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`resume_compares_both_contract_fields_after_a_sparse_runtime_update`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L377) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L461) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L503) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`initial_section_interruption_never_dispatches_without_a_frozen_system_message`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L580) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`resume_uses_the_last_surviving_assistant_usage_for_the_hard_limit`](../crates/os-agent-core/tests/rfc_runtime/wal.rs#L634) | [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |

## `crates/os-agent-proto/src/ctl.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`ctl_frames_round_trip`](../crates/os-agent-proto/src/ctl.rs#L31) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |

## `crates/os-agent-proto/src/ir.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`ir_jsonl_header_and_messages_round_trip`](../crates/os-agent-proto/src/ir.rs#L352) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |

## `crates/os-agent-proto/src/provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`provider_events_round_trip`](../crates/os-agent-proto/src/provider.rs#L111) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |

## `crates/os-agent-proto/src/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`wal_events_round_trip`](../crates/os-agent-proto/src/wal.rs#L93) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |

## `crates/os-agent-proto/tests/rfc_ir.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`required_header_fields_and_types_cannot_be_dropped_or_moved`](../crates/os-agent-proto/tests/rfc_ir.rs#L35) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`meta_and_header_options_have_their_declared_wire_types`](../crates/os-agent-proto/tests/rfc_ir.rs#L67) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`validated_ir_only_serializes_objects_for_arguments_and_provider_opaque`](../crates/os-agent-proto/tests/rfc_ir.rs#L102) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`golden_ir_keeps_frozen_text_extensions_and_attachment_references_without_opening_files`](../crates/os-agent-proto/tests/rfc_ir.rs#L155) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`tool_replies_form_a_bijection_and_are_contiguous`](../crates/os-agent-proto/tests/rfc_ir.rs#L178) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`a_single_tool_result_cannot_close_two_calls_with_the_same_id`](../crates/os-agent-proto/tests/rfc_ir.rs#L206) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`required_message_fields_and_turn_id_types_hold_for_all_four_roles`](../crates/os-agent-proto/tests/rfc_ir.rs#L216) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state`](../crates/os-agent-proto/tests/rfc_ir.rs#L251) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared`](../crates/os-agent-proto/tests/rfc_ir.rs#L270) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |

## `crates/os-agent-proto/tests/rfc_provider.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values`](../crates/os-agent-proto/tests/rfc_provider.rs#L7) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`usage_requires_both_totals_even_when_all_three_details_are_present`](../crates/os-agent-proto/tests/rfc_provider.rs#L56) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |

## `crates/provider-openai-responses/tests/rfc_responses.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`maps_request_and_streams_text_reasoning_usage_and_done`](../crates/provider-openai-responses/tests/rfc_responses.rs#L131) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`runtime_update_stays_in_chronological_input_and_frozen_instructions_keep_their_bytes`](../crates/provider-openai-responses/tests/rfc_responses.rs#L174) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`repeated_sse_notifications_emit_one_complete_tool_call`](../crates/provider-openai-responses/tests/rfc_responses.rs#L213) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`transient_http_errors_are_retryable_but_adapter_never_retries_itself`](../crates/provider-openai-responses/tests/rfc_responses.rs#L241) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`missing_terminal_event_is_not_reported_as_success`](../crates/provider-openai-responses/tests/rfc_responses.rs#L257) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`unsupported_ir_version_and_missing_media_fail_before_http`](../crates/provider-openai-responses/tests/rfc_responses.rs#L277) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`unreadable_media_emits_a_permanent_error_before_dispatch`](../crates/provider-openai-responses/tests/rfc_responses.rs#L326) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`adapter_reads_each_declared_media_type_from_disk_in_user_and_tool_messages`](../crates/provider-openai-responses/tests/rfc_responses.rs#L360) | [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`response_usage_preserves_inclusive_totals_and_available_details`](../crates/provider-openai-responses/tests/rfc_responses.rs#L424) | [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |

## `tests/test_distribution.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_reinstall_keeps_shell_startup_files_and_private_tools_out_of_the_user_bin`](../tests/test_distribution.py#L90) | [§2](../docs/v1.0.0-RFC.md?plain=1#L23) · [§10.3](../docs/v1.0.0-RFC.md?plain=1#L531) |
| [`test_installed_pipeline_and_fold`](../tests/test_distribution.py#L117) | [§1.2](../docs/v1.0.0-RFC.md?plain=1#L15) · [§2](../docs/v1.0.0-RFC.md?plain=1#L23) · [§3.1](../docs/v1.0.0-RFC.md?plain=1#L39) · [§10.3](../docs/v1.0.0-RFC.md?plain=1#L531) · [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§4.4](../docs/v1.0.0-RFC.md?plain=1#L172) · [§9.6](../docs/v1.0.0-RFC.md?plain=1#L488) |

## `tests/test_linux_fallback.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_archived_wal_growth_does_not_require_reading_the_archive_on_resume`](../tests/test_linux_fallback.py#L36) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`test_unavailable_landlock_warns_or_fails_before_inference`](../tests/test_linux_fallback.py#L84) | [§10.2](../docs/v1.0.0-RFC.md?plain=1#L524) |
| [`test_doctor_detects_broken_pipes_and_ineffective_or_unavailable_flock`](../tests/test_linux_fallback.py#L97) | [§10.3](../docs/v1.0.0-RFC.md?plain=1#L531) |
| [`test_inbox_polls_new_publications_when_kernel_notifications_are_unavailable`](../tests/test_linux_fallback.py#L124) | [§4.2](../docs/v1.0.0-RFC.md?plain=1#L142) |

## `tests/test_pdf_pages.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_single_file_and_stdin_render_the_selected_pages`](../tests/test_pdf_pages.py#L51) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) |

## `tests/test_provider_streaming.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L88) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L121) | [§4.3](../docs/v1.0.0-RFC.md?plain=1#L156) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) |
| [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L180) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§9.2](../docs/v1.0.0-RFC.md?plain=1#L427) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |

## `tests/test_shell_pipeline.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_aclr_and_agent_agree_on_the_serialized_fold_frame_size_limit`](../tests/test_shell_pipeline.py#L77) | [§7.2](../docs/v1.0.0-RFC.md?plain=1#L305) · [§6.1](../docs/v1.0.0-RFC.md?plain=1#L204) · [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) |
| [`test_trace_envelope_identifies_the_live_process_and_effective_agent_name`](../tests/test_shell_pipeline.py#L127) | [§3.3](../docs/v1.0.0-RFC.md?plain=1#L85) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`test_wal_append_failure_preserves_the_prefix_and_reopens_according_to_tail_state`](../tests/test_shell_pipeline.py#L163) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`test_skill_index_round_trips_xml_metadata_and_msh_reads_the_indexed_file_on_demand`](../tests/test_shell_pipeline.py#L206) | [§9.4](../docs/v1.0.0-RFC.md?plain=1#L461) · [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) |
| [`test_open_input_descriptor_defers_inference_until_eof`](../tests/test_shell_pipeline.py#L249) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`test_inference_can_finish_with_an_open_trace_consumer_that_stops_reading`](../tests/test_shell_pipeline.py#L295) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`test_msh_timeout_signals_the_whole_group_and_grants_three_seconds_before_kill`](../tests/test_shell_pipeline.py#L331) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`test_interrupting_a_live_msh_discards_its_partial_result_and_replays_a_closed_history`](../tests/test_shell_pipeline.py#L369) | [§6.2](../docs/v1.0.0-RFC.md?plain=1#L226) · [§6.3](../docs/v1.0.0-RFC.md?plain=1#L268) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) · [§9.1](../docs/v1.0.0-RFC.md?plain=1#L388) |
| [`test_real_msh_side_channels_are_write_end_pipes_and_relay_beyond_pipe_capacity`](../tests/test_shell_pipeline.py#L422) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§4.3](../docs/v1.0.0-RFC.md?plain=1#L156) |
| [`test_stdout_and_stderr_terminal_modes_are_independent`](../tests/test_shell_pipeline.py#L462) | [§4.1](../docs/v1.0.0-RFC.md?plain=1#L121) · [§4.3](../docs/v1.0.0-RFC.md?plain=1#L156) |
| [`test_real_shell_timeout_returns_an_error_tool_result_and_inference_continues`](../tests/test_shell_pipeline.py#L527) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) · [§6.4](../docs/v1.0.0-RFC.md?plain=1#L278) |
| [`test_wire_file_has_raw_ansi_and_model_receives_cleaned_text`](../tests/test_shell_pipeline.py#L534) | [§7.1](../docs/v1.0.0-RFC.md?plain=1#L292) · [§9.5](../docs/v1.0.0-RFC.md?plain=1#L476) |
| [`test_path_precedes_libexec_and_explicit_shell_path_has_priority_over_both`](../tests/test_shell_pipeline.py#L554) | [§2](../docs/v1.0.0-RFC.md?plain=1#L23) |
| [`test_nested_agent_uses_own_model_wal_and_budget_and_inherits_cli_auto`](../tests/test_shell_pipeline.py#L619) | [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§5.1](../docs/v1.0.0-RFC.md?plain=1#L183) |
| [`test_nested_agent_inherits_cli_deny_over_its_own_auto_frontmatter`](../tests/test_shell_pipeline.py#L623) | [§3.4](../docs/v1.0.0-RFC.md?plain=1#L102) · [§5.1](../docs/v1.0.0-RFC.md?plain=1#L183) |
