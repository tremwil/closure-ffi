use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2 as pm2;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, visit::Visit, visit_mut::VisitMut};

struct MacroInput {
    crate_path: syn::Path,
    alias: syn::ItemType,
    bare_fn: syn::TypeBareFn,
}

impl syn::parse::Parse for MacroInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let crate_path = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let alias: syn::ItemType = input.parse()?;

        match &*alias.ty {
            syn::Type::BareFn(bare_fn) => {
                if let Some(lt) = &bare_fn.lifetimes {
                    if lt.lifetimes.len() > 3 {
                        return Err(syn::Error::new_spanned(
                            &lt.lifetimes,
                            "At most 3 higher-ranked lifetimes are supported",
                        ));
                    }
                }

                // Check that bare_fn has no implicit lifetimes
                struct HasImplicitBoundLt(Vec<pm2::Span>);
                impl<'a> Visit<'a> for HasImplicitBoundLt {
                    fn visit_lifetime(&mut self, i: &'a syn::Lifetime) {
                        if i.ident.to_string() == "_" {
                            self.0.push(i.span());
                        }
                    }

                    fn visit_type_reference(&mut self, i: &'a syn::TypeReference) {
                        match i.lifetime {
                            Some(_) => self.visit_type(&i.elem),
                            None => self.0.push(i.and_token.span),
                        }
                    }
                }
                let mut implicit_lt_check = HasImplicitBoundLt(Vec::default());
                implicit_lt_check.visit_type_bare_fn(bare_fn);

                let mut implicit_lt_err = None;
                for err_span in implicit_lt_check.0 {
                    let err =
                        syn::Error::new(err_span, "Implicit lifetimes are not permitted here");
                    match implicit_lt_err.as_mut() {
                        None => implicit_lt_err = Some(err),
                        Some(e) => e.combine(err),
                    }
                }
                match implicit_lt_err {
                    Some(err) => Err(err),
                    None => Ok(Self {
                        crate_path,
                        bare_fn: bare_fn.clone(),
                        alias,
                    }),
                }
            }
            other => Err(syn::Error::new_spanned(
                other,
                &format!(
                    "Expected bare function type, got {}",
                    other.to_token_stream().to_string()
                ),
            )),
        }
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
        unsafety: fun.unsafety,
        abi: fun.abi.clone(),
        fn_token: syn::Token![fn](pm2::Span::call_site()),
        ident,
        generics: syn::Generics {
            lt_token: fun.lifetimes.as_ref().map(|lt| lt.lt_token),
            params: fun.lifetimes.as_ref().map(|lt| lt.lifetimes.clone()).unwrap_or_default(),
            gt_token: fun.lifetimes.as_ref().map(|lt| lt.gt_token),
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
pub fn bare_hrtb(tokens: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(tokens as MacroInput);

    input
        .bare_fn
        .unsafety
        .get_or_insert(syn::Token![unsafe](pm2::Span::call_site()));

    let bare_fn = &input.bare_fn;

    let thunk_ident = syn::Ident::new("thunk", pm2::Span::call_site());
    let arg_idents: Vec<_> = (0..input.bare_fn.inputs.len())
        .map(|i| syn::Ident::new(&format!("a{i}"), pm2::Span::call_site()))
        .collect();

    let mut thunk_sig = bare_fn_to_sig(bare_fn, thunk_ident.clone(), &arg_idents);

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
    let cc_marker_ident = syn::Ident::new(
        &format!("_{}_CCMarker", &input.alias.ident),
        pm2::Span::call_site(),
    );
    let crate_path = &input.crate_path;

    struct ImplDetails {
        thunk_trait_path: &'static str,
        fn_trait_path: &'static str,
        const_name: &'static str,
        body: pm2::TokenStream,
    }

    let impl_blocks: [ImplDetails; 3] = [
        ImplDetails {
            thunk_trait_path: "traits::FnOnceThunk",
            fn_trait_path: "::core::ops::FnOnce",
            const_name: "THUNK_TEMPLATE_ONCE",
            body: quote! {
                let closure_ptr: *mut #f_ident;
                #crate_path::arch::_thunk_asm!(closure_ptr);
                #crate_path::arch::_never_inline(|| closure_ptr.read()(#(#arg_idents),*))
            },
        },
        ImplDetails {
            thunk_trait_path: "traits::FnMutThunk",
            fn_trait_path: "::core::ops::FnMut",
            const_name: "THUNK_TEMPLATE_MUT",
            body: quote! {
                let closure_ptr: *mut #f_ident;
                #crate_path::arch::_thunk_asm!(closure_ptr);
                #crate_path::arch::_never_inline(|| (&mut *closure_ptr)(#(#arg_idents),*))
            },
        },
        ImplDetails {
            thunk_trait_path: "traits::FnThunk",
            fn_trait_path: "::core::ops::Fn",
            const_name: "THUNK_TEMPLATE",
            body: quote! {
                let closure_ptr: *const #f_ident;
                #crate_path::arch::_thunk_asm!(closure_ptr);
                #crate_path::arch::_never_inline(|| (&*closure_ptr)(#(#arg_idents),*))
            },
        },
    ];

    let alias_ident = &input.alias.ident;
    let alias_attrs = &input.alias.attrs;
    let alias_vis = &input.alias.vis;
    let alias_gen = &input.alias.generics;
    let (alias_impl_gen, alias_ty_params, alias_where) = &input.alias.generics.split_for_impl();

    let impls = impl_blocks.iter().map(|impl_block| {
        let fn_bound =
            bare_fn_to_trait_bound(&input.bare_fn, path_from_str(impl_block.fn_trait_path));
        let const_ident = syn::Ident::new(impl_block.const_name, pm2::Span::call_site());
        let body = &impl_block.body;
        let mut thunk_trait = input.crate_path.clone();
        thunk_trait.segments.extend(path_from_str(impl_block.thunk_trait_path).segments);

        let mut generics = input.alias.generics.clone();
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
            unsafe impl #impl_generics #thunk_trait<#alias_ident #alias_ty_params>
            for (#cc_marker_ident, #f_ident) #where_clause
            {
                const #const_ident: *const ::core::primitive::u8 = {
                    #thunk_sig {
                        #body
                    }
                    #thunk_ident::<#(#sig_tys),*> as *const ::core::primitive::u8
                };
            }
        }
    });

    let alias_ident_lit = syn::LitStr::new(&alias_ident.to_string(), pm2::Span::call_site());

    let mut punc_impl_lifetimes =
        bare_fn.lifetimes.as_ref().map(|lt| lt.lifetimes.clone()).unwrap_or_default();
    punc_impl_lifetimes.extend((punc_impl_lifetimes.len()..3).map(|i| {
        syn::GenericParam::Lifetime(syn::LifetimeParam::new(syn::Lifetime::new(
            &format!("'_extra_{i}"),
            pm2::Span::call_site(),
        )))
    }));
    let impl_lifetimes: Vec<_> = punc_impl_lifetimes.iter().collect();

    let tuple_args = bare_fn.inputs.iter().map(|i| &i.ty);
    let bare_fn_output = match &bare_fn.output {
        syn::ReturnType::Default => &syn::Type::Tuple(syn::TypeTuple {
            paren_token: syn::token::Paren(pm2::Span::call_site()),
            elems: syn::punctuated::Punctuated::new(),
        }),
        syn::ReturnType::Type(_, ty) => &*ty,
    };
    let arg_indices = (0..bare_fn.inputs.len() as u32).map(|index| {
        syn::Member::Unnamed(syn::Index {
            index,
            span: pm2::Span::call_site(),
        })
    });

    quote! {
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::default::Default)]
        #[doc(hidden)]
        struct #cc_marker_ident;

        #(#alias_attrs)*
        #[repr(transparent)]
        #alias_vis struct #alias_ident #alias_gen (pub #bare_fn) #alias_where;

        impl #alias_impl_gen #alias_ident #alias_ty_params #alias_where {
            /// Returns an instance of the calling convention marker type for this bare function.
            pub fn cc() -> #cc_marker_ident {
                #cc_marker_ident::default()
            }
        }

        impl #alias_impl_gen ::core::clone::Clone for #alias_ident #alias_ty_params #alias_where {
            fn clone(&self) -> Self {
                Self(self.0)
            }
        }

        impl #alias_impl_gen ::core::marker::Copy for #alias_ident #alias_ty_params #alias_where {}

        impl #alias_impl_gen ::core::fmt::Debug for #alias_ident #alias_ty_params #alias_where {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_tuple(#alias_ident_lit)
                    .field(&self.0)
                    .finish()
            }
        }

        impl #alias_impl_gen ::core::convert::From<#bare_fn> for #alias_ident #alias_ty_params #alias_where {
            fn from(value: #bare_fn) -> Self {
                Self(value)
            }
        }

        impl #alias_impl_gen ::core::convert::Into<#bare_fn> for #alias_ident #alias_ty_params #alias_where {
            fn into(self) -> #bare_fn {
                self.0
            }
        }

        impl #alias_impl_gen ::core::ops::Deref for #alias_ident #alias_ty_params #alias_where {
            type Target = #bare_fn;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl #alias_impl_gen #crate_path::traits::FnPtr for #alias_ident #alias_ty_params #alias_where {
            type CC = #cc_marker_ident;
            type Args<#punc_impl_lifetimes> = (#(#tuple_args,)*) where Self: #(#impl_lifetimes)+*;
            type Ret<#punc_impl_lifetimes> = #bare_fn_output where Self: #(#impl_lifetimes)+*;

            unsafe fn call<#punc_impl_lifetimes>(
                self,
                args: Self::Args<#punc_impl_lifetimes>
            ) -> Self::Ret<#punc_impl_lifetimes>
                where Self: #(#impl_lifetimes)+*
            {
                (self.0)(#(args.#arg_indices,)*)
            }
        }

        #(#impls)*
    }
    .into()
}
