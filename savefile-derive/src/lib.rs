#![recursion_limit = "128"]
#![deny(warnings)]
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

//! This crate allows automatic derivation of the Savefile-traits: Serialize, Deserialize, WithSchema, ReprC and Introspect .
//! The documentation for this is found in the Savefile crate documentation.

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
extern crate syn;

use common::{
    check_is_remove, compile_time_check_reprc, compile_time_size, get_extra_where_clauses, parse_attr_tag,
    path_to_string, FieldInfo,
};
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
#[allow(unused_imports)]
use std::iter::IntoIterator;
use syn::__private::bool;
use syn::token::Paren;
use syn::Type::Tuple;
use syn::{DeriveInput, FnArg, Ident, Index, ItemTrait, Pat, ReturnType, TraitItem, Type, TypeGenerics, TypeTuple};

fn implement_fields_serialize(
    field_infos: Vec<FieldInfo>,
    implicit_self: bool,
    index: bool,
) -> (TokenStream, Vec<TokenStream>) {
    let mut min_safe_version = 0;
    let mut output = Vec::new();

    let defspan = proc_macro2::Span::call_site();
    let span = proc_macro2::Span::call_site();
    let local_serializer = quote_spanned! { defspan => local_serializer};

    let reprc = quote! {
        _savefile::prelude::ReprC
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
                    panic!(
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

                if field_to_version < std::u32::MAX {
                    min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
                }
                if field_from_version < std::u32::MAX {
                    // An addition
                    min_safe_version = min_safe_version.max(field_from_version);
                }
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
            panic!(
                "savefile_abi_exportable arguments should be of form #[savefile_abi_exportable(version=0)], not '{}'",
                attr
            );
        }
        let key = keyvals[0].trim();
        let val = keyvals[1].trim();
        match key {
            "version" => {
                if version.is_some() {
                    panic!("version specified more than once");
                }
                version = Some(
                    val.parse()
                        .unwrap_or_else(|_|panic!("Version must be numeric, but was: {}", val)),
                );
            }
            _ => panic!("Unknown savefile_abi_exportable key: '{}'", key),
        }
    }
    let version: u32 = version.unwrap_or(0);

    let trait_name_str = parsed.ident.to_string();
    let trait_name = parsed.ident;
    let defspan = proc_macro2::Span::mixed_site();
    let uses = quote_spanned! { defspan =>
        extern crate savefile;
        extern crate savefile_abi;
        use savefile::prelude::{Schema, SchemaPrimitive, WithSchema, Serializer, Serialize, Deserializer, Deserialize, SavefileError, deserialize_slice_as_vec, ReadBytesExt,LittleEndian,AbiMethodArgument, AbiMethod, AbiMethodInfo,AbiTraitDefinition};
        use savefile_abi::{abi_result_receiver, FlexBuffer, AbiExportable, TraitObject, PackagedTraitObject, Owning, AbiErrorMsg, RawAbiCallResult, AbiConnection, AbiConnectionMethod, parse_return_value, AbiProtocol, abi_entry_light};
        use std::collections::HashMap;
        use std::mem::MaybeUninit;
        use std::io::Cursor;
    };

    let mut method_metadata: Vec<TokenStream> = vec![];
    let mut callee_method_trampoline: Vec<TokenStream> = vec![];
    let mut caller_method_trampoline = vec![];
    let mut extra_definitions = vec![];

    for (method_number, item) in parsed.items.iter().enumerate() {
        if method_number > u16::MAX.into() {
            panic!("Savefile only supports 2^16 methods per interface. Sorry.");
        }
        let method_number = method_number as u16;

        match item {
            TraitItem::Const(c) => {
                panic!(
                    "savefile_abi_exportable does not support associated consts: {}",
                    c.ident
                );
            }
            TraitItem::Method(method) => {
                let method_name = method.sig.ident.clone();
                //let method_name_str = method.sig.ident.to_string();
                //let mut metadata_arguments = vec![];

                let mut receiver_is_mut = false;
                let ret_type;
                let ret_declaration;
                match &method.sig.output {
                    ReturnType::Default => {
                        ret_type = Tuple(TypeTuple {
                            paren_token: Paren::default(),
                            elems: Default::default(),
                        })
                        .to_token_stream();
                        ret_declaration = quote! {}
                    }
                    ReturnType::Type(_, ty) => {
                        match &**ty {
                            Type::Path(_type_path) => {
                                ret_type = ty.to_token_stream();
                                ret_declaration = quote! { -> #ret_type }
                            }
                            Type::Reference(_) => {
                                panic!("References in return-position are not supported.")
                            }
                            Type::Tuple(TypeTuple { elems, .. }) => {
                                if elems.len() > 3 {
                                    panic!("Savefile presently only supports tuples up to 3 members. Either change to using a struct, or file an issue on savefile!");
                                }
                                // Empty tuple!
                                ret_type = ty.to_token_stream();
                                ret_declaration = quote! { -> #ret_type }
                            }
                            _ => panic!("Unsupported type in return-position: {:?}", ty),
                        }
                    }
                }

                let self_arg = method.sig.inputs.iter().next().unwrap_or_else(||panic!(
                    "Method {} has no arguments. This is not supported - it must at least have a self-argument.",
                    method_name
                ));
                if let FnArg::Receiver(recv) = self_arg {
                    if let Some(reference) = &recv.reference {
                        if reference.1.is_some() {
                            panic!(
                                "Method {} has a lifetime for 'self' argument. This is not supported",
                                method_name
                            );
                        }
                        if recv.mutability.is_some() {
                            receiver_is_mut = true;
                        }
                    } else {
                        panic!(
                            "Method {} takes 'self' by value. This is not supported. Use &self",
                            method_name
                        );
                    }
                } else {
                    panic!("Method {} must have 'self'-parameter", method_name);
                }
                let mut args = Vec::with_capacity(method.sig.inputs.len());
                for (arg_index, arg) in method.sig.inputs.iter().enumerate().skip(1) {
                    match arg {
                        FnArg::Typed(typ) => {
                            match &*typ.pat {
                                Pat::Ident(name) => {
                                    args.push((name.ident.clone(), &*typ.ty));
                                }
                                _ => panic!("Method {} had a parameter (#{}, where self is #0) which contained a complex pattern. This is not supported.", method_name, arg_index)
                            }
                        },
                        _ => panic!("Unexpected error: method {} had a self parameter that wasn't the first parameter!", method_name)
                    }
                }
                let mut current_name_index = 0u32;
                let name_baseplate = format!("Temp{}_{}", trait_name_str, method_name);
                let mut temp_name_generator = move || {
                    current_name_index += 1;
                    format!("{}_{}", name_baseplate, current_name_index)
                };

                let method_defs = crate::savefile_abi::generate_method_definitions(
                    version,
                    trait_name.clone(),
                    method_number,
                    method_name,
                    ret_declaration,
                    ret_type,
                    receiver_is_mut,
                    args,
                    &mut temp_name_generator,
                    &mut extra_definitions,
                );
                method_metadata.push(method_defs.method_metadata);
                callee_method_trampoline.push(method_defs.callee_method_trampoline);
                caller_method_trampoline.push(method_defs.caller_method_trampoline);
            }
            TraitItem::Type(t) => {
                panic!("savefile_abi_exportable does not support associated types: {}", t.ident);
            }
            TraitItem::Macro(m) => {
                panic!("savefile_abi_exportable does not support macro items: {:?}", m);
            }
            x => panic!("Unsupported item in trait definition: {:?}", x),
        }
    }

    let abi_entry_light = Ident::new(&format!("abi_entry_light_{}", trait_name_str), Span::call_site());

    let exports_for_trait = quote! {

        pub unsafe extern "C" fn #abi_entry_light(flag: AbiProtocol) {
            unsafe { abi_entry_light::<dyn #trait_name>(flag); }
        }

        unsafe impl AbiExportable for dyn #trait_name {
            const ABI_ENTRY : unsafe extern "C" fn (flag: AbiProtocol)  = #abi_entry_light;
            fn get_definition( version: u32) -> AbiTraitDefinition {
                AbiTraitDefinition {
                    name: #trait_name_str.to_string(),
                    methods: vec! [ #(#method_metadata,)* ]
                }
            }

            fn get_latest_version() -> u32 {
                #version
            }

            fn call(trait_object: TraitObject, method_number: u16, effective_version:u32, compatibility_mask: u64, data: &[u8], abi_result: *mut (), receiver: unsafe extern "C" fn(outcome: *const RawAbiCallResult, result_receiver: *mut ()/*Result<T,SaveFileError>>*/)) -> Result<(),SavefileError> {

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

        impl #trait_name for AbiConnection<dyn #trait_name> {
            #(#caller_method_trampoline)*
        }
    };

    //let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());
    let input = TokenStream::from(input);
    let expanded = quote! {
        #[allow(clippy::double_comparisons)]
        #[allow(clippy::needless_late_init)]
        const _:() = {
            #uses

            #(#extra_definitions)*

            #exports_for_trait

        };

        #input
    };

    // For debugging, uncomment to write expanded procmacro to file
    std::fs::write(format!("/home/anders/savefile/savefile-min-build/src/{}.rs",trait_name_str),expanded.to_string()).unwrap();

    expanded.into()
}
#[proc_macro]
pub fn savefile_abi_export(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = item.to_string();
    let symbols: Vec<_> = input.split(',').map(|x| x.trim()).collect();
    if symbols.len() != 2 {
        panic!("savefile_abi_export requires two parameters. The first parameter is the implementing type, the second is the trait it implements.");
    }
    let defspan = Span::call_site();
    let uses = quote_spanned! { defspan =>
        extern crate savefile_abi;
        use savefile_abi::{AbiProtocol, AbiExportableImplementation, abi_entry};
    };

    let implementing_type = Ident::new(symbols[0], Span::call_site());
    let trait_type = Ident::new(symbols[1], Span::call_site());
    let abi_entry = Ident::new(("abi_entry_".to_string() + symbols[1]).as_str(), Span::call_site());

    let expanded = quote! {
        #[allow(clippy::double_comparisons)]
        const _:() = {
            #uses
            unsafe impl AbiExportableImplementation for #implementing_type {
                const ABI_ENTRY: unsafe extern "C" fn (AbiProtocol) = #abi_entry;
                type AbiInterface = dyn #trait_type;

                fn new() -> Box<Self::AbiInterface> {
                    Box::new(#implementing_type::default())
                }
            }
            #[no_mangle]
            pub unsafe extern "C" fn #abi_entry(flag: AbiProtocol) where #implementing_type: Default + #trait_type {
                unsafe { abi_entry::<#implementing_type>(flag); }
            }
        };
    };

    expanded.into()
}

#[proc_macro_derive(
    Savefile,
    attributes(
        savefile_unsafe_and_fast,
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

    let expanded = quote! {
        #s

        #d

        #w

        #i

        #r
    };
    //std::fs::write("/home/anders/savefile/savefile-min-build/src/expanded.rs", expanded.to_string()).unwrap();


    expanded.into()
}
#[proc_macro_derive(
    SavefileNoIntrospect,
    attributes(
        savefile_unsafe_and_fast,
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

    let expanded = quote! {
        #s

        #d

        #w

        #r
    };

    expanded.into()
}

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

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause, quote! {_savefile::prelude::WithSchema});
    let uses = quote_spanned! { defspan =>
        extern crate savefile as _savefile;
    };
    let reprc = quote_spanned! {defspan=>
        _savefile::prelude::ReprC
    };
    let isreprc = quote_spanned! {defspan=>
        _savefile::prelude::IsReprC
    };
    quote! {

        #[allow(non_upper_case_globals)]
        const #dummy_const: () = {
            extern crate std;
            #uses
            impl #impl_generics #reprc for #name #ty_generics #where_clause #extra_where {
                #[allow(unused_comparisons,unused_variables, unused_variables)]
                unsafe fn repr_c_optimization_safe(file_version:u32) -> #isreprc {
                    #isreprc::no()
                }
            }
        };
    }
}

#[allow(non_snake_case)]
fn implement_reprc(
    field_infos: Vec<FieldInfo>,
    generics: syn::Generics,
    name: syn::Ident,
    expect_fast: bool,
) -> TokenStream {
    let generics = generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause, quote! {_savefile::prelude::ReprC});

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let reprc = quote_spanned! {defspan=>
        _savefile::prelude::ReprC
    };
    let isreprc = quote_spanned! {defspan=>
        _savefile::prelude::IsReprC
    };
    let spanof = quote_spanned! {defspan=>
        _savefile::prelude::span_of
    };
    let local_file_version = quote_spanned! { defspan => local_file_version};
    //let WithSchema = quote_spanned! { defspan => _savefile::prelude::WithSchema};
    let mut min_safe_version = 0;
    let mut optsafe_outputs = Vec::new();
    let uses = quote_spanned! { defspan =>
        extern crate savefile as _savefile;
    };

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    for field in field_infos.windows(2) {
        let field_name1 = field[0].get_accessor();
        let field_name2 = field[1].get_accessor();

        optsafe_outputs.push(quote!( (#spanof!(#name #ty_generics, #field_name1).end == #spanof!(#name #ty_generics, #field_name2).start )));
    }
    if field_infos.len() > 0 {
        if field_infos.len() == 1 {
            optsafe_outputs.push(quote!(  (#spanof!( #name #ty_generics, ..).start == 0 )));
            optsafe_outputs.push(quote!(  (#spanof!( #name #ty_generics, ..).end == std::mem::size_of::<Self>() )));
        } else {
            let first = field_infos.first().expect("field_infos.first()[2]").get_accessor();
            let last = field_infos.last().expect("field_infos.last()[2]").get_accessor();
            optsafe_outputs.push(quote!( (#spanof!(#name #ty_generics, #first).start == 0 )));
            optsafe_outputs.push(quote!( (#spanof!(#name #ty_generics,#last).end == std::mem::size_of::<Self>() )));
        }
    }

    for field in &field_infos {
        let verinfo = parse_attr_tag(field.attrs);
        if verinfo.ignore {
            if expect_fast {
                panic!(
                    "The #[savefile_unsafe_and_fast] attribute cannot be used for structures containing ignored fields"
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
                    panic!("The Removed type can only be used for removed fields. Use the savefile_version attribute to mark a field as only existing in previous versions.");
                } else {
                    return implement_reprc_hardcoded_false(name, generics);
                }
            }
            optsafe_outputs
                .push(quote_spanned!( span => <#field_type as #reprc>::repr_c_optimization_safe(#local_file_version).is_yes()));
        } else {
            if field_to_version < std::u32::MAX {
                min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
            }

            min_safe_version = min_safe_version.max(field_from_version);

            if !removed.is_removed() {
                optsafe_outputs.push(
                    quote_spanned!( span => <#field_type as #reprc>::repr_c_optimization_safe(#local_file_version).is_yes()),
                );
            }
        }
    }

    quote! {

        #[allow(non_upper_case_globals)]
        #[allow(clippy::manual_range_contains)]
        const #dummy_const: () = {
            extern crate std;
            #uses
            impl #impl_generics #reprc for #name #ty_generics #where_clause #extra_where {
                #[allow(unused_comparisons,unused_variables, unused_variables)]
                unsafe fn repr_c_optimization_safe(file_version:u32) -> #isreprc {
                    let local_file_version = file_version;
                    if file_version >= #min_safe_version #( && #optsafe_outputs)* {
                        unsafe { #isreprc::yes() }
                    } else {
                        #isreprc::no()
                    }
                }
            }
        };
    }
}

struct EnumSize {
    discriminant_size: u8,
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
                                    panic!("Savefile does not support enums with more than 2^32 variants.")
                                }
                                _ => panic!("Unsupported repr(X) attribute on enum: {}", size_str),
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
                panic!("The enum had an unreasonable number of variants");
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
#[proc_macro_derive(
    ReprC,
    attributes(
        savefile_versions,
        savefile_versions_as,
        savefile_ignore,
        savefile_default_val,
        savefile_default_fn
    )
)]
pub fn reprc(_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    panic!("The #[derive(ReprC)] style of unsafe performance opt-in has been removed. The performance gains are now available automatically for any packed struct.")
}
fn derive_reprc_new(input: DeriveInput) -> TokenStream {
    let name = input.ident;

    let mut opt_in_fast = false;
    for attr in input.attrs.iter() {
        match attr.parse_meta() {
            Ok(ref meta) => match meta {
                &syn::Meta::Path(ref x) => {
                    let x = path_to_string(x);
                    if x == "savefile_unsafe_and_fast" {
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
            /*if enum1.variants.len() >= 256 {
                if opt_in_fast {
                    panic!("The #[savefile_unsafe_and_fast] attribute assumes that the enum representation is u8 or i8. Savefile does not support enums with more than 256 variants. Sorry.");
                }
                return implement_reprc_hardcoded_false(name, input.generics);
            }*/
            let enum_size = get_enum_size(&input.attrs, enum1.variants.len());
            if !enum_size.explicit_size {
                if opt_in_fast {
                    panic!("The #[savefile_unsafe_and_fast] requires an explicit #[repr(u8)],#[repr(u16)] or #[repr(u32)], attribute.");
                }
                return implement_reprc_hardcoded_false(name, input.generics);
            }
            _ = enum_size.repr_c; //We're going to use this in the future

            for variant in enum1.variants.iter() {
                match &variant.fields {
                    //TODO: #[repr(u8,C)] enums could implement ReprC
                    &syn::Fields::Named(ref _fields_named) => {
                        if opt_in_fast {
                            panic!("The #[savefile_unsafe_and_fast] attribute cannot be used for enums with fields.");
                        }
                        return implement_reprc_hardcoded_false(name, input.generics);
                    }
                    &syn::Fields::Unnamed(ref _fields_unnamed) => {
                        if opt_in_fast {
                            panic!("The #[savefile_unsafe_and_fast] attribute cannot be used for enums with fields.");
                        }
                        return implement_reprc_hardcoded_false(name, input.generics);
                    }
                    &syn::Fields::Unit => {}
                }
            }
            implement_reprc(vec![], input.generics, name, opt_in_fast)
        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {
                let field_infos: Vec<FieldInfo> = namedfields
                    .named
                    .iter()
                    .enumerate()
                    .map(|(field_index, field)| FieldInfo {
                        ident: Some(field.ident.clone().expect("Expected identifier [8]")),
                        index: field_index as u32,
                        ty: &field.ty,
                        attrs: &field.attrs,
                    })
                    .collect();

                implement_reprc(field_infos, input.generics, name, opt_in_fast)
            }
            &syn::Fields::Unnamed(ref fields_unnamed) => {
                let field_infos: Vec<FieldInfo> = fields_unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| FieldInfo {
                        ident: None,
                        index: idx as u32,
                        ty: &field.ty,
                        attrs: &field.attrs,
                    })
                    .collect();

                implement_reprc(field_infos, input.generics, name, opt_in_fast)
            }
            &syn::Fields::Unit => implement_reprc(Vec::new(), input.generics, name, opt_in_fast),
        },
        _ => {
            if opt_in_fast {
                panic!("Unsupported data type");
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
            panic!("Type had more than one field with savefile_introspect_key - attribute");
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
            panic!("Unsupported datatype");
        }
    };

    expanded
}

#[allow(non_snake_case)]
fn implement_withschema(
    structname: &str,
    field_infos: Vec<FieldInfo>,
    is_enum: FieldOffsetStrategy,
    ty_generics: &TypeGenerics,
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
                offset = quote! { Some(get_variant_offsets(#variant_index)[#idx]) };
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
        if field_from_version == 0 && field_to_version == std::u32::MAX {
            if removed.is_removed() {
                panic!("The Removed type can only be used for removed fields. Use the savefile_version attribute.");
            }
            fields.push(quote_spanned!( span => #fields1.push(#Field { name:#name_str.to_string(), value:Box::new(<#field_type as #WithSchema>::schema(#local_version)), offset: #offset })));
        } else {
            let mut version_mappings = Vec::new();
            for dt in verinfo.deserialize_types.iter() {
                let dt_from = dt.from;
                let dt_to = dt.to;
                let dt_field_type = syn::Ident::new(&dt.serialized_type, span);
                version_mappings.push(quote!{
                    if #local_version >= #dt_from && local_version <= #dt_to {
                        #fields1.push(#Field { name:#name_str.to_string(), value:Box::new(<#dt_field_type as #WithSchema>::schema(#local_version)), offset: #offset });
                    }
                });
            }

            fields.push(quote_spanned!( span =>
                #(#version_mappings)*

                if #local_version >= #field_from_version && #local_version <= #field_to_version {
                    #fields1.push(#Field { name:#name_str.to_string(), value:Box::new(<#field_type as #WithSchema>::schema(#local_version)), offset: #offset });
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
    let uses = quote_spanned! { defspan =>
        extern crate savefile as _savefile;
        use std::mem::MaybeUninit;
    };

    let SchemaStruct = quote_spanned! { defspan => _savefile::prelude::SchemaStruct };
    let SchemaEnum = quote_spanned! { defspan => _savefile::prelude::SchemaEnum };
    let Schema = quote_spanned! { defspan => _savefile::prelude::Schema };
    let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    let Variant = quote_spanned! { defspan => _savefile::prelude::Variant };

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let max_variant_fields = enum1.variants.iter().map(|x| x.fields.len()).max().unwrap_or(0);

            let enum_size = get_enum_size(&input.attrs, enum1.variants.len());
            let need_determine_offsets =
                enum_size.explicit_size && (enum_size.repr_c || max_variant_fields == 0);

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
                    panic!("Savefile automatic derive does not support removal of enum values.");
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

                let fields = implement_withschema(&name.to_string(), field_infos, field_offset_strategy, &ty_generics);

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
                field_offset_impl = quote! {
                    pub fn get_field_offset_impl(value: &#name) -> [usize;#max_variant_fields] {
                        assert!(std::mem::size_of::<#name>()>0);
                        let base_ptr = value as *const #name as *const u8;
                        match value {
                            #(#variant_field_offset_extractors)*
                        }
                    }
                    pub fn get_variant_offsets(variant: usize) -> [usize;#max_variant_fields] {
                        let mut value : MaybeUninit<#name> = MaybeUninit::uninit();
                        let base_ptr = &mut value as *mut MaybeUninit<#name> as *mut u8;
                        unsafe { *base_ptr = variant as u8; }
                        get_field_offset_impl(unsafe { &*(&value as *const MaybeUninit<#name> as *const #name) } )
                    }
                };
            } else {
                field_offset_impl = quote! {};
            }

            let discriminant_size = enum_size.discriminant_size;

            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #withschema for #name #ty_generics #where_clause #extra_where {

                        #[allow(unused_mut)]
                        #[allow(unused_comparisons, unused_variables)]
                        fn schema(version:u32) -> #Schema {
                            let local_version = version;

                            #field_offset_impl

                            #Schema::Enum (
                                #SchemaEnum {
                                    dbg_name : stringify!(#name).to_string(),
                                    discriminant_size: #discriminant_size,
                                    variants : (vec![#(#variants),*]).into_iter().filter_map(|(fromver,tover,x)|{
                                        if local_version >= fromver && local_version <= tover {
                                            Some(x)
                                        } else {
                                            None
                                        }
                                    }).collect(),

                                }
                            )
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
                            ident: Some(field.ident.clone().expect("Expected identifier[2]")),
                            ty: &field.ty,
                            index: idx as u32,
                            attrs: &field.attrs,
                        })
                        .collect();

                    fields = implement_withschema(
                        &name.to_string(),
                        field_infos,
                        FieldOffsetStrategy::Struct,
                        &ty_generics,
                    );
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos: Vec<FieldInfo> = fields_unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(idx, f)| FieldInfo {
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
                        &ty_generics,
                    );
                }
                &syn::Fields::Unit => {
                    fields = Vec::new();
                }
            }
            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #withschema for #name #ty_generics #where_clause #extra_where {
                        #[allow(unused_comparisons)]
                        #[allow(unused_mut, unused_variables)]
                        fn schema(version:u32) -> #Schema {
                            let local_version = version;
                            let mut fields1 = Vec::new();
                            #(#fields;)* ;
                            #Schema::Struct(#SchemaStruct{
                                dbg_name: stringify!(#name).to_string(),
                                fields: fields1
                            })

                        }
                    }
                };
            }
        }
        _ => {
            panic!("Unsupported datatype");
        }
    };
    // For debugging, uncomment to write expanded procmacro to file
    //std::fs::write(format!("/home/anders/savefile/savefile-abi-min-lib/src/expanded.rs"),expanded.to_string()).unwrap();

    expanded
}
