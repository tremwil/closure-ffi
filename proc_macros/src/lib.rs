use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2 as pm2;
use quote::quote;
use syn::{parse_macro_input, visit_mut::VisitMut};

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
    generics: syn::Generics,
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
            generics: generics.unwrap_or_default(),
            bare_fn: input.parse()?,
        })
    }
}

fn bare_fn_to_trait_bound(fun: &syn::TypeBareFn, mut path: syn::Path) -> syn::TraitBound {
    let fn_part = path.segments.last_mut().unwrap();
    fn_part.arguments = syn::PathArguments::Parenthesized(syn::ParenthesizedGenericArguments {
        paren_token: Default::default(),
        inputs: fun.inputs.iter().map(|arg| arg.ty.clone()).collect(),
        output: fun.output.clone(),
    });

    syn::TraitBound {
        paren_token: None,
        modifier: syn::TraitBoundModifier::None,
        lifetimes: fun.lifetimes.clone(),
        path,
    }
}

fn bare_fn_to_sig(
    fun: &syn::TypeBareFn,
    ident: syn::Ident,
    arg_idents: &[syn::Ident],
) -> syn::Signature {
    syn::Signature {
        constness: None,
        asyncness: None,
        unsafety: fun.unsafety.clone(),
        abi: fun.abi.clone(),
        fn_token: syn::Token![fn](pm2::Span::call_site()),
        ident,
        generics: syn::Generics {
            lt_token: fun.lifetimes.as_ref().map(|lt| lt.lt_token.clone()),
            params: fun.lifetimes.as_ref().map(|lt| lt.lifetimes.clone()).unwrap_or_default(),
            gt_token: fun.lifetimes.as_ref().map(|lt| lt.gt_token.clone()),
            where_clause: None,
        },
        paren_token: syn::token::Paren::default(),
        inputs: fun
            .inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                syn::FnArg::Typed(syn::PatType {
                    attrs: Default::default(),
                    pat: Box::new(syn::Pat::Ident(syn::PatIdent {
                        attrs: Default::default(),
                        by_ref: None,
                        mutability: None,
                        ident: arg_idents[i].clone(),
                        subpat: None,
                    })),
                    colon_token: syn::Token![:](pm2::Span::call_site()),
                    ty: Box::new(input.ty.clone()),
                })
            })
            .collect(),
        variadic: None,
        output: fun.output.clone(),
    }
}

fn path_from_str(str: &str) -> syn::Path {
    syn::parse(TokenStream::from_str(str).unwrap()).unwrap()
}

struct ReplaceLt<F: FnMut(&mut syn::Lifetime)>(F);

impl<F: FnMut(&mut syn::Lifetime)> syn::visit_mut::VisitMut for ReplaceLt<F> {
    fn visit_lifetime_mut(&mut self, i: &mut syn::Lifetime) {
        self.0(i)
    }
}

#[proc_macro]
pub fn hrtb_cc(tokens: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(tokens as MacroInput);
    input
        .bare_fn
        .unsafety
        .get_or_insert(syn::Token![unsafe](pm2::Span::call_site()));

    let attrs = &input.attrs;
    let bare_fn = &input.bare_fn;

    let thunk_ident = syn::Ident::new("thunk", pm2::Span::call_site());
    let arg_idents: Vec<_> = (0..input.bare_fn.inputs.len())
        .map(|i| syn::Ident::new(&format!("a{i}"), pm2::Span::call_site()))
        .collect();

    let mut thunk_sig = bare_fn_to_sig(&bare_fn, thunk_ident.clone(), &arg_idents);

    let bare_fn_lt_idents = bare_fn
        .lifetimes
        .as_ref()
        .map(|lt| {
            lt.lifetimes
                .iter()
                .map(|p| match p {
                    syn::GenericParam::Lifetime(lt) => lt.lifetime.ident.to_string(),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ReplaceLt(|lt| {
        if let Some(for_ident) = bare_fn_lt_idents.iter().find(|&l| l == &lt.ident.to_string()) {
            lt.ident = syn::Ident::new(&format!("for_{for_ident}"), pm2::Span::call_site())
        }
    })
    .visit_signature_mut(&mut thunk_sig);

    let f_ident = syn::Ident::new("_F", pm2::Span::call_site());

    struct ImplDetails {
        thunk_trait_path: &'static str,
        fn_trait_path: &'static str,
        const_name: &'static str,
        body: pm2::TokenStream,
    }

    let impl_blocks: [ImplDetails; 3] = [
        ImplDetails {
            thunk_trait_path: "::closure_ffi::thunk::FnOnceThunk",
            fn_trait_path: "::core::ops::FnOnce",
            const_name: "THUNK_TEMPLATE_ONCE",
            body: quote! {
                let closure_ptr: *mut #f_ident;
                ::closure_ffi::arch::_thunk_asm!(closure_ptr);
                closure_ptr.read()(#(#arg_idents),*)
            },
        },
        ImplDetails {
            thunk_trait_path: "::closure_ffi::thunk::FnMutThunk",
            fn_trait_path: "::core::ops::FnMut",
            const_name: "THUNK_TEMPLATE_MUT",
            body: quote! {
                let closure_ptr: *mut #f_ident;
                ::closure_ffi::arch::_thunk_asm!(closure_ptr);
                (&mut *closure_ptr)(#(#arg_idents),*)
            },
        },
        ImplDetails {
            thunk_trait_path: "::closure_ffi::thunk::FnThunk",
            fn_trait_path: "::core::ops::Fn",
            const_name: "THUNK_TEMPLATE",
            body: quote! {
                let closure_ptr: *const #f_ident;
                ::closure_ffi::arch::_thunk_asm!(closure_ptr);
                (&*closure_ptr)(#(#arg_idents),*)
            },
        },
    ];

    let impls = impl_blocks.iter().map(|impl_block| {
        let fn_bound =
            bare_fn_to_trait_bound(&input.bare_fn, path_from_str(impl_block.fn_trait_path));
        let const_ident = syn::Ident::new(&impl_block.const_name, pm2::Span::call_site());
        let body = &impl_block.body;
        let thunk_trait = path_from_str(&impl_block.thunk_trait_path);

        let mut generics = input.generics.clone();
        generics.params.push(syn::GenericParam::Type(syn::TypeParam {
            attrs: Default::default(),
            ident: f_ident.clone(),
            colon_token: Some(syn::Token![:](pm2::Span::call_site())),
            bounds: [syn::TypeParamBound::Trait(fn_bound)].into_iter().collect(),
            eq_token: None,
            default: None,
        }));

        let mut thunk_sig = thunk_sig.clone();
        thunk_sig.generics.params.extend(generics.params.clone());

        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let sig_tys = generics.type_params().map(|t| &t.ident);

        quote! {
            unsafe impl #impl_generics #thunk_trait<_CustomThunk, #bare_fn>
            for (_CustomThunk, #f_ident) #where_clause
            {
                const #const_ident: *const u8 = {
                    #thunk_sig {
                        #body
                    }
                    #thunk_ident::<#(#sig_tys),*> as *const u8
                };
            }
        }
    });

    quote! {{
        #(#attrs)*
        #[derive(Debug, Clone, Copy)]
        struct _CustomThunk;

        #(#impls)*

        _CustomThunk
    }}
    .into()
}
