#![recursion_limit = "128"]
#![allow(warnings)]
#![allow(clippy::needless_borrowed_reference)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::match_ref_pats)] // This one I'd really like to clean up some day
#![allow(clippy::needless_late_init)]
#![allow(clippy::len_zero)]
#![allow(clippy::let_and_return)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::single_match)]

//! This crate allows automatic derivation of the Savefile-traits: Serialize, Deserialize, WithSchema, Packed and Introspect .
//! The documentation for this is found in the Savefile crate documentation.

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
extern crate syn;
#[macro_use]
extern crate proc_macro_error2;

use crate::savefile_abi::is_well_known;
use common::{
    check_is_remove, compile_time_check_reprc, compile_time_size, get_extra_where_clauses, parse_attr_tag,
    path_to_string, FieldInfo,
};
use std::sync::atomic::{AtomicBool, Ordering};
use proc_macro2::TokenStream;
use proc_macro2::{Span, TokenTree};
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
use std::iter::IntoIterator;
use syn::__private::bool;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::Type::Tuple;
use syn::{
    DeriveInput, FnArg, GenericArgument, GenericParam, Generics, Ident, ImplGenerics, Index, ItemTrait, Pat,
    PathArguments, ReturnType, TraitItem, Type, TypeGenerics, TypeParamBound, TypeTuple, WherePredicate,
};

fn implement_fields_serialize(
    field_infos: Vec<FieldInfo>,
    implicit_self: bool,
    index: bool,
) -> (TokenStream, Vec<TokenStream>) {
    let mut output = Vec::new();

    let defspan = proc_macro2::Span::call_site();
    let span = proc_macro2::Span::call_site();
    let local_serializer = quote_spanned! { defspan => local_serializer};

    let reprc = quote! {
        _savefile::prelude::Packed
    };

    let mut deferred_reprc: Option<(usize /*align*/, Vec<TokenStream>)> = None;
    fn realize_any_deferred(
        local_serializer: &TokenStream,
        deferred_reprc: &mut Option<(usize, Vec<TokenStream>)>,
        output: &mut Vec<TokenStream>,
    ) {
        let local_serializer: TokenStream = local_serializer.clone();
        if let Some((_align, deferred)) = deferred_reprc.take() {
            assert_eq!(deferred.is_empty(), false);
            let mut conditions = vec![];
            for item in deferred.windows(2) {
                let a = item[0].clone();
                let b = item[1].clone();
                if conditions.is_empty() == false {
                    conditions.push(quote!(&&));
                }
                conditions.push(quote!(
                    std::ptr::addr_of!(#a).add(1) as *const u8 == std::ptr::addr_of!(#b) as *const u8
                ));
            }
            if conditions.is_empty() {
                conditions.push(quote!(true));
            }
            let mut fallbacks = vec![];
            for item in deferred.iter() {
                fallbacks.push(quote!(
                <_ as _savefile::prelude::Serialize>::serialize(&#item, #local_serializer)?;
                ));
            }
            if deferred.len() == 1 {
                return output.push(quote!( #(#fallbacks)* ));
            }
            let mut iter = deferred.into_iter();
            let deferred_from = iter.next().expect("expected deferred_from");
            let deferred_to = iter.last().unwrap_or(deferred_from.clone());

            output.push(
                quote!(
                    unsafe {
                        if #(#conditions)* {
                         #local_serializer.raw_write_region(self,&#deferred_from,&#deferred_to, local_serializer.file_version)?;
                        } else {
                            #(#fallbacks)*
                        }
                    }
            ));
        }
    }

    let get_obj_id = |field: &FieldInfo| -> TokenStream {
        let objid = if index {
            assert!(implicit_self);
            let id = syn::Index {
                index: field.index,
                span,
            };
            quote! { self.#id}
        } else {
            let id = field.ident.clone().expect("Expected identifier[3]");
            if implicit_self {
                quote! { self.#id}
            } else {
                quote! { *#id}
            }
        };
        objid
    };

    for field in &field_infos {
        {
            let verinfo = parse_attr_tag(field.attrs);

            if verinfo.ignore {
                continue;
            }
            let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

            let removed = check_is_remove(field.ty);

            let type_size_align = compile_time_size(field.ty);
            let compile_time_reprc = compile_time_check_reprc(field.ty) && type_size_align.is_some();

            let obj_id = get_obj_id(field);

            if field_from_version == 0 && field_to_version == std::u32::MAX {
                if removed.is_removed() {
                    abort!(
                        field.ty.span(),
                        "The Removed type can only be used for removed fields. Use the savefile_versions attribute."
                    );
                }

                if compile_time_reprc {
                    let (_cursize, curalign) = type_size_align.expect("type_size_align");
                    if let Some((deferred_align, deferred_items)) = &mut deferred_reprc {
                        if *deferred_align == curalign {
                            deferred_items.push(obj_id);
                            continue;
                        }
                    } else {
                        deferred_reprc = Some((curalign, vec![obj_id]));
                        continue;
                    }
                }
                realize_any_deferred(&local_serializer, &mut deferred_reprc, &mut output);

                output.push(quote!(
                <_ as _savefile::prelude::Serialize>::serialize(&#obj_id, #local_serializer)?;
                ));
            } else {
                realize_any_deferred(&local_serializer, &mut deferred_reprc, &mut output);

                output.push(quote!(
                if #local_serializer.file_version >= #field_from_version && #local_serializer.file_version <= #field_to_version {
                    <_ as _savefile::prelude::Serialize>::serialize(&#obj_id, #local_serializer)?;
                }));
            }
        }
    }
    realize_any_deferred(&local_serializer, &mut deferred_reprc, &mut output);

    //let contents = format!("//{:?}",output);

    let total_reprc_opt: TokenStream;
    if field_infos.is_empty() == false {
        let first_field = get_obj_id(field_infos.first().expect("field_infos.first"));
        let last_field = get_obj_id(field_infos.last().expect("field_infos.last"));
        total_reprc_opt = quote!( unsafe { #local_serializer.raw_write_region(self,&#first_field, &#last_field, local_serializer.file_version)?; } );
    } else {
        total_reprc_opt = quote!();
    }

    let serialize2 = quote! {
        let local_serializer = serializer;

        if unsafe { <Self as #reprc>::repr_c_optimization_safe(local_serializer.file_version).is_yes() } {
            #total_reprc_opt
        } else {
            #(#output)*
        }
    };

    let fields_names = field_infos
        .iter()
        .map(|field| {
            let fieldname = field.ident.clone();
            quote! { #fieldname }
        })
        .collect();
    (serialize2, fields_names)
}

pub(crate) mod common;

mod serialize;

mod deserialize;

mod savefile_abi;

static HAVE_EMITTED_ASSERT_TIGHT: AtomicBool = AtomicBool::new(false);
fn assert_tight() -> TokenStream {
    if HAVE_EMITTED_ASSERT_TIGHT.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
        let tight = cfg!(feature="tight");
        quote! {
            const _ASSERT_TIGHT:() = {
                if #tight != savefile::TIGHT {
                    if savefile::TIGHT {
                        panic!("The feature 'tight' must be enabled in both savefile and savefile_derive, or in neither. It is only enabled in savefile.");
                    } else {
                        panic!("The feature 'tight' must be enabled in both savefile and savefile_derive, or in neither. It is only enabled in savefile-derive.");
                    }
                }
            };
        }
    }
    else {
        quote!()
    }
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn savefile_abi_exportable(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let parsed: ItemTrait = syn::parse(input.clone()).expect("Expected valid rust-code");

    let mut version = None;
    for item in attr.to_string().split(',') {
        let keyvals: Vec<_> = item.split('=').collect();
        if keyvals.len() != 2 {
            abort!(
                item.span(),
                "savefile_abi_exportable arguments should be of form #[savefile_abi_exportable(version=0)], not '{}'",
                attr
            );
        }
        let key = keyvals[0].trim();
        let val = keyvals[1].trim();
        match key {
            "version" => {
                if version.is_some() {
                    abort!(item.span(), "version specified more than once");
                }
                version = Some(
                    val.parse()
                        .unwrap_or_else(|_| abort!(item.span(), "Version must be numeric, but was: {}", val)),
                );
            }
            _ => abort!(item.span(), "Unknown savefile_abi_exportable key: '{}'", key),
        }
    }

    for attr in &parsed.attrs {
        let name_segs: Vec<_> = attr.path.segments.iter().map(|x| &x.ident).collect();
        if name_segs == ["async_trait"] || name_segs == ["async_trait", "async_trait"] {
            abort!(attr.path.segments.span(), "async_trait-attribute macro detected. The {} macro must go _before_ the #[savefile_abi_exportable(..)] macro!",
            attr.to_token_stream());
        }
    }
    let version: u32 = version.unwrap_or(0);

    let trait_name_str = parsed.ident.to_string();
    let trait_name = parsed.ident;
    let defspan = proc_macro2::Span::mixed_site();
    let uses = quote_spanned! { defspan =>
        extern crate savefile;
        extern crate savefile_abi;
        extern crate savefile_derive;
        use savefile::prelude::{Packed, Schema, SchemaPrimitive, WithSchema, WithSchemaContext, get_schema, get_result_schema, Serializer, Serialize, Deserializer, Deserialize, SavefileError, deserialize_slice_as_vec, ReadBytesExt,LittleEndian,ReceiverType,AbiMethodArgument, AbiMethod, AbiMethodInfo,AbiTraitDefinition};
        use savefile_abi::{parse_return_value_impl,abi_result_receiver,abi_boxed_trait_receiver, FlexBuffer, AbiExportable, TraitObject, PackagedTraitObject, Owning, AbiErrorMsg, RawAbiCallResult, AbiConnection, AbiConnectionMethod, AbiProtocol, abi_entry_light, AbiWaker};
        use std::collections::HashMap;
        use std::mem::MaybeUninit;
        use std::io::Cursor;
        use std::pin::Pin;
        use std::marker::Unpin;
        use std::future::Future;
        use std::task::{Waker, Poll, Context};
        use std::sync::Arc;
        use savefile_derive::savefile_abi_exportable;
    };

    let mut method_metadata: Vec<TokenStream> = vec![];
    let mut callee_method_trampoline: Vec<TokenStream> = vec![];
    let mut caller_method_trampoline = vec![];
    let mut extra_definitions = HashMap::new();

    if parsed.generics.params.is_empty() == false {
        abort!(
            parsed.generics.params.span(),
            "Savefile does not support generic traits."
        );
    }
    let mut send = false;
    let mut sync = false;
    for supertrait in parsed.supertraits.iter() {
        match supertrait {
            TypeParamBound::Trait(trait_bound) => {
                if let Some(lif) = &trait_bound.lifetimes {
                    abort!(lif.span(), "Savefile does not support lifetimes");
                }
                if let Some(seg) = trait_bound.path.segments.last() {
                    let id = seg.ident.to_string();
                    match id.as_str() {
                        "Copy" => abort!(seg.span(), "Savefile does not support Copy bounds for traits. The reason is savefile-abi needs to generate a wrapper, and this wrapper can't be copy."),
                        "Clone" => abort!(seg.span(), "Savefile does not support Clone bounds for traits. The reason is savefile-abi needs to generate a wrapper, and this wrapper can't be clone."),
                        /* these are ok, the wrappers actually do implement these*/
                        "Sync" => { sync = true;}
                        "Send" => { send = true;}
                        "Sized" => {}
                        "Debug" => {}
                        _ => abort!(seg.span(), "Savefile does not support bounds for traits. The reason is savefile-abi needs to generate a wrapper, and this wrapper doesn't know how to implement arbitrary bounds."),
                    }
                }
            }
            TypeParamBound::Lifetime(lif) => {
                if lif.ident != "static" {
                    abort!(lif.span(), "Savefile does not support lifetimes");
                }
            }
        }
    }

    if parsed.generics.where_clause.is_some() {
        abort!(
            parsed.generics.where_clause.span(),
            "Savefile does not support where-clauses for traits"
        );
    }

    for (method_number, item) in parsed.items.iter().enumerate() {
        if method_number > u16::MAX.into() {
            abort!(item.span(), "Savefile only supports 2^16 methods per interface. Sorry.");
        }
        let method_number = method_number as u16;

        match item {
            TraitItem::Const(c) => {
                abort!(
                    c.span(),
                    "savefile_abi_exportable does not support associated consts: {}",
                    c.ident
                );
            }
            TraitItem::Method(method) => {
                let mut is_ok = true;
                let mut async_trait_life_time = 0;
                let mut life0_life_time = 0;
                if let Some(wher) = &method.sig.generics.where_clause {
                    for w in wher.predicates.iter() {
                        match w {
                            WherePredicate::Type(t) => {
                                match &t.bounded_ty {
                                    Type::Path(p) => {
                                        if p.path.segments.len() == 1 {
                                            if p.path.segments[0].ident != "Self" {
                                                is_ok = false;
                                            }
                                        } else {
                                            is_ok = false;
                                        }
                                    }
                                    _ => {
                                        is_ok = false;
                                        break;
                                    }
                                }
                                if let Some(l) = &t.lifetimes {
                                    is_ok = false;
                                }
                                for bound in &t.bounds {
                                    match bound {
                                        TypeParamBound::Trait(t) => {
                                            if t.path.segments.len() == 1 {
                                                if t.path.segments[0].ident != "Sync" {
                                                    is_ok = false;
                                                }
                                            } else {
                                                is_ok = false;
                                            }
                                        }
                                        TypeParamBound::Lifetime(l) => {
                                            if l.ident != "async_trait" {
                                                is_ok = false;
                                            }
                                        }
                                    }
                                }
                            }
                            WherePredicate::Lifetime(l) => {
                                if l.lifetime.ident != "life0" {
                                    if !is_life(&l.lifetime) {
                                        is_ok = false;
                                    }
                                } else {
                                    life0_life_time += 1;
                                }
                                for bound in &l.bounds {
                                    if bound.ident != "async_trait" {
                                        is_ok = false;
                                    } else {
                                        async_trait_life_time += 1;
                                    }
                                }
                            }
                            WherePredicate::Eq(_) => {
                                is_ok = false;
                            }
                        }
                    }
                    if !is_ok {
                        abort!(
                            method.sig.generics.where_clause.span(),
                            "Savefile does not support where-clauses for methods"
                        );
                    }
                }

                let method_name = method.sig.ident.clone();

                let mut current_name_index = 0u32;
                let name_baseplate = format!("Temp{}_{}", trait_name_str, method_name);
                let mut temp_name_generator = move || {
                    current_name_index += 1;
                    format!("{}_{}", name_baseplate, current_name_index)
                };
                let mut receiver_is_mut = false;
                let mut receiver_is_pin = false;
                let ret_type: Type;
                let ret_declaration;
                let no_return;

                match &method.sig.output {
                    ReturnType::Default => {
                        ret_type = Tuple(TypeTuple {
                            paren_token: Paren::default(),
                            elems: Default::default(),
                        });
                        ret_declaration = quote! {};
                        no_return = true;
                    }
                    ReturnType::Type(_, ty) => {
                        ret_type = (**ty).clone();
                        match &**ty {
                            Type::Tuple(tup) if tup.elems.is_empty() => {
                                ret_declaration = quote! {};
                                no_return = true;
                            }
                            _ => {
                                ret_declaration = quote! { -> #ret_type };
                                no_return = false;
                            }
                        }
                    }
                }

                let self_arg = method.sig.inputs.iter().next().unwrap_or_else(|| {
                    abort!(
                        method.span(),
                        "Method '{}' has no arguments. This is not supported by savefile-abi - it must at least have a self-argument.",
                        method_name
                    )
                });
                match self_arg {
                    FnArg::Receiver(recv) => {
                        if let Some(reference) = &recv.reference {
                            if let Some(reference) = &reference.1 {
                                if reference.ident != "life0" {
                                    abort!(
                                        reference.span(),
                                        "Method '{}' has a lifetime \"'{}\" for 'self' argument. This is not supported by savefile-abi",
                                        method_name,
                                        reference.ident,
                                    );
                                } else {
                                    life0_life_time += 1;
                                }
                            }
                            if recv.mutability.is_some() {
                                receiver_is_mut = true;
                            }
                        } else {
                            abort!(
                                self_arg.span(),
                                "Method '{}' takes 'self' by value. This is not supported by savefile-abi. Use &self",
                                method_name
                            );
                        }
                    }
                    FnArg::Typed(pat) => {
                        let unsupported = || {
                            abort!(
                                        method.sig.span(),
                                        "Method '{}' has an unsupported 'self'-parameter. Try '&self', '&mut self', or 'self: Pin<&mut Self>'. Not supported: {}",
                                        method_name, self_arg.to_token_stream()
                                    );
                        };
                        match &*pat.pat {
                            Pat::Ident(ident) if ident.ident == "self" => {
                                if ident.by_ref.is_some() || ident.mutability.is_some() {
                                    unsupported();
                                }
                                if let Type::Path(path) = &*pat.ty {
                                    if !is_well_known(&path.path.segments, ["std", "pin", "Pin"]) {
                                        unsupported();
                                    }
                                    let seg = &path.path.segments.last().unwrap();
                                    let PathArguments::AngleBracketed(args) = &seg.arguments else {
                                        unsupported();
                                        unreachable!();
                                    };
                                    if args.args.len() != 1 {
                                        unsupported();
                                    }
                                    let arg = &args.args[0];
                                    let GenericArgument::Type(Type::Reference(typref)) = arg else {
                                        unsupported();
                                        unreachable!();
                                    };
                                    if typref.mutability.is_none() {
                                        abort!(
                                            method.sig.span(),
                                            "Method '{}' has an unsupported 'self'-parameter. Non-mutable references in Pin are presently not supported: {}",
                                            method_name, self_arg.to_token_stream()
                                        );
                                    }
                                    let Type::Path(typepath) = &*typref.elem else {
                                        unsupported();
                                        unreachable!();
                                    };
                                    if typepath.path.segments.len() != 1 {
                                        unsupported();
                                        unreachable!()
                                    };
                                    if typepath.path.segments[0].ident != "Self" {
                                        unsupported();
                                    }
                                    receiver_is_mut = true;
                                    receiver_is_pin = true;
                                } else {
                                    unsupported();
                                }
                            }
                            _ => {
                                abort!(
                                        pat.pat.span(),
                                        "Method '{}' must have 'self'-parameter (savefile-abi does not support methods without self)",
                                        method_name
                                    );
                            }
                        }
                    }
                }
                let mut args = Vec::with_capacity(method.sig.inputs.len());
                for arg in method.sig.inputs.iter().skip(1) {
                    match arg {
                        FnArg::Typed(typ) => {
                            match &*typ.pat {
                                Pat::Ident(name) => {
                                    args.push((name.ident.clone(), &*typ.ty));
                                }
                                _ => abort!(typ.pat.span(), "Method '{}' has a parameter which contains a complex pattern. This is not supported by savefile-abi.", method_name)
                            }
                        },
                        _ => abort!(arg.span(), "Unexpected error: method {} had a self parameter that wasn't the first parameter!", method_name)
                    }
                }
                if method.sig.asyncness.is_some() {
                    let out = match &method.sig.output {
                        ReturnType::Default => {
                            quote! {()}
                        }
                        ReturnType::Type(_, t) => t.to_token_stream(),
                    };
                    abort!(
                        method.sig.asyncness.span(),
                        "savefile-abi does not support async methods. You can try returning a boxed future instead: Pin<Box<Future<Output={}>>>",
                        out
                    )
                }
                if method.sig.variadic.is_some() {
                    abort!(
                        method.sig.variadic.span(),
                        "savefile-abi does not support variadic methods."
                    )
                }
                if method.sig.unsafety.is_some() {
                    abort!(
                        method.sig.unsafety.span(),
                        "savefile-abi does not presently support unsafe methods."
                    )
                }
                if method.sig.abi.is_some() {
                    abort!(method.sig.abi.span(), "savefile-abi does not need (or support) 'extern \"C\"' or similar ABI-constructs. Just remove this keyword.")
                }

                // NOTE!
                // This is part of the heuristics that detects async_trait macro output.
                // However, we don't actually support reference arguments to functions
                // returning futures.
                fn is_life(id: &syn::Lifetime) -> bool {
                    let s = id.ident.to_string();
                    if !s.starts_with("life") {
                        return false;
                    }
                    s.strip_prefix("life").unwrap().parse::<usize>().is_ok()
                }

                if method.sig.generics.params.is_empty() == false {
                    for item in method.sig.generics.params.iter() {
                        match item {
                            GenericParam::Type(typ) => {
                                abort!(typ.span(), "savefile-abi does not support generic methods.")
                            }
                            GenericParam::Const(typ) => {
                                abort!(typ.span(), "savefile-abi does not support const-generic methods.")
                            }
                            GenericParam::Lifetime(l) => {
                                if l.lifetime.ident != "life0"
                                    && l.lifetime.ident != "async_trait"
                                    && !is_life(&l.lifetime)
                                {
                                    abort!(
                                        method.sig.generics.params.span(),
                                        "savefile-abi does not support methods with lifetimes."
                                    );
                                } else {
                                    if l.lifetime.ident == "life0" {
                                        life0_life_time += 1;
                                    }
                                    async_trait_life_time += 1;
                                }
                            }
                        }
                    }
                }

                let async_trait_macro_detected;
                if life0_life_time >= 3 && async_trait_life_time >= 3 {
                    async_trait_macro_detected = true;
                } else if life0_life_time == 0 && async_trait_life_time == 0 {
                    async_trait_macro_detected = false;
                } else {
                    abort!(
                        item.span(),
                        "savefile-abi has heuristics that detects the use of the #[async_trait]-macro. This heuristic produced a partial result. It is possible that an incompatible version of async_trait crate has been used. Diagnostics: {} {}",
                        life0_life_time, async_trait_life_time
                    );
                }

                let method_defs = crate::savefile_abi::generate_method_definitions(
                    version,
                    trait_name.clone(),
                    method_number,
                    method_name,
                    ret_declaration,
                    ret_type,
                    no_return,
                    receiver_is_mut,
                    receiver_is_pin,
                    args,
                    &mut temp_name_generator,
                    &mut extra_definitions,
                    async_trait_macro_detected,
                );
                method_metadata.push(method_defs.method_metadata);
                callee_method_trampoline.push(method_defs.callee_method_trampoline);
                caller_method_trampoline.push(method_defs.caller_method_trampoline);
            }
            TraitItem::Type(t) => {
                abort!(
                    t.span(),
                    "savefile_abi_exportable does not support associated types: {}",
                    t.ident
                );
            }
            TraitItem::Macro(m) => {
                abort!(
                    m.span(),
                    "savefile_abi_exportable does not support macro items: {:?}",
                    m
                );
            }
            TraitItem::Verbatim(v) => {
                abort!(
                    v.span(),
                    "Unsupported item in trait definition: {}",
                    v.to_token_stream()
                );
            }
            x => abort!(
                x.span(),
                "Unsupported item in trait definition: {}",
                x.to_token_stream()
            ),
        }
    }

    let abi_entry_light = Ident::new(&format!("abi_entry_light_{}", trait_name_str), Span::call_site());

    let exports_for_trait = quote! {

        unsafe extern "C" fn #abi_entry_light(flag: AbiProtocol) {
            unsafe { abi_entry_light::<dyn #trait_name>(flag); }
        }

        #[automatically_derived]
        unsafe impl AbiExportable for dyn #trait_name {
            const ABI_ENTRY : unsafe extern "C" fn (flag: AbiProtocol)  = #abi_entry_light;
            fn get_definition( version: u32) -> AbiTraitDefinition {
                AbiTraitDefinition {
                    name: #trait_name_str.to_string(),
                    methods: vec! [ #(#method_metadata,)* ],
                    sync: #sync,
                    send: #send
                }
            }

            fn get_latest_version() -> u32 {
                #version
            }

            fn call(trait_object: TraitObject, method_number: u16, effective_version:u32, compatibility_mask: u64, data: &[u8], abi_result: *mut (), __savefile_internal_receiver: unsafe extern "C" fn(outcome: *const RawAbiCallResult, result_receiver: *mut ()/*Result<T,SaveFileError>>*/)) -> Result<(),SavefileError> {

                let mut cursor = Cursor::new(data);

                let mut deserializer = Deserializer {
                    file_version: cursor.read_u32::<LittleEndian>()?,
                    reader: &mut cursor,
                    ephemeral_state: HashMap::new(),
                };

                match method_number {
                    #(#callee_method_trampoline,)*
                    _ => {
                        return Err(SavefileError::general("Unknown method number"));
                    }
                }
                Ok(())
            }
        }

        #[automatically_derived]
        impl #trait_name for AbiConnection<dyn #trait_name> {
            #(#caller_method_trampoline)*
        }
    };

    //let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());
    let input = TokenStream::from(input);
    let extra_definitions: Vec<_> = extra_definitions.values().map(|(_, x)| x).collect();
    let assert_tight = assert_tight();
    let expanded = quote! {
        #[allow(clippy::double_comparisons)]
        #[allow(clippy::needless_question_mark)]
        #[allow(unused_variables)]
        #[allow(clippy::needless_late_init)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[allow(non_upper_case_globals)]
        #[allow(clippy::manual_range_contains)]
        #[allow(non_local_definitions)]
        const _:() = {
            #uses

            #assert_tight

            #(#extra_definitions)*

            #exports_for_trait

        };

        #input
    };

    // For debugging, uncomment to write expanded procmacro to file
    // std::fs::write(format!("/home/anders/savefile/savefile-min-build/src/{}.rs",trait_name_str),expanded.to_string()).unwrap();

    expanded.into()
}
#[proc_macro_error]
#[proc_macro]
pub fn savefile_abi_export(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let tokens = proc_macro2::TokenStream::from(item);

    let mut tokens_iter = tokens.into_iter();

    let Some(implementing_type) = tokens_iter.next() else {
        abort!(Span::call_site(), "The macro savefile_abi_export! requires two parameters. The first parameter must be the implementing type, the second is the trait it implements.");
    };
    let Some(comma) = tokens_iter.next() else {
        abort!(Span::call_site(), "The macro savefile_abi_export! requires two parameters. The first parameter must be the implementing type, the second is the trait it implements.");
    };
    if let TokenTree::Punct(p) = comma {
        if p.as_char() != ',' {
            abort!(p.span(), "Expected a comma (','). The macro savefile_abi_export! requires two parameters. The first parameter must be the implementing type, the second is the trait it implements, and these must be separated by a comma.");
        }
    } else {
        abort!(comma.span(), "Expected a comma (','). The macro savefile_abi_export! requires two parameters. The first parameter must be the implementing type, the second is the trait it implements, and these must be separated by a comma.");
    }
    let Some(trait_type) = tokens_iter.next() else {
        abort!(Span::call_site(), "The macro savefile_abi_export! requires two parameters. The first parameter must be the implementing type, the second is the trait it implements. Expected trait name.");
    };

    if let Some(extra) = tokens_iter.next() {
        abort!(extra.span(), "Unexpected token. The macro savefile_abi_export! requires exactly two parameters. The first parameter must be the implementing type, the second is the trait it implements.");
    }

    let defspan = Span::call_site();
    let uses = quote_spanned! { defspan =>
        extern crate savefile_abi;
        use savefile_abi::{AbiProtocol, AbiExportableImplementation, abi_entry,parse_return_value_impl};
    };

    let abi_entry = Ident::new(
        ("abi_entry_".to_string() + &trait_type.to_string()).as_str(),
        Span::call_site(),
    );

    let assert_tight = assert_tight();
    let expanded = quote! {
        #[allow(clippy::needless_question_mark)]
        #[allow(clippy::double_comparisons)]
        #[allow(non_local_definitions)]
        const _:() = {
            #uses

            #assert_tight

            #[automatically_derived]
            unsafe impl AbiExportableImplementation for #implementing_type where #implementing_type: Default + #trait_type {
                const ABI_ENTRY: unsafe extern "C" fn (AbiProtocol) = #abi_entry;
                type AbiInterface = dyn #trait_type;

                fn new() -> Box<Self::AbiInterface> {
                    std::boxed::Box::new(#implementing_type::default())
                }
            }
            #[no_mangle]
            unsafe extern "C" fn #abi_entry(flag: AbiProtocol) where #implementing_type: Default + #trait_type {
                unsafe { abi_entry::<#implementing_type>(flag); }
            }
        };
    };

    expanded.into()
}

#[proc_macro_error]
#[proc_macro_derive(
    Savefile,
    attributes(
        savefile_unsafe_and_fast,
        savefile_require_fast,
        savefile_versions,
        savefile_versions_as,
        savefile_introspect_ignore,
        savefile_introspect_key,
        savefile_ignore,
        savefile_default_val,
        savefile_default_fn
    )
)]
pub fn savefile(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).expect("Expected valid rust code [Savefile]");

    let s = serialize::savefile_derive_crate_serialize(input.clone());

    let d = deserialize::savefile_derive_crate_deserialize(input.clone());

    let w = savefile_derive_crate_withschema(input.clone());

    let i = savefile_derive_crate_introspect(input.clone());

    let r = derive_reprc_new(input);

    let assert_tight = assert_tight();

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    let expanded = quote! {
        #s

        #d

        #i

        #[allow(non_upper_case_globals)]
        #[allow(clippy::double_comparisons)]
        #[allow(clippy::manual_range_contains)]
        const #dummy_const: () = {
            extern crate savefile as _savefile;
            use std::mem::MaybeUninit;
            use savefile::prelude::Packed;
            #assert_tight

            #w
            #r
        };

    };
    //std::fs::write("/home/anders/savefile/savefile-min-build/src/expanded.rs", expanded.to_string()).unwrap();

    expanded.into()
}
#[proc_macro_error]
#[proc_macro_derive(
    SavefileNoIntrospect,
    attributes(
        savefile_unsafe_and_fast,
        savefile_require_fast,
        savefile_versions,
        savefile_versions_as,
        savefile_ignore,
        savefile_introspect_ignore,
        savefile_default_val,
        savefile_default_fn
    )
)]
pub fn savefile_no_introspect(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).expect("Expected valid rust code [SavefileNoIntrospect]");

    let s = serialize::savefile_derive_crate_serialize(input.clone());

    let d = deserialize::savefile_derive_crate_deserialize(input.clone());

    let w = savefile_derive_crate_withschema(input.clone());

    let r = derive_reprc_new(input);

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    let assert_tight = assert_tight();

    let expanded = quote! {
        #s

        #d

        #[allow(non_upper_case_globals)]
        #[allow(clippy::double_comparisons)]
        #[allow(clippy::manual_range_contains)]
        const #dummy_const: () = {
            extern crate savefile as _savefile;
            use std::mem::MaybeUninit;
            use savefile::prelude::Packed;

            #assert_tight

            #w
            #r
        };
    };

    expanded.into()
}

#[proc_macro_error]
#[proc_macro_derive(
    SavefileIntrospectOnly,
    attributes(
        savefile_versions,
        savefile_versions_as,
        savefile_introspect_ignore,
        savefile_ignore,
        savefile_default_val,
        savefile_default_fn
    )
)]
pub fn savefile_introspect_only(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).expect("Expected valid rust code [SavefileIntrospectOnly]");

    let i = savefile_derive_crate_introspect(input);

    let expanded = quote! {
        #i
    };

    expanded.into()
}

#[allow(non_snake_case)]
fn implement_reprc_hardcoded_false(name: syn::Ident, generics: syn::Generics) -> TokenStream {
    let defspan = proc_macro2::Span::call_site();

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause, quote! {_savefile::prelude::WithSchema});
    let reprc = quote_spanned! {defspan=>
        _savefile::prelude::Packed
    };
    let isreprc = quote_spanned! {defspan=>
        _savefile::prelude::IsPacked
    };
    quote! {

        #[automatically_derived]
        impl #impl_generics #reprc for #name #ty_generics #where_clause #extra_where {
            #[allow(unused_comparisons,unused_variables, unused_variables)]
            unsafe fn repr_c_optimization_safe(file_version:u32) -> #isreprc {
                #isreprc::no()
            }
        }

    }
}

#[allow(non_snake_case)]
fn implement_reprc_struct(
    field_infos: Vec<FieldInfo>,
    generics: syn::Generics,
    name: syn::Ident,
    expect_fast: bool,
) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause, quote! {_savefile::prelude::Packed});

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let reprc = quote_spanned! {defspan=>
        _savefile::prelude::Packed
    };
    let isreprc = quote_spanned! {defspan=>
        _savefile::prelude::IsPacked
    };
    let offsetof = quote_spanned! {defspan=>
        _savefile::prelude::offset_of
    };
    let local_file_version = quote_spanned! { defspan => local_file_version};
    //let WithSchema = quote_spanned! { defspan => _savefile::prelude::WithSchema};
    let mut min_safe_version = 0;
    let mut packed_outputs = Vec::new();
    let mut reprc_outputs = Vec::new();

    for field in field_infos.windows(2) {
        let field_name1 = field[0].get_accessor();
        let field_name2 = field[1].get_accessor();
        let ty = field[0].ty;
        packed_outputs.push(quote!( (#offsetof!(#name #ty_generics, #field_name1) + std::mem::size_of::<#ty>() == #offsetof!(#name #ty_generics, #field_name2) )));
    }
    if field_infos.len() > 0 {
        if field_infos.len() == 1 {
            let ty = field_infos[0].ty;
            let field_name = field_infos[0].get_accessor();
            packed_outputs.push(quote!(  (#offsetof!( #name #ty_generics, #field_name) == 0 )));
            packed_outputs.push(quote!(  (#offsetof!( #name #ty_generics, #field_name) + std::mem::size_of::<#ty>() == std::mem::size_of::<#name #ty_generics>() )));
        } else {
            let first = field_infos.first().expect("field_infos.first()[2]").get_accessor();
            let last_field = field_infos.last().expect("field_infos.last()[2]");
            let last = last_field.get_accessor();
            let last_ty = &last_field.ty;
            packed_outputs.push(quote!( (#offsetof!(#name #ty_generics, #first) == 0 )));
            packed_outputs.push(quote!( (#offsetof!(#name #ty_generics, #last) + std::mem::size_of::<#last_ty>()  == std::mem::size_of::<#name #ty_generics>() )));
        }
    }

    for field in &field_infos {
        let verinfo = parse_attr_tag(field.attrs);
        if verinfo.ignore {
            if expect_fast {
                abort!(
                    field.field_span,
                    "The #[savefile_require_fast] attribute cannot be used for structures containing ignored fields"
                );
            } else {
                return implement_reprc_hardcoded_false(name, generics);
            }
        }
        let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

        let removed = check_is_remove(field.ty);
        let field_type = &field.ty;
        if field_from_version == 0 && field_to_version == std::u32::MAX {
            if removed.is_removed() {
                if expect_fast {
                    abort!(field.ty.span(), "The Removed type can only be used for removed fields. Use the savefile_version attribute to mark a field as only existing in previous versions.");
                } else {
                    return implement_reprc_hardcoded_false(name, generics);
                }
            }
            reprc_outputs
                .push(quote_spanned!( span => <#field_type as #reprc>::repr_c_optimization_safe(#local_file_version).is_yes()));
        } else {
            min_safe_version = min_safe_version.max(verinfo.min_safe_version());

            if !removed.is_removed() {
                reprc_outputs.push(
                    quote_spanned!( span => <#field_type as #reprc>::repr_c_optimization_safe(#local_file_version).is_yes()),
                );
            }
        }
    }

    let require_packed = if expect_fast {
        quote!(
            const _: () = {
                if !PACKED {
                    panic!("Memory layout not optimal - requires padding which disables savefile-optimization");
                }
            };
        )
    } else {
        quote!()
    };

    let packed_storage = if generics.params.is_empty() == false {
        quote!(let)
    } else {
        quote!(const)
    };

    quote! {

        #[automatically_derived]
        impl #impl_generics #reprc for #name #ty_generics #where_clause #extra_where {
            #[allow(unused_comparisons,unused_variables, unused_variables)]
            unsafe fn repr_c_optimization_safe(file_version:u32) -> #isreprc {
                let local_file_version = file_version;
                #packed_storage PACKED : bool = true #( && #packed_outputs)*;
                #require_packed
                if file_version >= #min_safe_version && PACKED #( && #reprc_outputs)*{
                    unsafe { #isreprc::yes() }
                } else {
                    #isreprc::no()
                }
            }
        }
    }
}

#[derive(Debug)]
struct EnumSize {
    discriminant_size: u8,
    #[allow(unused)] //Keep around, useful for debugging
    repr_c: bool,
    explicit_size: bool,
}

fn get_enum_size(attrs: &[syn::Attribute], actual_variants: usize) -> EnumSize {
    let mut size_u8: Option<u8> = None;
    let mut repr_c_seen = false;
    let mut have_seen_explicit_size = false;
    for attr in attrs.iter() {
        if let Ok(ref meta) = attr.parse_meta() {
            match meta {
                &syn::Meta::NameValue(ref _x) => {}
                &syn::Meta::Path(ref _x) => {}
                &syn::Meta::List(ref metalist) => {
                    let path = path_to_string(&metalist.path);
                    if path == "repr" {
                        for x in &metalist.nested {
                            let size_str: String = match *x {
                                syn::NestedMeta::Meta(ref inner_x) => match inner_x {
                                    &syn::Meta::NameValue(ref _x) => {
                                        continue;
                                    }
                                    &syn::Meta::Path(ref path) => path_to_string(path),
                                    &syn::Meta::List(ref _metalist) => {
                                        continue;
                                    }
                                },
                                syn::NestedMeta::Lit(ref lit) => match lit {
                                    &syn::Lit::Str(ref litstr) => litstr.value(),
                                    _ => {
                                        continue;
                                        //panic!("Unsupported repr-attribute: repr({:?})", x.clone().into_token_stream());
                                    }
                                },
                            };
                            match size_str.as_ref() {
                                "C" => repr_c_seen = true,
                                "u8" => {
                                    size_u8 = Some(1);
                                    have_seen_explicit_size = true;
                                }
                                "i8" => {
                                    size_u8 = Some(1);
                                    have_seen_explicit_size = true;
                                }
                                "u16" => {
                                    size_u8 = Some(2);
                                    have_seen_explicit_size = true;
                                }
                                "i16" => {
                                    size_u8 = Some(2);
                                    have_seen_explicit_size = true;
                                }
                                "u32" => {
                                    size_u8 = Some(4);
                                    have_seen_explicit_size = true;
                                }
                                "i32" => {
                                    size_u8 = Some(4);
                                    have_seen_explicit_size = true;
                                }
                                "u64" | "i64" => {
                                    abort!(
                                        metalist.path.span(),
                                        "Savefile does not support enums with more than 2^32 variants."
                                    )
                                }
                                _ => abort!(
                                    metalist.path.span(),
                                    "Unsupported repr(X) attribute on enum: {}",
                                    size_str
                                ),
                            }
                        }
                    }
                }
            }
        }
    }
    let discriminant_size = size_u8.unwrap_or_else(|| {
        if actual_variants <= 256 {
            1
        } else if actual_variants <= 65536 {
            2
        } else {
            if actual_variants >= u32::MAX as usize {
                abort_call_site!("The enum had an unreasonable number of variants");
            }
            4
        }
    });
    EnumSize {
        discriminant_size,
        repr_c: repr_c_seen,
        explicit_size: have_seen_explicit_size,
    }
}
#[proc_macro_error]
#[proc_macro_derive(
    Packed,
    attributes(
        savefile_versions,
        savefile_versions_as,
        savefile_ignore,
        savefile_default_val,
        savefile_default_fn
    )
)]
pub fn reprc(_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    abort_call_site!("The #[derive(Packed)] style of unsafe performance opt-in has been removed. The performance gains are now available automatically for any packed struct.")
}
fn derive_reprc_new(input: DeriveInput) -> TokenStream {
    let name = input.ident;
    let (impl_generics, ty_generics, _where_clause) = input.generics.split_for_impl();

    let mut opt_in_fast = false;
    for attr in input.attrs.iter() {
        match attr.parse_meta() {
            Ok(ref meta) => match meta {
                &syn::Meta::Path(ref x) => {
                    let x = path_to_string(x);
                    if x == "savefile_unsafe_and_fast" || x == "savefile_require_fast" {
                        opt_in_fast = true;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    /*if !opt_in_fast {
        return implement_reprc_hardcoded_false(name, input.generics);
    }*/

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let enum_size = get_enum_size(&input.attrs, enum1.variants.len());
            let any_fields = enum1.variants.iter().any(|v| v.fields.len() > 0);
            if !enum_size.explicit_size {
                if opt_in_fast {
                    if any_fields {
                        abort_call_site!("The #[savefile_require_fast] requires an explicit #[repr(u8),C],#[repr(u16,C)] or #[repr(u32,C)], attribute.");
                    } else {
                        abort_call_site!("The #[savefile_require_fast] requires an explicit #[repr(u8)],#[repr(u16)] or #[repr(u32)], attribute.");
                    }
                }
                return implement_reprc_hardcoded_false(name, input.generics);
            }

            let mut conditions = vec![];

            let mut min_safe_version: u32 = 0;

            let mut unique_field_types = HashSet::new();

            let fn_impl_generics = if !input.generics.params.is_empty() {
                quote! { :: #impl_generics}
            } else {
                quote! {}
            };
            for (variant_index, variant) in enum1.variants.iter().enumerate() {
                let mut attrs: Vec<_> = vec![];

                let mut num_fields = 0usize;
                let mut field_types = vec![];
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        for field in fields_named.named.iter() {
                            attrs.push(&field.attrs);
                            field_types.push(&field.ty);
                            num_fields += 1;
                        }
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        for field in fields_unnamed.unnamed.iter() {
                            attrs.push(&field.attrs);
                            field_types.push(&field.ty);
                            num_fields += 1;
                        }
                    }
                    &syn::Fields::Unit => {}
                }
                for i in 0usize..num_fields {
                    let verinfo = parse_attr_tag(&attrs[i]);
                    if check_is_remove(&field_types[i]).is_removed() {
                        if verinfo.version_to == u32::MAX {
                            abort!(field_types[i].span(), "Removed fields must have a max version, provide one using #[savefile_versions=\"..N\"]")
                        }
                        min_safe_version = min_safe_version.max(verinfo.version_to + 1);
                    }
                    let typ = field_types[i].to_token_stream();

                    let variant_index = proc_macro2::Literal::u32_unsuffixed(variant_index as u32);

                    unique_field_types.insert(field_types[i].clone());
                    if i == 0 {
                        let discriminant_bytes = enum_size.discriminant_size as usize;
                        conditions.push( quote!( (#discriminant_bytes == (get_variant_offsets #fn_impl_generics(#variant_index)[#i])) ) );
                    }
                    if i == num_fields - 1 {
                        conditions.push(
                            quote!(  (std::mem::size_of::<#name #ty_generics>() == (get_variant_offsets #fn_impl_generics(#variant_index)[#i]) + std::mem::size_of::<#typ>())  )
                        );
                    } else {
                        let n = i + 1;
                        let end_offset_condition = quote!(  (get_variant_offsets #fn_impl_generics(#variant_index)[#n] == (get_variant_offsets #fn_impl_generics(#variant_index)[#i]) + std::mem::size_of::<#typ>())  );
                        conditions.push(quote!(#end_offset_condition));
                    };
                }

                for attr in attrs {
                    let verinfo = parse_attr_tag(attr);
                    if verinfo.ignore {
                        if opt_in_fast {
                            abort_call_site!(
                                "The #[savefile_require_fast] attribute cannot be used for structures containing ignored fields"
                            );
                        } else {
                            return implement_reprc_hardcoded_false(name, input.generics);
                        }
                    }
                    min_safe_version = min_safe_version.max(verinfo.min_safe_version());
                }
            }

            let defspan = proc_macro2::Span::call_site();
            let generics = input.generics;
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            let extra_where = get_extra_where_clauses(&generics, where_clause, quote! {_savefile::prelude::Packed});
            let reprc = quote_spanned! { defspan=>
                _savefile::prelude::Packed
            };
            let isreprc = quote_spanned! {defspan=>
                _savefile::prelude::IsPacked
            };

            if conditions.is_empty() {
                conditions.push(quote!(true));
            }
            let require_packed = if opt_in_fast {
                quote!(
                    const _: () = {
                        if !PACKED {
                            panic!("Memory layout not optimal - requires padding which disables savefile-optimization");
                        }
                    };
                )
            } else {
                quote!()
            };
            let mut reprc_condition = vec![];
            for typ in unique_field_types {
                reprc_condition.push(quote!(
                    <#typ as Packed>::repr_c_optimization_safe(file_version).is_yes()
                ));
            }

            let packed_decl = if generics.params.is_empty() {
                quote! { const }
            } else {
                quote! { let }
            };
            let packed_constraints = if any_fields {
                quote!(
                    #packed_decl PACKED : bool = true #( && #conditions)*;
                    #require_packed
                    if !PACKED {
                        return #isreprc::no();
                    }
                )
            } else {
                quote!()
            };

            return quote! {

                #[automatically_derived]
                impl #impl_generics #reprc for #name #ty_generics #where_clause #extra_where {
                    #[allow(unused_comparisons,unused_variables, unused_variables)]
                    unsafe fn repr_c_optimization_safe(file_version:u32) -> #isreprc {
                        let local_file_version = file_version;

                        #packed_constraints

                        if file_version >= #min_safe_version #( && #reprc_condition)* {
                            unsafe { #isreprc::yes() }
                        } else {
                            #isreprc::no()
                        }
                    }
                }
            };

            //implement_reprc_struct(vec![], input.generics, name, opt_in_fast) //Hacky, consider enum without any fields as a field-less struct
        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {
                let field_infos: Vec<FieldInfo> = namedfields
                    .named
                    .iter()
                    .enumerate()
                    .map(|(field_index, field)| FieldInfo {
                        ident: Some(field.ident.clone().expect("Expected identifier [8]")),
                        field_span: field.ident.as_ref().unwrap().span(),
                        index: field_index as u32,
                        ty: &field.ty,
                        attrs: &field.attrs,
                    })
                    .collect();

                implement_reprc_struct(field_infos, input.generics, name, opt_in_fast)
            }
            &syn::Fields::Unnamed(ref fields_unnamed) => {
                let field_infos: Vec<FieldInfo> = fields_unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| FieldInfo {
                        field_span: field.ty.span(),
                        ident: None,
                        index: idx as u32,
                        ty: &field.ty,
                        attrs: &field.attrs,
                    })
                    .collect();

                implement_reprc_struct(field_infos, input.generics, name, opt_in_fast)
            }
            &syn::Fields::Unit => implement_reprc_struct(Vec::new(), input.generics, name, opt_in_fast),
        },
        _ => {
            if opt_in_fast {
                abort_call_site!("Unsupported data type");
            }
            return implement_reprc_hardcoded_false(name, input.generics);
        }
    };

    expanded
}

#[allow(non_snake_case)]
fn implement_introspect(
    field_infos: Vec<FieldInfo>,
    need_self: bool,
) -> (Vec<TokenStream>, Vec<TokenStream>, Option<TokenStream>) {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();

    //let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    //let Introspect = quote_spanned! { defspan => _savefile::prelude::Introspect };
    //let fields1=quote_spanned! { defspan => fields1 };
    let index1 = quote_spanned! { defspan => index };
    let introspect_item = quote_spanned! { defspan=>
        _savefile::prelude::introspect_item
    };

    let mut fields = Vec::new();
    let mut fields_names = Vec::new();
    let mut introspect_key = None;
    let mut index_number = 0usize;
    for (idx, field) in field_infos.iter().enumerate() {
        let verinfo = parse_attr_tag(field.attrs);
        if verinfo.introspect_key && introspect_key.is_some() {
            abort!(
                field.field_span,
                "Type had more than one field with savefile_introspect_key - attribute"
            );
        }
        if verinfo.introspect_ignore {
            continue;
        }
        if need_self {
            let fieldname;
            let fieldname_raw;

            let id = field.get_accessor();
            fieldname = quote! {&self.#id};
            fieldname_raw = quote! {#id};

            fields.push(quote_spanned!( span => if #index1 == #index_number { return Some(#introspect_item(stringify!(#fieldname_raw).to_string(), #fieldname))}));
            if verinfo.introspect_key {
                let fieldname_raw2 = fieldname_raw.clone();
                introspect_key = Some(quote! {self.#fieldname_raw2});
            }
            fields_names.push(fieldname_raw);
        } else if let Some(id) = field.ident.clone() {
            let fieldname;
            let quoted_fieldname;
            let raw_fieldname = id.to_string();
            let id2 = id.clone();
            fieldname = id;
            quoted_fieldname = quote! { #fieldname };
            fields.push(quote_spanned!( span => if #index1 == #index_number { return Some(#introspect_item(#raw_fieldname.to_string(), #quoted_fieldname))}));
            fields_names.push(quoted_fieldname);
            if verinfo.introspect_key {
                introspect_key = Some(quote!(#id2))
            }
        } else {
            let fieldname;
            let quoted_fieldname;
            let raw_fieldname = idx.to_string();
            fieldname = Ident::new(&format!("v{}", idx), span);
            let fieldname2 = fieldname.clone();
            quoted_fieldname = quote! { #fieldname };
            fields.push(quote_spanned!( span => if #index1 == #index_number { return Some(#introspect_item(#raw_fieldname.to_string(), #quoted_fieldname))}));
            fields_names.push(quoted_fieldname);
            if verinfo.introspect_key {
                introspect_key = Some(quote!(#fieldname2))
            }
        }

        index_number += 1;
    }

    (fields_names, fields, introspect_key)
}

#[allow(non_snake_case)]
fn savefile_derive_crate_introspect(input: DeriveInput) -> TokenStream {
    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause, quote! {_savefile::prelude::Introspect});

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let introspect = quote_spanned! {defspan=>
        _savefile::prelude::Introspect
    };
    let introspect_item_type = quote_spanned! {defspan=>
        _savefile::prelude::IntrospectItem
    };
    let uses = quote_spanned! { defspan =>
        extern crate savefile as _savefile;
    };

    //let SchemaStruct = quote_spanned! { defspan => _savefile::prelude::SchemaStruct };
    //let SchemaEnum = quote_spanned! { defspan => _savefile::prelude::SchemaEnum };
    //let Schema = quote_spanned! { defspan => _savefile::prelude::Schema };
    //let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    //let Variant = quote_spanned! { defspan => _savefile::prelude::Variant };

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut variants = Vec::new();
            let mut value_variants = Vec::new();
            let mut len_variants = Vec::new();
            for variant in enum1.variants.iter() {
                /*if var_idx >= 256 {
                    panic!("Savefile does not support enums with 256 variants or more. Sorry.");
                }*/
                //let var_idx = var_idx as u8;
                let var_ident = variant.ident.clone();
                let variant_name = quote! { #var_ident };
                let variant_name_spanned = quote_spanned! { span => #variant_name};

                let mut field_infos = Vec::new();

                let return_value_name_str = format!("{}::{}", name, var_ident);
                let return_value_name = quote!(#return_value_name_str);
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        for (idx, f) in fields_named.named.iter().enumerate() {
                            field_infos.push(FieldInfo {
                                ident: Some(f.ident.clone().expect("Expected identifier[9]")),
                                field_span: f.ident.as_ref().unwrap().span(),
                                index: idx as u32,
                                ty: &f.ty,
                                attrs: &f.attrs,
                            });
                        }
                        let (fields_names, fields, introspect_key) = implement_introspect(field_infos, false);
                        let fields_names1 = fields_names.clone();
                        let fields_names2 = fields_names.clone();
                        let fields_names3 = fields_names.clone();
                        let num_fields = fields_names3.len();
                        if let Some(introspect_key) = introspect_key {
                            value_variants.push(quote!(#name::#variant_name_spanned{#(#fields_names,)*} => {
                                #introspect_key.to_string()
                            }
                            ));
                        } else {
                            value_variants.push(quote!( #name::#variant_name_spanned{#(#fields_names2,)*} => {
                                #return_value_name.to_string()
                            } ));
                        }
                        variants.push(quote!( #name::#variant_name_spanned{#(#fields_names1,)*} => {
                                #(#fields;)*
                            } ));
                        len_variants.push(quote!( #name::#variant_name_spanned{#(#fields_names3,)*} => {
                                #num_fields
                            } ));
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        for (idx, f) in fields_unnamed.unnamed.iter().enumerate() {
                            field_infos.push(FieldInfo {
                                field_span: f.ty.span(),
                                ident: None,
                                index: idx as u32,
                                ty: &f.ty,
                                attrs: &f.attrs,
                            });
                        }
                        let (fields_names, fields, introspect_key) = implement_introspect(field_infos, false);
                        let fields_names1 = fields_names.clone();
                        let fields_names2 = fields_names.clone();
                        let fields_names3 = fields_names.clone();
                        let num_fields = fields_names3.len();

                        if let Some(introspect_key) = introspect_key {
                            value_variants.push(quote!( #name::#variant_name_spanned(#(#fields_names1,)*) => {
                                    #introspect_key.to_string()
                            }));
                        } else {
                            value_variants.push(
                                quote!( #name::#variant_name_spanned(#(#fields_names2,)*) => #return_value_name.to_string() )
                            );
                        }

                        variants.push(quote!( #name::#variant_name_spanned(#(#fields_names,)*) => { #(#fields;)* } ));
                        len_variants.push(quote!( #name::#variant_name_spanned(#(#fields_names3,)*) => {
                                #num_fields
                            } ));
                    }
                    &syn::Fields::Unit => {
                        //No fields
                        variants.push(quote! {
                            #name::#variant_name_spanned => {}
                        });
                        value_variants.push(quote!( #name::#variant_name_spanned => #return_value_name.to_string() ));
                        len_variants.push(quote!( #name::#variant_name_spanned => 0));
                    }
                }

                //variants.push(quote!{})
            }
            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses

                    #[automatically_derived]
                    impl #impl_generics #introspect for #name #ty_generics #where_clause #extra_where {

                        #[allow(unused_mut)]
                        #[allow(unused_comparisons, unused_variables)]
                        fn introspect_value(&self) -> String {
                            match self {
                                #(#value_variants,)*
                            }
                        }
                        #[allow(unused_mut)]
                        #[allow(unused_comparisons, unused_variables)]
                        fn introspect_child(&self, index:usize) -> Option<Box<dyn #introspect_item_type+'_>> {
                            match self {
                                #(#variants,)*
                            }
                            return None;
                        }
                        #[allow(unused_mut)]
                        #[allow(unused_comparisons, unused_variables)]
                        fn introspect_len(&self) -> usize {
                            match self {
                                #(#len_variants,)*
                            }
                        }

                    }
                };
            }
        }
        &syn::Data::Struct(ref struc) => {
            let fields;
            match &struc.fields {
                &syn::Fields::Named(ref namedfields) => {
                    let field_infos: Vec<FieldInfo> = namedfields
                        .named
                        .iter()
                        .enumerate()
                        .map(|(idx, field)| FieldInfo {
                            ident: Some(field.ident.clone().expect("Expected identifier[10]")),
                            field_span: field.ident.as_ref().unwrap().span(),
                            ty: &field.ty,
                            index: idx as u32,
                            attrs: &field.attrs,
                        })
                        .collect();

                    fields = implement_introspect(field_infos, true);
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos: Vec<FieldInfo> = fields_unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(idx, f)| FieldInfo {
                            ident: None,
                            field_span: f.ty.span(),
                            ty: &f.ty,
                            index: idx as u32,
                            attrs: &f.attrs,
                        })
                        .collect();

                    fields = implement_introspect(field_infos, true);
                }
                &syn::Fields::Unit => {
                    fields = (Vec::new(), Vec::new(), None);
                }
            }
            let fields1 = fields.1;
            let introspect_key: Option<TokenStream> = fields.2;
            let field_count = fields1.len();
            let value_name;
            if let Some(introspect_key) = introspect_key {
                value_name = quote! { #introspect_key.to_string()};
            } else {
                value_name = quote! { stringify!(#name).to_string() };
            }
            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses

                    #[automatically_derived]
                    impl #impl_generics #introspect for #name #ty_generics #where_clause #extra_where {
                        #[allow(unused_comparisons)]
                        #[allow(unused_mut, unused_variables)]
                        fn introspect_value(&self) -> String {
                            #value_name
                        }
                        #[allow(unused_comparisons)]
                        #[allow(unused_mut, unused_variables)]
                        fn introspect_child(&self, index: usize) -> Option<Box<dyn #introspect_item_type+'_>> {
                            #(#fields1;)*
                            return None;
                        }
                        fn introspect_len(&self) -> usize {
                            #field_count
                        }
                    }
                };
            }
        }
        _ => {
            abort_call_site!("Unsupported datatype");
        }
    };

    expanded
}

#[allow(non_snake_case)]
fn implement_withschema(
    structname: &str,
    field_infos: Vec<FieldInfo>,
    is_enum: FieldOffsetStrategy,
    generics: &Generics,
    ty_generics: &TypeGenerics,
    impl_generics: &ImplGenerics,
) -> Vec<TokenStream> {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let local_version = quote_spanned! { defspan => local_version};
    let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    let WithSchema = quote_spanned! { defspan => _savefile::prelude::WithSchema };
    let fields1 = quote_spanned! { defspan => fields1 };

    let structname = Ident::new(structname, defspan);
    let offset_of = quote_spanned! {defspan=>
        _savefile::prelude::offset_of
    };

    let fn_impl_generics = if !generics.params.is_empty() {
        quote! { :: #impl_generics}
    } else {
        quote! {}
    };
    let mut fields = Vec::new();
    for (idx, field) in field_infos.iter().enumerate() {
        let verinfo = parse_attr_tag(field.attrs);
        if verinfo.ignore {
            continue;
        }
        let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

        let offset;
        match is_enum {
            FieldOffsetStrategy::EnumWithKnownOffsets(variant_index) => {
                offset = quote! { Some(get_variant_offsets #fn_impl_generics (#variant_index)[#idx]) };
            }
            FieldOffsetStrategy::EnumWithUnknownOffsets => {
                offset = quote! { None };
            }
            FieldOffsetStrategy::Struct => {
                if let Some(name) = field.ident.clone() {
                    offset = quote! { Some(#offset_of!(#structname #ty_generics, #name)) };
                } else {
                    let idx = Index::from(idx);
                    offset = quote! { Some(#offset_of!(#structname #ty_generics, #idx)) }
                };
            }
        }

        let name_str = if let Some(name) = field.ident.clone() {
            name.to_string()
        } else {
            idx.to_string()
        };
        let removed = check_is_remove(field.ty);
        let field_type = &field.ty;
        if field_from_version == 0 && field_to_version == u32::MAX {
            if removed.is_removed() {
                abort!(
                    field.ty.span(),
                    "The Removed type can only be used for removed fields. Use the savefile_version attribute."
                );
            }
            fields.push(quote_spanned!( span => #fields1.push(unsafe{#Field::unsafe_new(#name_str.to_string(), std::boxed::Box::new(<#field_type as #WithSchema>::schema(#local_version, context)), #offset)} )));
        } else {
            let mut version_mappings = Vec::new();
            let offset = if field_to_version != u32::MAX {
                quote!(None)
            } else {
                offset
            };
            for dt in verinfo.deserialize_types.iter() {
                let dt_from = dt.from;
                let dt_to = dt.to;
                let dt_field_type = syn::Ident::new(&dt.serialized_type, span);
                // We don't supply offset in this case, deserialized type doesn't match field type
                version_mappings.push(quote!{
                    if #local_version >= #dt_from && local_version <= #dt_to {
                        #fields1.push(#Field ::new( #name_str.to_string(), std::boxed::Box::new(<#dt_field_type as #WithSchema>::schema(#local_version, context))) );
                    }
                });
            }

            fields.push(quote_spanned!( span =>
                #(#version_mappings)*

                if #local_version >= #field_from_version && #local_version <= #field_to_version {
                    #fields1.push(unsafe{#Field ::unsafe_new( #name_str.to_string(), std::boxed::Box::new(<#field_type as #WithSchema>::schema(#local_version, context)), #offset )} );
                }
                ));
        }
    }
    fields
}

enum FieldOffsetStrategy {
    Struct,
    EnumWithKnownOffsets(usize /*variant index*/),
    EnumWithUnknownOffsets,
}

#[allow(non_snake_case)]
fn savefile_derive_crate_withschema(input: DeriveInput) -> TokenStream {
    //let mut have_u8 = false;

    //let discriminant_size = discriminant_size.expect("Enum discriminant must be u8, u16 or u32. Use for example #[repr(u8)].");

    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause, quote! {_savefile::prelude::WithSchema});

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let withschema = quote_spanned! {defspan=>
        _savefile::prelude::WithSchema
    };

    let SchemaStruct = quote_spanned! { defspan => _savefile::prelude::SchemaStruct };
    let SchemaEnum = quote_spanned! { defspan => _savefile::prelude::SchemaEnum };
    let Schema = quote_spanned! { defspan => _savefile::prelude::Schema };
    let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    let Variant = quote_spanned! { defspan => _savefile::prelude::Variant };

    //let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let max_variant_fields = enum1.variants.iter().map(|x| x.fields.len()).max().unwrap_or(0);

            let enum_size = get_enum_size(&input.attrs, enum1.variants.len());
            let need_determine_offsets = enum_size.explicit_size;

            let mut variants = Vec::new();
            let mut variant_field_offset_extractors = vec![];
            for (var_idx, variant) in enum1.variants.iter().enumerate() {
                /*if var_idx >= 256 {
                    panic!("Savefile does not support enums with 256 total variants. Sorry.");
                }*/
                let var_idx = var_idx as u8;
                let var_ident = variant.ident.clone();
                let variant_name = quote! { #var_ident };
                let variant_name_spanned = quote_spanned! { span => stringify!(#variant_name).to_string()};

                let verinfo = parse_attr_tag(&variant.attrs);
                let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

                if field_to_version != std::u32::MAX {
                    abort!(
                        variant.span(),
                        "Savefile automatic derive does not support removal of enum values."
                    );
                }

                let mut field_infos = Vec::new();

                let mut field_offset_extractors = vec![];

                let offset_extractor_match_clause;
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        let mut field_pattern = vec![];
                        for (idx, f) in fields_named.named.iter().enumerate() {
                            let field_name = f
                                .ident
                                .as_ref()
                                .expect("Enum variant with named fields *must* actually have a name")
                                .clone();
                            field_offset_extractors.push(quote!(unsafe { (#field_name as *const _ as *const u8).offset_from(base_ptr) as usize }));
                            field_pattern.push(field_name);
                            field_infos.push(FieldInfo {
                                ident: Some(f.ident.clone().expect("Expected identifier[1]")),
                                field_span: f.ident.as_ref().unwrap().span(),
                                ty: &f.ty,
                                index: idx as u32,
                                attrs: &f.attrs,
                            });
                        }
                        offset_extractor_match_clause = quote! {#name::#var_ident { #(#field_pattern,)* } };
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        let mut field_pattern = vec![];
                        for (idx, f) in fields_unnamed.unnamed.iter().enumerate() {
                            let field_binding = Ident::new(&format!("x{}", idx), Span::call_site());
                            field_pattern.push(field_binding.clone());
                            field_offset_extractors.push(quote!(unsafe { (#field_binding as *const _ as *const u8).offset_from(base_ptr) as usize }));
                            field_infos.push(FieldInfo {
                                ident: None,
                                field_span: f.ty.span(),
                                index: idx as u32,
                                ty: &f.ty,
                                attrs: &f.attrs,
                            });
                        }
                        offset_extractor_match_clause = quote! {#name::#var_ident ( #(#field_pattern,)* ) };
                    }
                    &syn::Fields::Unit => {
                        offset_extractor_match_clause = quote! {#name::#var_ident};
                        //No fields
                    }
                }
                while field_offset_extractors.len() < max_variant_fields {
                    field_offset_extractors.push(quote! {0});
                }

                variant_field_offset_extractors.push(quote! {
                   #offset_extractor_match_clause => {
                       [ #(#field_offset_extractors,)* ]
                   }
                });

                let field_offset_strategy = if need_determine_offsets && field_infos.is_empty() == false {
                    FieldOffsetStrategy::EnumWithKnownOffsets(var_idx as usize)
                } else {
                    FieldOffsetStrategy::EnumWithUnknownOffsets
                };

                let fields = implement_withschema(
                    &name.to_string(),
                    field_infos,
                    field_offset_strategy,
                    &generics,
                    &ty_generics,
                    &impl_generics,
                );

                variants.push(quote! {
                (#field_from_version,
                 #field_to_version,
                 #Variant { name: #variant_name_spanned, discriminant: #var_idx, fields:
                    {
                        let mut fields1 = Vec::<#Field>::new();
                        #(#fields;)*
                        fields1
                    }}
                )});
            }

            let field_offset_impl;
            if need_determine_offsets {
                let varbuf_assign;
                if enum_size.discriminant_size == 1 {
                    varbuf_assign = quote!( varbuf[0] = variant as u8; );
                } else if enum_size.discriminant_size == 2 {
                    // We only support little endian
                    varbuf_assign = quote!(
                        varbuf[0] = variant as u8;
                        varbuf[1] = (variant>>8) as u8;
                    );
                } else if enum_size.discriminant_size == 4 {
                    // We only support little endian
                    varbuf_assign = quote!(
                        varbuf[0] = variant as u8;
                        varbuf[1] = (variant>>8) as u8;
                        varbuf[2] = (variant>>16) as u8;
                        varbuf[3] = (variant>>24) as u8;
                    );
                } else {
                    abort_call_site!("Unsupported enum size: {}", enum_size.discriminant_size);
                }
                let not_const_if_gen = if generics.params.is_empty() {
                    quote! {const}
                } else {
                    quote! {}
                };
                let conjure_variant;
                if generics.params.is_empty() {
                    conjure_variant = quote! {
                        let mut varbuf = [0u8;std::mem::size_of::<#name #ty_generics>()];
                        #varbuf_assign
                        let mut value : MaybeUninit<#name #ty_generics> = unsafe { std::mem::transmute(varbuf) };
                    }
                } else {
                    let discr_type;
                    match enum_size.discriminant_size {
                        1 => discr_type = quote! { u8 },
                        2 => discr_type = quote! { u16 },
                        4 => discr_type = quote! { u32 },
                        _ => unreachable!(),
                    }
                    conjure_variant = quote! {
                        let mut value = MaybeUninit::< #name #ty_generics >::uninit();
                        let discr: *mut #discr_type = &mut value as *mut MaybeUninit<#name #ty_generics> as *mut #discr_type;
                        unsafe {
                            *discr = variant as #discr_type;
                        }
                    }
                }

                field_offset_impl = quote! {
                    #not_const_if_gen fn get_field_offset_impl #impl_generics (value: &#name #ty_generics) -> [usize;#max_variant_fields] {
                        assert!(std::mem::size_of::<#name #ty_generics>()>0);
                        let base_ptr = value as *const #name #ty_generics as *const u8;
                        match value {
                            #(#variant_field_offset_extractors)*
                        }
                    }
                    #not_const_if_gen fn get_variant_offsets #impl_generics(variant: usize) -> [usize;#max_variant_fields] {
                        #conjure_variant
                        //let base_ptr = &mut value as *mut MaybeUninit<#name> as *mut u8;
                        //unsafe { *base_ptr = variant as u8; }
                        get_field_offset_impl(unsafe { &*(&value as *const MaybeUninit<#name #ty_generics> as *const #name #ty_generics) } )
                    }
                };
            } else {
                field_offset_impl = quote! {};
            }

            let discriminant_size = enum_size.discriminant_size;
            let has_explicit_repr = enum_size.repr_c;

            quote! {
                #field_offset_impl

                #[automatically_derived]
                impl #impl_generics #withschema for #name #ty_generics #where_clause #extra_where {

                    #[allow(unused_mut)]
                    #[allow(unused_comparisons, unused_variables)]
                    fn schema(version:u32, context: &mut _savefile::prelude::WithSchemaContext) -> #Schema {
                        let local_version = version;

                        #Schema::Enum (
                            unsafe{#SchemaEnum::new_unsafe(
                                stringify!(#name).to_string(),
                                (vec![#(#variants),*]).into_iter().filter_map(|(fromver,tover,x)|{
                                    if local_version >= fromver && local_version <= tover {
                                        Some(x)
                                    } else {
                                        None
                                    }
                                }).collect(),
                                #discriminant_size,
                                #has_explicit_repr,
                                Some(std::mem::size_of::<#name #ty_generics>()),
                                Some(std::mem::align_of::<#name #ty_generics>()),
                            )}
                        )
                    }
                }

            }
        }
        &syn::Data::Struct(ref struc) => {
            let fields;
            match &struc.fields {
                &syn::Fields::Named(ref namedfields) => {
                    let field_infos: Vec<FieldInfo> = namedfields
                        .named
                        .iter()
                        .enumerate()
                        .map(|(idx, field)| FieldInfo {
                            ident: Some(field.ident.clone().expect("Expected identifier[2]")),
                            field_span: field.ident.span(),
                            ty: &field.ty,
                            index: idx as u32,
                            attrs: &field.attrs,
                        })
                        .collect();

                    fields = implement_withschema(
                        &name.to_string(),
                        field_infos,
                        FieldOffsetStrategy::Struct,
                        &generics,
                        &ty_generics,
                        &impl_generics,
                    );
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos: Vec<FieldInfo> = fields_unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(idx, f)| FieldInfo {
                            field_span: f.ty.span(),
                            ident: None,
                            index: idx as u32,
                            ty: &f.ty,
                            attrs: &f.attrs,
                        })
                        .collect();
                    fields = implement_withschema(
                        &name.to_string(),
                        field_infos,
                        FieldOffsetStrategy::Struct,
                        &generics,
                        &ty_generics,
                        &impl_generics,
                    );
                }
                &syn::Fields::Unit => {
                    fields = Vec::new();
                }
            }
            quote! {
                #[automatically_derived]
                impl #impl_generics #withschema for #name #ty_generics #where_clause #extra_where {
                    #[allow(unused_comparisons)]
                    #[allow(unused_mut, unused_variables)]
                    fn schema(version:u32, context: &mut _savefile::prelude::WithSchemaContext) -> #Schema {
                        let local_version = version;
                        let mut fields1 = Vec::new();
                        #(#fields;)* ;
                        #Schema::Struct(unsafe{#SchemaStruct::new_unsafe(
                            stringify!(#name).to_string(),
                            fields1,
                            Some(std::mem::size_of::<#name #ty_generics>()),
                            Some(std::mem::align_of::<#name #ty_generics>()),
                        )})

                    }
                }
            }
        }
        _ => {
            abort_call_site!("Unsupported datatype");
        }
    };
    // For debugging, uncomment to write expanded procmacro to file
    //std::fs::write(format!("/home/anders/savefile/savefile-abi-min-lib/src/expanded.rs"),expanded.to_string()).unwrap();

    expanded
}
