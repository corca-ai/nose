use super::*;

#[test]
fn language_predicates_preserve_existing_gates() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(
            profile.operators().primitive_order_comparisons(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        let byte_pack = profile
            .operators()
            .c_integer_byte_pack_contract(CBytePackWidth::U32);
        assert_eq!(byte_pack.is_some(), lang == Lang::C);
        if let Some(contract) = byte_pack {
            assert_eq!(contract.base_domain, DomainRequirement::BYTE_ARRAY);
            assert_eq!(
                contract.required_high_lane_cast,
                Some(SourceFactKind::Cast(SourceCastKind::CUnsigned32))
            );
        }
        assert_eq!(
            profile.effects().non_overloadable_index_assignment(),
            matches!(lang, Lang::C | Lang::Go | Lang::Java)
        );
        assert_eq!(
            profile.effects().java_this_field_place(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().js_like_shadowed_module_bindings(),
            matches!(
                lang,
                Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
            )
        );
        assert_eq!(
            profile.modules().java_class_literal_exports(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().java_type_declarations_shadow_stdlib(),
            lang == Lang::Java
        );
        assert_eq!(
            profile.modules().go_import_namespace_facts(),
            lang == Lang::Go
        );
    }
}

#[test]
fn stdlib_predicates_preserve_existing_gates() {
    for &lang in ALL_LANGS {
        let stdlib = semantics(lang).stdlib();
        assert_eq!(stdlib.python_collection_factories(), lang == Lang::Python);
        assert_eq!(stdlib.python_deque_factory(), lang == Lang::Python);
        assert_eq!(stdlib.java_collection_factories(), lang == Lang::Java);
        assert_eq!(stdlib.java_map_factories(), lang == Lang::Java);
        assert_eq!(stdlib.java_primitive_integer_ops(), lang == Lang::Java);
        assert_eq!(stdlib.ruby_set_factory(), lang == Lang::Ruby);
        assert_eq!(stdlib.rust_vec_macro_factory(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_vec_new_factory(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_std_collection_factories(), lang == Lang::Rust);
        assert_eq!(stdlib.rust_std_map_factories(), lang == Lang::Rust);
        assert_eq!(stdlib.go_literal_zero_map_lookup(), lang == Lang::Go);
        assert_eq!(stdlib.rust_filter_map_option_contract(), lang == Lang::Rust);
    }
}
