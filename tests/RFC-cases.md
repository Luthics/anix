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

## `tests/test_provider_streaming.py`

| 测试 | RFC 条款 |
| --- | --- |
| [`test_long_msh_can_outlive_the_provider_timeout_between_two_requests`](../tests/test_provider_streaming.py#L179) | [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_paced_msh_arguments_finish_before_the_shell_runs`](../tests/test_provider_streaming.py#L211) | [§7.1](../docs/v1.1.0-RFC.md?plain=1#L415) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_each_delta_reaches_trace_before_the_upstream_sends_its_next_frame`](../tests/test_provider_streaming.py#L237) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_interleaved_calls_and_opaque_state_round_trip_through_the_real_tool_results`](../tests/test_provider_streaming.py#L270) | [§4.3](../docs/v1.1.0-RFC.md?plain=1#L194) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.1](../docs/v1.1.0-RFC.md?plain=1#L530) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) |
| [`test_http_429_and_503_are_traced_by_the_core_before_replaying_the_request`](../tests/test_provider_streaming.py#L329) | [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_keyboard_interrupt_returns_to_the_same_shell_while_waiting_for_provider`](../tests/test_provider_streaming.py#L363) | [§3.1](../docs/v1.1.0-RFC.md?plain=1#L62) · [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_keyboard_interrupt_reaches_a_provider_that_closed_stdout_before_exit`](../tests/test_provider_streaming.py#L420) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§6.3](../docs/v1.1.0-RFC.md?plain=1#L380) · [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
| [`test_inbox_commits_one_message_then_interrupts_the_next_in_the_same_coprocess`](../tests/test_provider_streaming.py#L461) | [§4.1](../docs/v1.1.0-RFC.md?plain=1#L153) · [§4.2](../docs/v1.1.0-RFC.md?plain=1#L179) · [§6.2](../docs/v1.1.0-RFC.md?plain=1#L342) · [§6.4](../docs/v1.1.0-RFC.md?plain=1#L388) · [§6.5](../docs/v1.1.0-RFC.md?plain=1#L398) · [§9.2](../docs/v1.1.0-RFC.md?plain=1#L568) · [§9.5](../docs/v1.1.0-RFC.md?plain=1#L634) |
