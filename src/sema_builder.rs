use crate::ast;

#[derive(Debug, Clone)]
pub struct ItemSig {
    pub name: String,
    pub ix: usize,
}

pub fn collect_signatures(items: &[ast::Item]) -> Vec<ItemSig> {
    items
        .iter()
        .enumerate()
        .map(|(ix, item)| ItemSig {
            name: item.ident.clone(),
            ix,
        })
        .collect()
}

pub fn collect_all_vardecls<'a>(
    items: &'a [ast::Item],
    sigs: &[ItemSig],
) -> Vec<(&'a str, &'a ast::VarDecl)> {
    let mut out = Vec::new();
    for sig in sigs {
        let item = &items[sig.ix];
        for vardecl in &item.blk {
            out.push((&item.ident[..], vardecl));
        }
    }
    out
}