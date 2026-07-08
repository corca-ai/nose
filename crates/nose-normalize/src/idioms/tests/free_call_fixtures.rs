use super::support::*;

pub(super) fn free_call_il(lang: Lang, name: &str, shadow_name: bool) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let mut module_kids = Vec::new();
    let mut cid_names = Vec::new();
    if shadow_name {
        let sym = interner.intern(name);
        cid_names.push(sym);
        module_kids.push(b.add(NodeKind::Param, Payload::Cid(0), sp(), &[]));
    }
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(name)),
        sp(),
        &[],
    );
    let arg = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("x")),
        sp(),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[callee, arg]);
    module_kids.push(call);
    let il = finish_module_il(b, lang, module_kids, cid_names);
    (il, interner, call)
}
