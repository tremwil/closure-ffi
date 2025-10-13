use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2 as pm2;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, visit::Visit, visit_mut::VisitMut};

struct MacroInput {
    thunk_attrs: Vec<syn::Attribute>,
    crate_path: syn::Path,
    alias: syn::ItemType,
    bare_fn: syn::TypeBareFn,
}

impl syn::parse::Parse for MacroInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let thunk_attrs = input.call(syn::Attribute::parse_outer)?;
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
                        if i.ident == "_" {
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
                    let err = syn::Error::new(
                        err_span,
                        "Implicit lifetimes are not permitted; you must name this lifetime",
                    );
                    match implicit_lt_err.as_mut() {
                        None => implicit_lt_err = Some(err),
                        Some(e) => e.combine(err),
                    }
                }
                match implicit_lt_err {
                    Some(err) => Err(err),
                    None => Ok(Self {
                        thunk_attrs,
                        crate_path,
                        bare_fn: bare_fn.clone(),
                        alias,
                    }),
                }
            }
            other => Err(syn::Error::new_spanned(
                other,
                format!(
                    "Expected bare function type, got {}",
                    other.to_token_stream()
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
        &format!("{}_CC", &input.alias.ident),
        pm2::Span::call_site(),
    );
    let crate_path = &input.crate_path;
    let thunk_attrs = &input.thunk_attrs;
    let alias_ident = &input.alias.ident;
    let alias_attrs = &input.alias.attrs;
    let alias_vis = &input.alias.vis;
    let alias_gen = &input.alias.generics;
    let (alias_impl_gen, alias_ty_params, alias_where) = &input.alias.generics.split_for_impl();
    let mut impl_lifetimes =
        bare_fn.lifetimes.as_ref().map(|lt| lt.lifetimes.clone()).unwrap_or_default();
    impl_lifetimes.extend((impl_lifetimes.len()..3).map(|i| {
        syn::GenericParam::Lifetime(syn::LifetimeParam::new(syn::Lifetime::new(
            &format!("'_extra_{i}"),
            pm2::Span::call_site(),
        )))
    }));
    let arg_indices: Vec<_> = (0..bare_fn.inputs.len() as u32)
        .map(|index| {
            syn::Member::Unnamed(syn::Index {
                index,
                span: pm2::Span::call_site(),
            })
        })
        .collect();

    struct ImplDetails {
        thunk_trait_path: &'static str,
        fn_trait_path: &'static str,
        const_name: &'static str,
        call_ident: &'static str,
        call_receiver: pm2::TokenStream,
        thunk_body: pm2::TokenStream,
        factory_ident: &'static str,
        factory_bound: &'static str,
    }

    let impl_blocks: [ImplDetails; 3] = [
        ImplDetails {
            thunk_trait_path: "traits::FnOnceThunk",
            fn_trait_path: "::core::ops::FnOnce",
            const_name: "THUNK_TEMPLATE_ONCE",
            call_ident: "call_once",
            factory_ident: "make_once_thunk",
            factory_bound: "traits::PackedFnOnce",
            call_receiver: syn::Token![self](pm2::Span::call_site()).into_token_stream(),
            thunk_body: quote! {
                if const { ::core::mem::size_of::<#f_ident>() == 0 } {
                    let closure: #f_ident = unsafe { ::core::mem::zeroed() };
                    closure(#(#arg_idents),*)
                }
                else {
                    let closure_ptr: *mut #f_ident;
                    #crate_path::arch::_thunk_asm!(closure_ptr);
                    #crate_path::arch::_invoke(|| closure_ptr.read()(#(#arg_idents),*))
                }
            },
        },
        ImplDetails {
            thunk_trait_path: "traits::FnMutThunk",
            fn_trait_path: "::core::ops::FnMut",
            const_name: "THUNK_TEMPLATE_MUT",
            call_ident: "call_mut",
            factory_ident: "make_mut_thunk",
            factory_bound: "traits::PackedFnMut",
            call_receiver: quote! { &mut self },
            thunk_body: quote! {
                if const { ::core::mem::size_of::<#f_ident>() == 0 } {
                    let closure: &mut #f_ident = unsafe { &mut *::core::ptr::dangling_mut() };
                    closure(#(#arg_idents),*)
                }
                else {
                    let closure_ptr: *mut #f_ident;
                    #crate_path::arch::_thunk_asm!(closure_ptr);
                    #crate_path::arch::_invoke(|| (&mut *closure_ptr)(#(#arg_idents),*))
                }
            },
        },
        ImplDetails {
            thunk_trait_path: "traits::FnThunk",
            fn_trait_path: "::core::ops::Fn",
            const_name: "THUNK_TEMPLATE",
            call_ident: "call",
            factory_ident: "make_thunk",
            factory_bound: "traits::PackedFn",
            call_receiver: quote! { &self },
            thunk_body: quote! {
                if const { ::core::mem::size_of::<#f_ident>() == 0 } {
                    let closure: &#f_ident = unsafe { &*::core::ptr::dangling() };
                    closure(#(#arg_idents),*)
                }
                else {
                    let closure_ptr: *const #f_ident;
                    #crate_path::arch::_thunk_asm!(closure_ptr);
                    #crate_path::arch::_invoke(|| (&*closure_ptr)(#(#arg_idents),*))
                }
            },
        },
    ];

    struct ImplTokens {
        thunk_impl: pm2::TokenStream,
        thunk_factory_impl: pm2::TokenStream,
    }

    let impl_tokens: Vec<_> = impl_blocks.iter().map(|impl_block| {
        let fn_bound =
            bare_fn_to_trait_bound(&input.bare_fn, path_from_str(impl_block.fn_trait_path));

        let const_ident = syn::Ident::new(impl_block.const_name, pm2::Span::call_site());
        let call_ident = syn::Ident::new(impl_block.call_ident, pm2::Span::call_site());
        let call_receiver = &impl_block.call_receiver;
        let body = &impl_block.thunk_body;
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

        let thunk_impl = quote! {
            unsafe impl #impl_generics #thunk_trait<#alias_ident #alias_ty_params>
            for (#cc_marker_ident, #f_ident) #where_clause
            {
                const #const_ident: *const ::core::primitive::u8 = {
                    #(#thunk_attrs)*
                    #[allow(clippy::too_many_arguments)]
                    #thunk_sig {
                        #body
                    }
                    #thunk_ident::<#(#sig_tys),*> as *const ::core::primitive::u8
                };

                #[allow(unused_variables)]
                #[inline(always)]
                unsafe fn #call_ident<'_x, '_y, '_z>(#call_receiver, args: <#alias_ident #alias_ty_params as #crate_path::traits::FnPtr>::Args<'_x, '_y, '_z>
                ) ->
                    <#alias_ident #alias_ty_params as #crate_path::traits::FnPtr>::Ret<'_x, '_y, '_z>
                {
                    (self.1)(#(args.#arg_indices,)*)
                }
            }
        };

        let factory_bound_path = path_from_str(impl_block.factory_bound);
        let factory_ident = syn::Ident::new(impl_block.factory_ident, pm2::Span::call_site());
        let thunk_factory_impl = quote! {
            #[inline(always)]
            #[allow(unused_mut)]
            fn #factory_ident<F>(mut fun: F) -> impl #thunk_trait<Self>
            where
                F: for<'_x, '_y, '_z> #crate_path::#factory_bound_path<'_x, '_y, '_z, Self>
            {
                #[inline(always)]
                fn coerce #impl_generics (fun: #f_ident) -> #f_ident #where_clause {
                    fun
                }

                let coerced = coerce(move |#(#arg_idents,)*| fun((#(#arg_idents,)*)));
                (#cc_marker_ident, coerced)
            }
        };

        ImplTokens {
            thunk_impl,
            thunk_factory_impl
        }
    }).collect();

    let alias_ident_lit = syn::LitStr::new(&alias_ident.to_string(), pm2::Span::call_site());
    let alias_ident_doc_lit =
        syn::LitStr::new(&format!("[`{alias_ident}`]."), pm2::Span::call_site());

    let tuple_args = bare_fn.inputs.iter().map(|i| &i.ty);
    let bare_fn_output = match &bare_fn.output {
        syn::ReturnType::Default => &syn::Type::Tuple(syn::TypeTuple {
            paren_token: syn::token::Paren(pm2::Span::call_site()),
            elems: syn::punctuated::Punctuated::new(),
        }),
        syn::ReturnType::Type(_, ty) => ty,
    };

    let trait_impls = impl_tokens.iter().map(|t| &t.thunk_impl);
    let thunk_factory_impls = impl_tokens.iter().map(|t| &t.thunk_factory_impl);

    quote! {
        /// Calling convention marker type for higher-ranked bare function wrapper type
        #[doc = #alias_ident_doc_lit]
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::default::Default)]
        #alias_vis struct #cc_marker_ident;

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

        impl #alias_impl_gen ::core::convert::From<#alias_ident #alias_ty_params> for #bare_fn #alias_where {
            fn from(value: #alias_ident #alias_ty_params) -> Self {
                value.0
            }
        }

        impl #alias_impl_gen ::core::ops::Deref for #alias_ident #alias_ty_params #alias_where {
            type Target = #bare_fn;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        unsafe impl #alias_impl_gen #crate_path::traits::FnPtr for #alias_ident #alias_ty_params #alias_where {
            type CC = #cc_marker_ident;
            type Args<#impl_lifetimes> = (#(#tuple_args,)*);
            type Ret<#impl_lifetimes> = #bare_fn_output;

            #[inline(always)]
            unsafe fn call<#impl_lifetimes>(
                self,
                args: Self::Args<#impl_lifetimes>
            ) -> Self::Ret<#impl_lifetimes>
            {
                (self.0)(#(args.#arg_indices,)*)
            }

            #[inline(always)]
            unsafe fn from_ptr(ptr: *const ()) -> Self {
                unsafe { core::mem::transmute_copy(&ptr) }
            }

            #[inline(always)]
            fn to_ptr(self) -> *const () {
                self.0 as *const _
            }

            #(#thunk_factory_impls)*
        }

        #(#trait_impls)*
    }
    .into()
}
