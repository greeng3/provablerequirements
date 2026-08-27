Traceability report

REQ001 — REQ001
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/lib.rs:51 → health_json
    src/lib.rs:61 → health_json_reports_ok_and_current_version

REQ002 — REQ002
  formalized: —   implemented: —   verified: —
  verdict: never verified

REQ003 — REQ003
  formalized: —   implemented: —   verified: —
  verdict: never verified

REQ004 — REQ004
  formalized: —   implemented: —   verified: —
  verdict: never verified

REQ005 — REQ005
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/server.rs:6
    src/server.rs:62 → serve
    src/server.rs:75 → health
    src/server.rs:420 → health_route_returns_ok_json

REQ006 — REQ006
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/server.rs:7
    src/server.rs:373 → static_asset
    src/server.rs:431 → root_serves_embedded_index_html
    src/server.rs:439 → unknown_path_falls_back_to_index

REQ007 — REQ007
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/doorstop.rs:6
    src/doorstop.rs:42 → discover
    src/doorstop.rs:190 → discover_reads_prefix_and_items

REQ008 — REQ008
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/adopt.rs:4
    src/adopt.rs:25 → companion_name
    src/adopt.rs:115 → scaffold
    src/adopt.rs:408 → scaffold_creates_root_and_manifest

REQ009 — REQ009
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/doorstop.rs:102 → DoorstopSource
    src/reqforge.rs:15
    src/source.rs:8
    src/adopt.rs:314 → a_reqforge_subject_resolves_through_the_same_seam_as_a_doorstop_one
    src/adopt.rs:334 → a_subject_without_a_collection_is_still_read_as_doorstop
    src/doorstop.rs:207 → doorstop_source_reads_prose_and_revision
    src/reqforge.rs:294 → reads_a_reqforge_artifact_as_a_source_item
    src/reqforge.rs:319 → the_revision_follows_the_prose_and_not_the_timestamp
    src/reqforge.rs:388 → an_inactive_artifact_is_not_a_requirement
    src/source.rs:124 → the_revision_token_is_a_pinned_digest_not_a_build_local_hash

REQ010 — REQ010
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/triage.rs:7
    src/triage.rs:471 → prose_floor_defaults_to_prose_and_honors_hint
    src/triage.rs:631 → reclassify_never_replaces_an_operators_choice
    src/triage.rs:660 → a_fully_operator_set_backlog_reclassifies_nothing
    src/triage.rs:910 → seed_is_additive_and_set_overrides
    src/triage.rs:936 → state_persists_and_reloads

REQ011 — REQ011
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/status.rs:6
    src/status.rs:247 → funnel_keeps_states_distinct

REQ012 — REQ012
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/llm.rs:10
    src/llm.rs:591 → classify_maps_buckets_by_id
    src/llm.rs:705 → classify_tolerates_code_fenced_json
    src/llm.rs:720 → extracts_provider_response_shapes

REQ013 — REQ013
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/draft.rs:12
    src/draft.rs:368 → open_is_additive_and_preserves_candidate

REQ014 — REQ014
  formalized: yes   implemented: yes   verified: yes
  verdict: not-determined [mechanical] [stale — re-verify]
    src/draft.rs:12
    src/draft.rs:386 → stale_when_source_revision_moves

REQ015 — REQ015
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/formalize.rs:16
    src/formalize.rs:209 → translate_returns_candidate
    src/formalize.rs:222 → translate_strips_code_fence
    src/formalize.rs:235 → translate_rejects_empty_candidate

REQ016 — REQ016
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/prl/ast.rs:10 → Requirement
    src/prl/check.rs:11
    src/prl/error.rs:7
    src/prl/lexer.rs:6 → Token
    src/prl/parser.rs:11
    src/prl.rs:13 → ast
    src/prl.rs:131 → gate_rejects_undeclared_predicate_with_a_line
    src/prl.rs:148 → gate_rejects_malformed_input_at_parse

REQ017 — REQ017
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/formalize.rs:16
    src/prl/error.rs:7
    src/prl/vacuity.rs:13
    src/prl.rs:13 → ast
    src/draft.rs:447 → set_gate_updates_outcome_only
    src/formalize.rs:290 → repair_loop_recovers_on_second_attempt
    src/formalize.rs:307 → repair_loop_gives_up_after_max_attempts
    src/formalize.rs:324 → vacuity_warnings_do_not_drive_repair
    src/prl.rs:116 → gate_accepts_but_warns_on_vacuity

REQ018 — REQ018
  formalized: —   implemented: yes   verified: —
  verdict: never verified
    src/prl/readback.rs:13

REQ019 — REQ019
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/draft.rs:484 → admit_records_review_and_provenance
    src/draft.rs:508 → editing_candidate_revokes_admission

REQ020 — REQ020
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/reqforge.rs:16
    src/doorstop.rs:232 → annotate_stamps_and_replaces_provreq_block
    src/reqforge.rs:437 → annotate_appends_a_provreq_review_log_entry
    src/reqforge.rs:467 → annotate_appends_and_does_not_replace
    src/reqforge.rs:574 → annotate_rejects_unknown_item

REQ021 — REQ021
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/grounding.rs:17
    src/trace/mod.rs:253
    src/draft.rs:541 → set_binding_attaches_and_overwrites_by_symbol
    src/draft.rs:567 → editing_candidate_clears_bindings
    src/grounding.rs:638 → bindable_symbols_are_declared_predicates
    src/grounding.rs:646 → category_and_fidelity_default_from_the_requirement
    src/grounding.rs:655 → is_bindable_rejects_undeclared_symbols
    src/grounding.rs:800 → a_binding_the_caller_did_not_resolve_parks_in_every_category
    src/trace/mod.rs:163 → bindable_symbols_are_declared_predicates
    src/trace/tags.rs:186 → split_ids

REQ022 — REQ022
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/engine.rs:28
    src/engine.rs:646 → absent_binary_detects_as_missing
    src/engine.rs:659 → present_binary_detects_as_available
    src/engine.rs:862 → version_parsing_and_comparison
    src/engine.rs:882 → readiness_needs_every_category_engine
    src/engine.rs:938 → uncategorized_requirement_is_blocked

REQ023 — REQ023
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/verdict.rs:14
    src/verdict.rs:729 → grounded_but_no_engine_is_unknown_no_engine
    src/verdict.rs:739 → parked_grounding_is_unknown_missing_grounding_with_reasons
    src/verdict.rs:753 → render_shows_status_reason_and_provenance
    src/verdict.rs:768 → render_handles_missing_subject_commit

REQ024 — REQ024
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/engine.rs:29
    src/prl/fragment.rs:32
    src/engine.rs:627 → an_unwired_engine_is_never_ready
    src/engine.rs:636 → not_wired_is_distinct_from_missing
    src/engine.rs:908 → unwired_engine_blocks_readiness
    src/prl/fragment.rs:245 → liveness_at_category_one_is_out_of_fragment
    src/prl/fragment.rs:264 → invariants_are_in_fragment_at_category_one
    src/prl/fragment.rs:286 → every_temporal_pattern_is_out_of_fragment_at_category_one
    src/prl/fragment.rs:310 → can_reach_is_only_expressible_at_the_model_category
    src/prl/fragment.rs:340 → every_declared_category_must_express_the_claim
    src/prl/fragment.rs:363 → category_less_candidate_is_not_fragment_checked
    src/prl/fragment.rs:375 → out_of_fragment_error_is_actionable
    src/prl.rs:95 → gate_rejects_liveness_at_category_one

REQ025 — REQ025
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/grounding.rs:17
    src/rust_adapter.rs:21
    src/trace/mod.rs:253
    src/grounding.rs:755 → verdict_parks_when_a_binding_does_not_resolve
    src/grounding.rs:780 → verdict_parks_when_a_binding_was_never_resolved
    src/grounding.rs:1090 → predicate_arity_comes_from_the_vocabulary
    src/rust_adapter.rs:2186 → resolves_a_bool_function_to_its_source_location
    src/rust_adapter.rs:2593 → missing_function_does_not_resolve
    src/rust_adapter.rs:2602 → arity_mismatch_does_not_resolve
    src/rust_adapter.rs:2619 → non_boolean_function_does_not_resolve
    src/rust_adapter.rs:2631 → result_bool_is_not_mistaken_for_bool
    src/rust_adapter.rs:2643 → duplicate_names_are_ambiguous_never_guessed
    src/rust_adapter.rs:2659 → finds_functions_in_modules_and_impls
    src/rust_adapter.rs:2672 → unparseable_file_does_not_blind_resolution
    src/rust_adapter.rs:2685 → skips_the_companion_tree_and_git
    src/rust_adapter.rs:2716 → empty_observable_resolves_to_nothing
    src/rust_adapter.rs:2724 → non_rust_files_are_not_searched
    src/rust_adapter.rs:3165 → resolved_readback_states_the_syntactic_limit

REQ026 — REQ026
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/grounding.rs:863 → bindable_sorts_are_quantifier_sorts_and_declared_sorts
    src/grounding.rs:924 → unbound_sort_parks_even_when_every_predicate_resolves
    src/grounding.rs:950 → unresolved_sort_parks
    src/rust_adapter.rs:2818 → resolves_a_sort_to_a_struct_enum_or_alias
    src/rust_adapter.rs:2886 → unknown_sort_does_not_resolve
    src/rust_adapter.rs:2899 → duplicate_sorts_are_ambiguous_never_guessed
    src/rust_adapter.rs:3134 → predicates_and_sorts_do_not_cross_resolve

REQ027 — REQ027
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/engine.rs:29
    src/kani.rs:28
    src/verdict.rs:14
    src/kani.rs:534 → quantified_invariant_lowers_to_a_proof_harness
    src/kani.rs:555 → harness_is_inert_under_the_subjects_own_cargo_test
    src/kani.rs:563 → calls_follow_the_subjects_parameter_modes
    src/kani.rs:587 → never_lowers_to_the_negated_invariant
    src/kani.rs:611 → unbound_sort_does_not_lower
    src/kani.rs:632 → unresolved_predicate_does_not_lower
    src/kani.rs:670 → temporal_patterns_do_not_lower
    src/kani.rs:685 → successful_verification_is_holds
    src/kani.rs:694 → failed_verification_is_fails_with_a_witness
    src/kani.rs:721 → failed_verification_without_playback_still_fails
    src/kani.rs:734 → unrecognised_output_is_inconclusive_and_names_the_actionable_cause
    src/kani.rs:770 → empty_output_is_inconclusive_with_a_readable_reason
    src/kani.rs:820 → harness_name_is_a_valid_prefixed_identifier
    src/kani.rs:874 → real_kani_verifies_a_true_invariant
    src/kani.rs:913 → real_kani_refutes_a_false_invariant_with_a_concrete_witness
    src/kani.rs:932 → real_kani_cannot_decide_when_the_sort_is_not_instantiable
    src/kani.rs:975 → real_kani_run_leaves_no_trace_in_the_subject
    src/kani.rs:989 → an_existing_file_is_never_overwritten
    src/kani.rs:1013 → a_kani_pass_is_bounded_model_checked_never_proven
    src/kani.rs:1150 → a_kani_refutation_becomes_a_fails_carrying_its_witness
    src/kani.rs:1172 → an_undecided_run_is_unknown_inconclusive_never_a_verdict
    src/rust_adapter.rs:2438 → resolved_predicate_reports_how_its_parameters_take_arguments

REQ028 — REQ028
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/spec_paths.rs:32
    src/tla_adapter.rs:38
    src/grounding.rs:1116 → model_requirement_grounds_when_every_binding_resolves
    src/grounding.rs:1421 → model_requirement_parks_when_a_binding_does_not_resolve
    src/grounding.rs:1448 → model_requirement_parks_when_a_binding_has_the_wrong_arity
    src/grounding.rs:1493 → the_arity_a_model_binding_must_match_comes_from_the_vocabulary
    src/grounding.rs:1507 → model_requirement_parks_when_a_binding_was_never_resolved
    src/spec_paths.rs:119 → a_configured_root_resolves_against_the_subject
    src/spec_paths.rs:134 → a_sibling_root_resolves_to_a_clean_absolute_path
    src/spec_paths.rs:155 → an_unconfigured_subject_has_no_extra_roots
    src/spec_paths.rs:164 → an_unparseable_manifest_means_no_extra_roots
    src/spec_paths.rs:171 → a_missing_root_keeps_the_configured_path
    src/spec_paths.rs:182 → a_blank_entry_is_not_a_root
    src/tla_adapter.rs:597 → a_spec_outside_the_subject_resolves
    src/tla_adapter.rs:613 → a_root_inside_the_subject_does_not_duplicate_its_specs
    src/tla_adapter.rs:630 → the_fingerprint_covers_external_specs_and_moves_with_them
    src/tla_adapter.rs:649 → an_in_tree_subject_has_no_external_fingerprint
    src/tla_adapter.rs:658 → the_fingerprint_is_independent_of_root_order
    src/tla_adapter.rs:677 → resolves_an_operator_definition_to_its_location
    src/tla_adapter.rs:690 → a_variable_a_constant_and_a_set_all_resolve
    src/tla_adapter.rs:754 → an_undefined_name_does_not_resolve
    src/tla_adapter.rs:765 → a_name_only_in_a_comment_does_not_resolve
    src/tla_adapter.rs:772 → a_keyword_prefix_is_not_a_declaration
    src/tla_adapter.rs:784 → an_equality_expression_is_not_a_definition
    src/tla_adapter.rs:794 → a_function_definition_takes_no_operator_arguments
    src/tla_adapter.rs:812 → a_predicate_bound_to_a_variable_that_takes_no_arguments_is_refused
    src/tla_adapter.rs:837 → an_operator_applied_to_too_few_arguments_is_refused
    src/tla_adapter.rs:853 → a_multi_argument_operator_resolves_only_at_its_own_arity
    src/tla_adapter.rs:865 → a_higher_order_parameter_counts_as_one_argument
    src/tla_adapter.rs:874 → ambiguity_is_reported_before_arity
    src/tla_adapter.rs:887 → the_read_back_claims_only_what_was_checked
    src/tla_adapter.rs:901 → an_unreadable_parameter_list_claims_no_arity
    src/tla_adapter.rs:913 → duplicate_definitions_are_ambiguous_never_guessed
    src/tla_adapter.rs:925 → the_walk_skips_the_companion_and_git
    src/tla_adapter.rs:950 → non_tla_files_are_not_searched
    src/tla_adapter.rs:959 → empty_observable_resolves_to_nothing
    src/verdict_store.rs:450 → an_external_spec_moving_makes_a_verdict_stale
    src/verdict_store.rs:477 → a_verdict_without_a_spec_fingerprint_is_not_flagged
    src/verdict_store.rs:496 → losing_the_configured_specs_makes_a_verdict_stale

REQ029 — REQ029
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/tlc.rs:41
    src/engine.rs:675 → present_host_without_the_engine_marker_is_missing
    src/tla_adapter.rs:729 → declared_constants_reads_whole_lines_and_ignores_comments
    src/tlc.rs:1102 → quantified_leads_to_lowers_to_a_temporal_property
    src/tlc.rs:1122 → cfg_names_the_subject_spec_and_the_property
    src/tlc.rs:1146 → a_configured_constant_is_assigned_in_the_cfg
    src/tlc.rs:1165 → an_unconfigured_subject_gets_the_cfg_it_always_had
    src/tlc.rs:1173 → a_bounded_holds_reports_the_model_it_was_checked_under
    src/tlc.rs:1192 → an_assignment_the_model_does_not_declare_is_refused_by_name
    src/tlc.rs:1220 → a_model_value_needs_no_declaration_in_the_spec
    src/tlc.rs:1239 → every_outcome_reports_the_model_it_was_produced_under
    src/tlc.rs:1287 → constants_are_read_from_the_manifest
    src/tlc.rs:1301 → a_subject_that_configured_nothing_has_no_constants
    src/tlc.rs:1324 → a_constant_provreq_cannot_write_is_refused_by_name
    src/tlc.rs:1335 → safety_and_eventually_patterns_lower_to_tla_operators
    src/tlc.rs:1375 → unbound_sort_does_not_lower
    src/tlc.rs:1394 → unbound_predicate_does_not_lower
    src/tlc.rs:1412 → metric_leads_to_does_not_lower
    src/tlc.rs:1431 → out_of_core_pattern_does_not_lower
    src/tlc.rs:1450 → successful_check_is_holds
    src/tlc.rs:1459 → temporal_violation_is_fails_with_a_witness
    src/tlc.rs:1490 → invariant_violation_is_fails
    src/tlc.rs:1510 → unassigned_constant_is_inconclusive_and_names_the_cause
    src/tlc.rs:1525 → an_error_line_that_announces_its_cause_carries_it
    src/tlc.rs:1561 → a_location_in_the_generated_module_becomes_what_provreq_generated
    src/tlc.rs:1596 → an_unquotable_generated_location_is_dropped_rather_than_kept
    src/tlc.rs:1619 → a_sany_semantic_banner_reports_the_cause_not_the_count
    src/tlc.rs:1651 → a_sany_parse_banner_reports_the_cause_not_the_count
    src/tlc.rs:1674 → a_banner_with_no_cause_after_it_is_still_the_reason
    src/tlc.rs:1684 → empty_output_is_inconclusive
    src/tlc.rs:1694 → module_name_is_a_valid_prefixed_identifier
    src/tlc.rs:1702 → module_header_is_read_from_the_spec
    src/tlc.rs:1717 → the_module_search_path_is_resolved_whatever_the_operator_typed
    src/tlc.rs:1754 → a_tlc_pass_is_bounded_model_checked_never_proven
    src/tlc.rs:1770 → a_tlc_violation_becomes_a_fails_carrying_its_witness
    src/tlc.rs:1791 → an_undecided_run_is_unknown_inconclusive_never_a_verdict
    src/tlc.rs:1851 → real_tlc_verifies_a_true_leads_to
    src/tlc.rs:1873 → real_tlc_reports_the_cause_of_a_wrong_arity_binding
    src/tlc.rs:1948 → real_tlc_checks_a_parameterised_spec_under_the_operators_model
    src/tlc.rs:2017 → real_tlc_ignores_an_assignment_the_spec_does_not_declare
    src/tlc.rs:2046 → real_tlc_faults_in_the_generated_module_read_as_what_provreq_generated
    src/tlc.rs:2098 → real_tlc_refutes_an_unfair_leads_to_with_a_witness
    src/tlc.rs:2121 → real_tlc_run_leaves_no_trace_in_the_subject
    src/tlc.rs:2148 → a_subject_without_spec_is_an_honest_error
    src/tlc.rs:2167 → a_run_writes_nothing_into_the_spec_directory

REQ030 — REQ030
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/engine.rs:320 → engines_for
    src/verdict.rs:352 → aggregate
    src/engine.rs:918 → category_is_ready_when_any_ensemble_engine_is_ready
    src/verdict.rs:802 → single_holds_aggregates_to_that_holds
    src/verdict.rs:817 → agreeing_holds_corroborate
    src/verdict.rs:835 → holds_versus_fails_is_divergence
    src/verdict.rs:852 → a_fails_refutes_and_keeps_the_witness
    src/verdict.rs:864 → inconclusive_does_not_block_a_holds
    src/verdict.rs:1025 → all_inconclusive_is_unknown_inconclusive

REQ031 — REQ031
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/creusot.rs:34
    src/creusot.rs:770 → quantified_invariant_lowers_to_a_forall_proof_assert
    src/creusot.rs:792 → calls_go_through_crate_not_a_crate_name
    src/creusot.rs:801 → calls_follow_the_subjects_parameter_modes
    src/creusot.rs:814 → never_lowers_to_a_negated_unquantified_assertion
    src/creusot.rs:844 → unbound_sort_does_not_lower
    src/creusot.rs:863 → unresolved_predicate_does_not_lower
    src/creusot.rs:878 → temporal_patterns_do_not_lower
    src/creusot.rs:893 → proved_output_is_holds
    src/creusot.rs:902 → unproved_goal_is_inconclusive_never_fails
    src/creusot.rs:917 → a_partial_proof_is_not_holds
    src/creusot.rs:925 → a_compile_failure_is_inconclusive_and_names_the_error_and_its_site
    src/creusot.rs:1190 → unrecognised_output_is_inconclusive
    src/creusot.rs:1200 → harness_name_is_a_valid_prefixed_identifier
    src/creusot.rs:1215 → a_creusot_pass_is_proven_and_not_bounded
    src/creusot.rs:1227 → an_inconclusive_run_is_unknown_never_a_verdict
    src/creusot.rs:1394 → real_creusot_proves_a_true_invariant
    src/creusot.rs:1409 → real_creusot_cannot_prove_a_false_invariant
    src/creusot.rs:1425 → real_creusot_is_inconclusive_on_opaque_predicates
    src/creusot.rs:1703 → real_creusot_run_leaves_no_trace_in_the_subject
    src/creusot.rs:1735 → an_existing_harness_file_is_never_overwritten
    src/creusot.rs:1760 → a_subject_with_no_crate_root_is_inconclusive
    src/verdict.rs:880 → a_proven_holds_does_not_wear_the_bounded_caveat
    src/verdict.rs:900 → proven_outranks_bounded_model_checked_in_the_ensemble

REQ032 — REQ032
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/prusti.rs:34
    src/prusti.rs:437 → quantified_invariant_lowers_to_a_forall_prusti_assert
    src/prusti.rs:459 → calls_go_through_crate_not_a_crate_name
    src/prusti.rs:468 → calls_follow_the_subjects_parameter_modes
    src/prusti.rs:481 → never_lowers_to_a_negated_unquantified_assertion
    src/prusti.rs:511 → unbound_sort_does_not_lower
    src/prusti.rs:530 → unresolved_predicate_does_not_lower
    src/prusti.rs:545 → temporal_patterns_do_not_lower
    src/prusti.rs:560 → finished_output_is_holds
    src/prusti.rs:570 → verification_error_is_inconclusive_never_fails
    src/prusti.rs:586 → a_compile_failure_is_inconclusive_and_names_the_cause
    src/prusti.rs:602 → a_missing_prusti_contracts_dependency_is_inconclusive
    src/prusti.rs:637 → unrecognised_output_is_inconclusive
    src/prusti.rs:647 → harness_name_is_a_valid_prefixed_identifier
    src/prusti.rs:662 → a_prusti_pass_is_proven_and_not_bounded
    src/prusti.rs:674 → an_inconclusive_run_is_unknown_never_a_verdict
    src/prusti.rs:746 → real_prusti_proves_a_true_invariant
    src/prusti.rs:761 → real_prusti_cannot_prove_a_false_invariant
    src/prusti.rs:777 → real_prusti_is_inconclusive_on_opaque_predicates
    src/prusti.rs:814 → real_prusti_run_leaves_no_trace_in_the_subject
    src/prusti.rs:834 → an_existing_harness_file_is_never_overwritten
    src/prusti.rs:859 → a_subject_with_no_crate_root_is_inconclusive

REQ033 — REQ033
  formalized: —   implemented: yes   verified: —
  verdict: never verified
    src/contract_draft.rs:21

REQ034 — REQ034
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/server.rs:8
    src/server.rs:142 → requirements
    src/server.rs:447 → requirements_on_unadopted_subject_is_conflict
    src/server.rs:459 → requirements_lists_items_with_coverage
    src/status.rs:492 → backlog_pairs_each_item_with_its_triage_and_formalization

REQ035 — REQ035
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/detail.rs:12
    src/server.rs:250 → requirement_detail
    src/detail.rs:191 → item_without_a_draft_has_no_formalization
    src/detail.rs:204 → admitted_draft_surfaces_candidate_readback_and_review
    src/server.rs:502 → detail_for_unknown_id_is_not_found
    src/server.rs:511 → detail_for_known_id_returns_the_item

REQ036 — REQ036
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/detail.rs:91 → grounding_report
    src/detail.rs:265 → grounding_report_reports_per_binding_and_parks_on_any_unresolved

REQ037 — REQ037
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/server.rs:185 → set_triage
    src/server.rs:540 → triage_write_sets_the_bucket_and_returns_updated_coverage
    src/server.rs:565 → triage_write_rejects_an_unknown_bucket

REQ038 — REQ038
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/server.rs:332 → verify_requirement
    src/verify.rs:11
    src/server.rs:591 → verify_unknown_id_is_not_found
    src/server.rs:604 → verify_unadopted_subject_is_conflict
    src/server.rs:642 → verify_undrafted_item_is_honest_no_draft_state
    src/verdict.rs:779 → report_carries_labels_provenance_and_per_engine_breakdown
    src/verify.rs:760 → unknown_id_is_none
    src/verify.rs:769 → undrafted_item_is_no_draft
    src/verify.rs:778 → unadopted_subject_is_error

REQ039 — REQ039
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/verdict_store.rs:15
    src/server.rs:616 → stored_verdict_surfaces_on_the_row_and_drift_drops_it_from_verified
    src/status.rs:309 → verified_counts_only_fresh_holds_and_row_surfaces_drift
    src/verdict_store.rs:746 → unmoved_verdict_is_fresh
    src/verdict_store.rs:764 → each_moved_axis_is_a_named_reason
    src/verdict_store.rs:851 → record_and_load_round_trip_replacing_prior
    src/verdict_store.rs:877 → missing_file_is_empty_store

REQ040 — REQ040
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/semantic_draft.rs:18
    src/rust_adapter.rs:2567 → fn_source_at_extracts_the_whole_function
    src/semantic_draft.rs:405 → parse_clauses_keeps_only_attribute_lines
    src/semantic_draft.rs:422 → prompt_carries_dialect_intent_claim_and_source
    src/semantic_draft.rs:498 → drafts_per_function_dedups_and_skips_declined
    src/semantic_draft.rs:532 → applies_ordered_block_with_indentation

REQ041 — REQ041
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/main.rs:2282 → proof_step_is_proved_when_the_drafting_engine_holds
    src/main.rs:2318 → proof_step_carries_reasons_when_inconclusive
    src/semantic_draft.rs:597 → repair_stops_when_first_draft_proves
    src/semantic_draft.rs:615 → repair_feeds_reason_back_and_recovers
    src/semantic_draft.rs:650 → repair_is_bounded
    src/semantic_draft.rs:672 → repair_gives_up_when_nothing_drafted

REQ042 — REQ042
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/llm.rs:890 → load_config_honors_explicit_timeout

REQ043 — REQ043
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/status.rs:408 → stale_counts_drifted_verdicts_of_any_polarity

REQ044 — REQ044
  formalized: —   implemented: —   verified: —
  verdict: never verified

REQ045 — REQ045
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/status.rs:347 → editing_the_admitted_candidate_drifts_its_verdict
    src/verdict_store.rs:808 → formalization_change_drifts_the_verdict
    src/verdict_store.rs:828 → no_admitted_formalization_drifts_the_verdict
    src/verdict_store.rs:839 → pre_axis_verdict_is_not_flagged_on_formalization

REQ046 — REQ046
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/provision.rs:13
    src/provision.rs:421 → jar_lives_under_the_data_dir
    src/provision.rs:429 → only_failure_is_a_failure

REQ047 — REQ047
  formalized: yes   implemented: yes   verified: yes
  verdict: proven (model-checked (bounded)) [mechanical] [stale — re-verify]
    src/provision.rs:14
    src/provision.rs:395 → kani_plan_names_the_commands_it_would_run
    src/provision.rs:408 → kani_platform_gate_matches_upstream_reach

REQ048 — REQ048
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/buildenv.rs:328 → no_config_is_native
    src/buildenv.rs:337 → image_config_with_comments_and_trailing_comma_resolves
    src/buildenv.rs:359 → dockerfile_config_resolves_under_either_spelling
    src/buildenv.rs:379 → unreadable_config_is_not_reported_as_native
    src/buildenv.rs:395 → nested_config_folder_resolves_deterministically
    src/buildenv.rs:415 → comment_stripping_does_not_touch_string_contents
    src/buildenv.rs:429 → advice_separates_the_command_answer_from_the_build_env_answer
    src/buildenv.rs:450 → heavy_tier_advice_names_this_subjects_environment
    src/buildenv.rs:481 → advice_does_not_tell_you_to_extend_the_image_you_are_running
    src/buildenv.rs:504 → nothing_missing_yields_no_advice
    src/buildenv.rs:511 → this_repos_own_dev_container_resolves_to_its_image
    src/main.rs:2210 → install_says_which_kind_of_no_it_is
    src/main.rs:2261 → engine_names_resolve_from_their_cli_spelling

REQ049 — REQ049
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/proving_env.rs:193 → unversioned_engines_are_recorded_separately_not_as_a_version
    src/proving_env.rs:245 → an_engine_with_no_version_never_drifts_a_verdict
    src/proving_env.rs:262 → an_engine_version_change_is_named_drift
    src/proving_env.rs:272 → the_declared_label_drifts_independently_of_the_engines
    src/proving_env.rs:293 → the_container_marker_is_context_not_drift
    src/proving_env.rs:303 → describe_states_the_blind_spot_and_the_missing_label
    src/proving_env.rs:331 → the_declared_label_is_read_from_the_manifest_and_never_fails
    src/verdict_store.rs:427 → a_verdict_proved_elsewhere_is_stale
    src/verdict_store.rs:654 → a_verdict_predating_the_environment_axis_is_not_flagged

REQ050 — REQ050
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/verdict_store.rs:622 → a_recorded_environment_is_distinguishable_from_an_unrecorded_one
    web/src/App.test.tsx:168 → a stored verdict says where it was proved, or that it was never recorded

REQ051 — REQ051
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/server.rs:105 → engines
    src/creusot.rs:1240 → a_subject_whose_creusot_std_cannot_work_with_the_tool_is_unusable
    src/creusot.rs:1264 → a_usable_pair_and_an_uninvolved_subject_are_left_alone
    src/engine.rs:690 → a_binary_that_cannot_start_is_not_available
    src/engine.rs:714 → an_installed_creusot_is_not_ready_for_a_subject_it_would_refuse_to_start_on
    src/engine.rs:764 → unusable_is_distinct_from_missing
    src/engine.rs:776 → an_engine_that_ran_and_objected_to_its_input_is_still_present
    src/proving_env.rs:223 → an_engine_that_cannot_start_is_not_part_of_the_proving_environment
    src/server.rs:475 → engines_route_reports_every_engine_with_a_toneable_state

REQ052 — REQ052
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/llm.rs:616 → classify_leaves_missing_and_unknown_untriaged
    src/llm.rs:639 → a_reply_with_no_usable_assignment_is_an_error
    src/llm.rs:674 → one_usable_assignment_is_still_an_answer
    src/llm.rs:693 → empty_assistant_content_is_a_failed_response
    src/llm.rs:748 → the_prompt_asks_about_lowering_not_about_wording
    src/triage.rs:509 → a_declined_item_is_left_exactly_as_it_was
    src/triage.rs:783 → a_failed_classifier_leaves_the_existing_state_untouched

REQ053 — REQ053
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/triage.rs:687 → a_fully_triaged_backlog_plans_no_work
    src/triage.rs:719 → reclassify_puts_every_item_back_in_front_of_the_classifier

REQ054 — REQ054
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/triage.rs:737 → a_stopped_reclassify_still_covers_every_item
    src/triage.rs:830 → a_failed_batch_keeps_the_batches_that_landed_and_a_retry_resumes
    src/triage.rs:893 → a_zero_batch_size_still_classifies_the_backlog

REQ055 — REQ055
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/lowering.rs:641 → a_free_function_lowers_to_a_free_call
    src/lowering.rs:650 → a_method_lowers_to_a_method_call_not_a_free_call
    src/lowering.rs:724 → a_variant_test_lowers_to_a_match_expression
    src/lowering.rs:1201 → a_nullary_method_is_not_lowerable
    src/rust_adapter.rs:2234 → a_function_returning_an_enum_binds_through_one_of_its_variants
    src/rust_adapter.rs:2265 → a_variant_that_does_not_exist_names_the_ones_that_do
    src/rust_adapter.rs:2279 → a_variant_on_a_non_enum_return_says_so
    src/rust_adapter.rs:2290 → qualifying_by_type_disambiguates_a_shared_method_name
    src/rust_adapter.rs:2315 → a_method_found_by_its_bare_name_is_still_a_method
    src/rust_adapter.rs:2335 → a_method_the_type_does_not_have_names_the_ones_it_does
    src/rust_adapter.rs:2426 → a_path_deeper_than_two_segments_does_not_resolve

REQ056 — REQ056
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/main.rs:2161 → a_path_passed_where_an_id_belongs_is_recognised
    src/main.rs:2182 → the_path_hint_does_not_fire_on_other_mistakes

REQ057 — REQ057
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/grounding.rs:896 → declared_parameter_sorts_type_check_every_position
    src/grounding.rs:973 → expected_param_types_follow_the_quantified_arguments_sort
    src/grounding.rs:1001 → a_sort_that_did_not_resolve_says_nothing_about_the_parameter
    src/grounding.rs:1048 → a_position_the_requirement_cannot_speak_for_stays_unknown
    src/rust_adapter.rs:2463 → a_parameter_typed_against_a_different_sort_does_not_resolve
    src/rust_adapter.rs:2490 → positions_a_name_comparison_cannot_speak_for_are_skipped
    src/rust_adapter.rs:2513 → a_receivers_type_is_the_type_it_is_implemented_on
    src/rust_adapter.rs:2532 → arity_and_return_type_are_reported_before_a_parameter_type
    src/rust_adapter.rs:2551 → a_variant_test_checks_its_parameter_types_too
    src/rust_adapter.rs:3407 → the_parameter_check_discriminates_on_type_arguments
    src/rust_adapter.rs:3435 → a_side_that_writes_no_type_arguments_is_compared_on_the_name_alone

REQ058 — REQ058
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/lowering.rs:744 → a_primitive_sort_lowers_unprefixed
    src/rust_adapter.rs:2837 → a_primitive_type_resolves_as_a_sort
    src/rust_adapter.rs:2856 → str_and_string_are_not_primitive_sorts
    src/rust_adapter.rs:2872 → a_declared_type_wins_over_the_primitive_of_the_same_name

REQ059 — REQ059
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/grounding.rs:879 → a_declared_parameter_type_is_a_bindable_sort
    src/grounding.rs:896 → declared_parameter_sorts_type_check_every_position
    src/lowering.rs:873 → free_variables_are_closed_over_their_declared_sorts
    src/lowering.rs:980 → a_variable_without_a_declared_sort_does_not_lower
    src/lowering.rs:1005 → an_explicit_binder_wins_over_the_declared_parameter_sort
    src/lowering.rs:1028 → a_literal_argument_is_not_closed_over
    src/prl/ast.rs:325 → free_variables_of_an_invariant_are_bound_by_their_declared_sorts
    src/prl/ast.rs:346 → an_explicit_binder_leads_and_keeps_its_own_sort
    src/prl/ast.rs:363 → a_variable_the_requirement_does_not_type_has_no_sort
    src/prl/ast.rs:386 → literals_and_expressions_are_not_bound
    src/prl/ast.rs:398 → closure_applies_only_to_an_invariant_in_the_code_fragment
    src/prl/readback.rs:224 → readback_states_an_implicit_closure
    src/prl/readback.rs:244 → readback_names_a_variable_with_no_declared_sort

REQ060 — REQ060
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/reqforge.rs:361 → a_mac_sidecar_never_becomes_a_requirement
    src/rust_adapter.rs:2108 → a_hidden_source_file_is_still_the_subjects_own
    src/rust_adapter.rs:2128 → mac_resource_files_never_become_binding_candidates
    src/subject_tree.rs:90 → prunes_build_and_vcs_directories_by_name
    src/subject_tree.rs:108 → prunes_a_tagged_cache_directory_under_any_name
    src/subject_tree.rs:128 → an_unsigned_tag_does_not_prune
    src/subject_tree.rs:140 → prunes_hidden_directories_no_name_list_could_enumerate
    src/subject_tree.rs:156 → never_prunes_the_root_of_the_walk
    src/subject_tree.rs:171 → prunes_the_operating_systems_own_files
    src/subject_tree.rs:184 → reads_files_the_author_wrote_however_they_are_named
    src/tla_adapter.rs:541 → mac_resource_files_never_become_model_specs

REQ061 — REQ061
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/lowering.rs:1051 → a_call_is_named_through_the_module_it_was_found_in
    src/lowering.rs:1086 → a_variant_test_names_the_enum_through_its_own_module
    src/lowering.rs:1120 → an_item_with_no_module_path_does_not_lower
    src/rust_adapter.rs:2737 → module_path_follows_the_cargo_layout
    src/rust_adapter.rs:2763 → resolution_records_the_module_it_found_the_item_in
    src/rust_adapter.rs:2785 → an_item_outside_the_crate_resolves_but_has_no_module
    src/rust_adapter.rs:2804 → duplicate_enums_are_ambiguous_never_pooled

REQ062 — REQ062
  formalized: —   implemented: yes   verified: —
  verdict: never verified
    src/lowering.rs:28

REQ063 — REQ063
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/prusti.rs:615 → a_toolchain_ceiling_is_named_as_one_not_as_a_build_error

REQ064 — REQ064
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/creusot.rs:1027 → a_prover_crash_is_not_reported_as_the_subjects_fault
    src/creusot.rs:1053 → crash_reports_are_told_apart_from_the_operators_own
    src/creusot.rs:1141 → the_constants_likely_source_is_offered_not_asserted

REQ065 — REQ065
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/kani.rs:780 → an_uninstantiable_sort_is_explained_as_a_precondition_not_a_compiler_error
    src/kani.rs:806 → an_unrelated_unsatisfied_bound_is_not_read_as_an_instantiability_problem

REQ066 — REQ066
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/prl/check.rs:11
    src/prl/readback.rs:13
    src/lowering.rs:926 → a_boolean_variable_lowers_to_itself_as_a_condition
    src/lowering.rs:957 → a_non_boolean_variable_used_as_a_condition_does_not_lower
    src/prl/check.rs:110 → a_variable_used_as_a_condition_is_not_an_undeclared_predicate
    src/prl/check.rs:122 → a_bare_name_the_claim_does_not_bind_is_still_undeclared
    src/prl/check.rs:135 → a_declared_nullary_predicate_is_still_a_predicate
    src/prl/readback.rs:305 → a_variable_used_as_a_condition_reads_as_a_condition
    src/prl/readback.rs:320 → a_nullary_predicate_still_reads_as_a_predicate

REQ067 — REQ067
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/creusot.rs:1073 → an_untranslatable_construct_is_the_provers_limit_not_the_subjects_fault
    src/creusot.rs:1103 → an_untranslatable_constant_is_the_provers_limit_too

REQ068 — REQ068
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/creusot.rs:1614 → real_creusot_proves_a_claim_over_ordinary_functions_through_their_mirrors
    src/creusot.rs:1639 → real_creusot_will_not_prove_a_claim_through_a_mirror_that_lies

REQ069 — REQ069
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/lowering.rs:771 → an_applied_sort_lowers_with_a_path_for_every_part
    src/mirror_draft.rs:1476 → a_mirror_signature_writes_type_arguments_out
    src/rust_adapter.rs:3275 → a_sort_observable_may_apply_a_type_argument
    src/rust_adapter.rs:3292 → a_type_argument_resolves_against_the_subject_like_any_sort
    src/rust_adapter.rs:3317 → type_arguments_are_checked_against_the_declarations_arity
    src/rust_adapter.rs:3347 → a_nested_type_argument_is_refused_by_name
    src/rust_adapter.rs:3361 → an_argument_that_names_no_type_refuses_the_application
    src/rust_adapter.rs:3386 → a_missing_head_type_reads_as_missing_not_as_bad_arguments
    src/rust_adapter.rs:3397 → the_readback_of_an_application_names_every_part
    src/rust_adapter.rs:3407 → the_parameter_check_discriminates_on_type_arguments

REQ070 — REQ070
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/creusot.rs:1103 → an_untranslatable_constant_is_the_provers_limit_too
    src/creusot.rs:1164 → a_later_untranslatable_construct_does_not_preempt_the_first_error

REQ071 — REQ071
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/verify.rs:662 → subject_source_fingerprint
    src/verdict_store.rs:675 → a_companion_only_commit_is_not_code_drift
    src/verdict_store.rs:697 → source_movement_is_code_drift
    src/verdict_store.rs:721 → a_verdict_without_a_source_fingerprint_keeps_the_commit_rule
    src/verify.rs:785 → source_fingerprint_ignores_the_records_other_axes_own
    src/verify.rs:854 → source_fingerprint_ignores_reqforge_requirements_too
    src/verify.rs:918 → a_non_repo_has_no_source_fingerprint

REQ072 — REQ072
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/llm.rs:366 → build_prompt
    src/prl/fragment.rs:131 → triage_boundaries
    src/rust_adapter.rs:1896 → inventory
    src/llm.rs:526 → prompt_carries_observables_and_states_its_cap
    src/llm.rs:547 → an_empty_context_renders_no_observables_section
    src/llm.rs:555 → prompt_boundaries_come_from_the_gate
    src/prl/fragment.rs:209 → triage_boundaries_place_every_verb_once_per_category
    src/rust_adapter.rs:2165 → inventory_names_predicates_and_sorts

REQ073 — REQ073
  formalized: —   implemented: yes   verified: —
  verdict: never verified
    src/migrate.rs:12

REQ074 — Author a requirement into the ReqForge collection
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/create.rs:11
    src/create.rs:107 → authors_a_readable_unreviewed_requirement
    src/create.rs:141 → refuses_to_overwrite_an_existing_id

REQ075 — Scan requirement-trace tags and resolve them to the source they annotate
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/trace/carve.rs:8
    src/trace/languages.rs:7 → Language
    src/trace/mod.rs:19 → carve
    src/trace/resolve.rs:12
    src/trace/tags.rs:9
    src/trace/mod.rs:157 → verifies_tag_resolves_through_attribute_to_the_fn
    src/trace/mod.rs:183 → a_hyphenless_id_is_accepted
    src/trace/mod.rs:198 → implements_aliases_map_to_one_kind
    src/trace/mod.rs:213 → a_module_level_tag_resolves_to_no_symbol
    src/trace/mod.rs:230 → an_appledouble_sidecar_is_not_scanned
    src/trace/mod.rs:248 → parenthetical_prose_ids_are_filtered_by_prefix

REQ076 — Run a tagged test and record it as asserted evidence
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/trace/run.rs:16
    src/verdict.rs:1054 → a_verdict_from_a_tagged_test_alone_is_asserted
    src/verdict.rs:1064 → a_mechanical_proof_dominates_the_verdict_correspondence
    src/verdict.rs:1081 → a_mechanical_inconclusive_does_not_make_an_asserted_holds_mechanical
    src/verdict.rs:1098 → a_tagged_fail_against_a_proof_is_divergence
    src/verdict.rs:1114 → correspondence_is_surfaced_in_report_and_render
    src/verdict.rs:1125 → an_engine_verdict_stays_mechanical

REQ077 — On-demand traceability report
  formalized: —   implemented: yes   verified: yes
  verdict: never verified
    src/report.rs:12
    src/report.rs:401 → a_row_joins_formalization_tags_and_the_asserted_verdict
    src/report.rs:429 → outcomes_map_and_never_verified_is_distinct
    src/report.rs:520 → a_tag_for_an_unknown_id_is_an_orphan
    src/report.rs:531 → text_render_marks_stale_and_asserted

REQ078 — Verdict breadcrumb on the requirement
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/reqforge.rs:505 → record_verdict_appends_a_provreq_verdict_entry
    src/reqforge.rs:533 → record_verdict_appends_and_does_not_replace
    src/reqforge.rs:562 → record_verdict_rejects_unknown_item
    src/verify.rs:899 → a_verdict_transition_is_first_or_changed_status

REQ079 — Report recommends retiring a redundant asserted test
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/report.rs:462 → recommends_retiring_an_asserted_test_a_mechanical_verdict_covers
    src/report.rs:488 → keeps_a_sole_or_stronger_asserted_test
    src/report.rs:546 → text_render_shows_recommendations_with_the_caveat

REQ080 — Validation fails on an orphan code tag
  formalized: —   implemented: —   verified: yes
  verdict: never verified
    src/check.rs:105 → check_fails_on_an_orphan_tag
