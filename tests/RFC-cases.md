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

## `crates/anix-core/tests/rfc_runtime/fd.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment`](../crates/anix-core/tests/rfc_runtime/fd.rs#L61) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input`](../crates/anix-core/tests/rfc_runtime/fd.rs#L94) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider`](../crates/anix-core/tests/rfc_runtime/fd.rs#L136) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§3.4](../docs/v1.1.0-RFC.md?plain=1#L129) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) |

## `crates/anix-core/tests/rfc_runtime/wal.rs`

| 测试 | RFC 条款 |
| --- | --- |
| [`legacy_wal_without_vision_or_fold_usage_resumes_without_rewriting_the_prefix`](../crates/anix-core/tests/rfc_runtime/wal.rs#L32) | [§1.4](../docs/v1.1.0-RFC.md?plain=1#L33) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`repeated_resume_appends_one_vision_update_per_change_and_reuses_frozen_text`](../crates/anix-core/tests/rfc_runtime/wal.rs#L68) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§5.5](../docs/v1.1.0-RFC.md?plain=1#L279) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) |

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
