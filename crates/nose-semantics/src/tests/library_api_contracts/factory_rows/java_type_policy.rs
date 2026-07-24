use super::*;

#[test]
fn java_type_policy_keeps_callee_identity_coordinate_only() {
    assert_eq!(
        library_api_callee_contract_hash(java_util_static_member("List", "of")),
        stable_symbol_hash("java_util_static_member:List:of")
    );
    assert_eq!(
        library_api_callee_contract_hash(LibraryApiCalleeContract::JavaStaticMember {
            owner: JavaTypeReference::imported_unshadowed(
                "com.google.common.collect",
                "ImmutableList",
                None,
            ),
            method: "of",
        }),
        stable_symbol_hash("java_static_member:com.google.common.collect:ImmutableList:of")
    );
    assert_eq!(
        library_api_callee_contract_hash(LibraryApiCalleeContract::JavaUtilConstructor {
            type_ref: JavaTypeReference::imported_unshadowed(
                "java.util",
                "ArrayList",
                Some("java.util.ArrayList"),
            ),
        }),
        stable_symbol_hash("java_util_constructor:java.util:ArrayList:java.util.ArrayList")
    );
}
