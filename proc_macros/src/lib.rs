use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::discouraged::AnyDelimiter, parse_macro_input};

// the Parse impl for syn::Generics ignores the where clause. This expects
// it right after the generic parameters.
struct GenericsWithWhere(syn::Generics);
impl syn::parse::Parse for GenericsWithWhere {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(GenericsWithWhere({
            let mut generics: syn::Generics = input.parse()?;
            generics.where_clause = input.parse()?;
            generics
        }))
    }
}

struct MacroInput {
    attrs: Vec<syn::Attribute>,
    generics: Option<syn::Generics>,
    bare_fn: syn::TypeBareFn,
}

impl syn::parse::Parse for MacroInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let all_attrs = input.call(syn::Attribute::parse_outer)?;
        let mut attrs = Vec::new();
        let mut generics = None;

        for attr in all_attrs {
            if !attr.path().is_ident("with") {
                attrs.push(attr);
            } else if generics.is_some() {
                return Err(syn::Error::new_spanned(
                    attr.path().get_ident(),
                    "with attribute is already present",
                ));
            } else {
                let meta_list = attr.meta.require_list()?;
                generics = Some(meta_list.parse_args::<GenericsWithWhere>()?.0);
            }
        }

        Ok(Self {
            attrs,
            generics,
            bare_fn: input.parse()?,
        })
    }
}

#[proc_macro]
pub fn hrtb_cc(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as MacroInput);

    let split_generics = input.generics.as_ref().map(|g| g.split_for_impl());
    let (impl_generics, ty_generics, where_clause) = (
        split_generics.as_ref().map(|s| s.0.clone()),
        split_generics.as_ref().map(|s| s.1.clone()),
        split_generics.as_ref().and_then(|s| s.2),
    );

    quote! {}.into()
}
