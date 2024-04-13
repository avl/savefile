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

use proc_macro2::{Literal, Span};
use proc_macro2::TokenStream;
    #[allow(unused_imports)]
use std::iter::IntoIterator;
use quote::ToTokens;
use syn::{DeriveInput, Expr, FnArg, GenericArgument, GenericParam, Generics, Ident, Index, ItemTrait, Lit, ParenthesizedGenericArguments, Pat, PathArguments, ReturnType, TraitItem, Type, TypeGenerics, TypeParamBound, TypeTuple, WhereClause};
use syn::__private::bool;
use syn::token::{Paren};
use syn::Type::Tuple;

#[derive(Debug)]
struct VersionRange {
    from: u32,
    to: u32,
    convert_fun: String,
    serialized_type: String,
}

#[derive(Debug)]
struct AttrsResult {
    version_from: u32,
    version_to: u32,
    ignore: bool,
    default_fn: Option<syn::Ident>,
    default_val: Option<TokenStream>,
    deserialize_types: Vec<VersionRange>,
    introspect_key: bool,
    introspect_ignore: bool,
}

enum RemovedType {
    NotRemoved,
    Removed,
    AbiRemoved
}
impl RemovedType {
    fn is_removed(&self) -> bool {
        match self {
            RemovedType::NotRemoved => {false}
            RemovedType::Removed => {true}
            RemovedType::AbiRemoved => {true}
        }
    }
}
fn check_is_remove(field_type: &syn::Type) -> RemovedType {

    let mut tokens = TokenStream::new();
    field_type.to_tokens(&mut tokens);
    for tok in tokens.into_iter() {
        if tok.to_string() == "Removed" {
            return RemovedType::Removed;
            //TODO: This is not robust, since it's based on text matching
        }
        if tok.to_string() == "AbiRemoved" {
            return RemovedType::AbiRemoved;
            //TODO: This is not robust, since it's based on text matching
        }
    }
    RemovedType::NotRemoved
}

fn parse_attr_tag(attrs: &[syn::Attribute]) -> AttrsResult {
    parse_attr_tag2(attrs, false)
}

fn overlap<'a>(b: &'a VersionRange) -> impl Fn(&'a VersionRange) -> bool {
    assert!(b.to >= b.from);
    move |a: &'a VersionRange| {
        assert!(a.to >= a.from);
        let no_overlap = a.to < b.from || a.from > b.to;
        !no_overlap
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments.last().expect("Expected at least one segment").ident.to_string()
}



fn parse_attr_tag2(attrs: &[syn::Attribute], _is_string_default_val: bool) -> AttrsResult {
    let mut field_from_version = None;
    let mut field_to_version = None;
    let mut default_fn = None;
    let mut default_val = None;
    let mut ignore = false;
    let mut introspect_ignore = false;
    let mut introspect_key = false;
    let mut deser_types = Vec::new();
    for attr in attrs.iter() {
        match attr.parse_meta() {
            Ok(ref meta) => {
                match meta {
                    syn::Meta::Path(x) => {
                        let x = path_to_string(x);
                        if x == "savefile_ignore" {
                            ignore = true;
                        }
                        if x == "savefile_introspect_key" {
                            introspect_key = true;
                        }
                        if x == "savefile_introspect_ignore" {
                            introspect_ignore = true;
                        }
                    }
                    &syn::Meta::List(ref _x) => {
                    }
                    &syn::Meta::NameValue(ref x) => {
                        let path = path_to_string(&x.path);
                        if path == "savefile_default_val" {
                            match &x.lit {
                                &syn::Lit::Str(ref litstr) => {
                                    default_val = Some(quote! { str::parse(#litstr).expect("Expected valid literal string") })
                                },
                                _ => {
                                    let lv = &x.lit;
                                    default_val = Some(quote!{#lv});
                                }
                            };

                        };
                        if path == "savefile_default_fn" {
                            let default_fn_str_lit = match &x.lit {
                                &syn::Lit::Str(ref litstr) => litstr,
                                _ => {
                                    panic!("Unexpected attribute value, please specify savefile_default_fn method names within quotes.");
                                }
                            };
                            default_fn = Some(syn::Ident::new(
                                &default_fn_str_lit.value(),
                                proc_macro2::Span::call_site(),
                            ));
                        };

                        if path == "savefile_ignore" {
                            ignore = true;
                        };
                        if path == "savefile_introspect_ignore" {
                            introspect_ignore = true;
                        };
                        if path == "savefile_versions_as" {
                            match &x.lit {
                                &syn::Lit::Str(ref litstr2) => {
                                    let output2: Vec<String> =
                                        litstr2.value().splitn(3, ':').map(|x| x.to_string()).collect();
                                    if output2.len() != 3 && output2.len() != 2 {
                                        panic!("The #savefile_versions_as tag must contain a version range and a deserialization type, such as : #[savefile_versions_as=0..3:MyStructType]");
                                    }
                                    let litstr = &output2[0];

                                    let convert_fun: String;
                                    let version_type: String;

                                    if output2.len() == 2 {
                                        convert_fun = "".to_string();
                                        version_type = output2[1].to_string();
                                    } else {
                                        convert_fun = output2[1].to_string();
                                        version_type = output2[2].to_string();
                                    }

                                    let output: Vec<String> = litstr.split("..").map(|x| x.to_string()).collect();
                                    if output.len() != 2 {
                                        panic!("savefile_versions_as tag must contain a (possibly half-open) range, such as 0..3 or 2.. (fields present in all versions to date should not use the savefile_versions_as-attribute)");
                                    }
                                    let (a, b) = (output[0].to_string(), output[1].to_string());

                                    let from_ver = if a.trim() == "" {
                                        0
                                    } else if let Ok(a_u32) = a.parse::<u32>() {
                                        a_u32
                                    } else {
                                        panic!("The from version in the version tag must be an integer. Use #[savefile_versions_as=0..3:MyStructType] for example");
                                    };

                                    let to_ver = if b.trim() == "" {
                                        std::u32::MAX
                                    } else if let Ok(b_u32) = b.parse::<u32>() {
                                        b_u32
                                    } else {
                                        panic!("The to version in the version tag must be an integer. Use #[savefile_versions_as=0..3:MyStructType] for example");
                                    };
                                    if to_ver < from_ver {
                                        panic!("Version ranges must specify lower number first.");
                                    }

                                    let item = VersionRange {
                                        from: from_ver,
                                        to: to_ver,
                                        convert_fun: convert_fun.to_string(),
                                        serialized_type: version_type.to_string(),
                                    };
                                    if deser_types.iter().any(overlap(&item)) {
                                        panic!("#savefile_versions_as attributes may not specify overlapping ranges");
                                    }
                                    deser_types.push(item);
                                }
                                _ => panic!("Unexpected datatype for value of attribute savefile_versions_as"),
                            }
                        }

                        if path == "savefile_versions" {
                            match &x.lit {
                                &syn::Lit::Str(ref litstr) => {
                                    let output: Vec<String> = litstr.value().split("..").map(|x| x.to_string()).collect();
                                    if output.len() != 2 {
                                        panic!("savefile_versions tag must contain a (possibly half-open) range, such as 0..3 or 2.. (fields present in all versions to date should not use the savefile_versions-attribute)");
                                    }
                                    let (a, b) = (output[0].to_string(), output[1].to_string());

                                    if field_from_version.is_some() || field_to_version.is_some() {
                                        panic!("There can only be one savefile_versions attribute on each field.")
                                    }
                                    if a.trim() == "" {
                                        field_from_version = Some(0);
                                    } else if let Ok(a_u32) = a.parse::<u32>() {
                                        field_from_version = Some(a_u32);
                                    } else {
                                        panic!("The from version in the version tag must be an integer. Use #[savefile_versions=0..3] for example");
                                    }

                                    if b.trim() == "" {
                                        field_to_version = Some(std::u32::MAX);
                                    } else if let Ok(b_u32) = b.parse::<u32>() {
                                        field_to_version = Some(b_u32);
                                    } else {
                                        panic!("The to version in the version tag must be an integer. Use #[savefile_versions=0..3] for example");
                                    }
                                    if field_to_version.expect("Expected field_to_version") < field_from_version.expect("expected field_from_version") {
                                        panic!("savefile_versions ranges must specify lower number first.");
                                    }
                                }
                                _ => panic!("Unexpected datatype for value of attribute savefile_versions"),
                            }
                        }
                    }
                }
            }
            Err(e) => {
                panic!("Unparsable attribute: {:?} ({:?})",e, attr.tokens);
            }
        }
    }

    let versions_tag_range = VersionRange {
        from: field_from_version.unwrap_or(0),
        to: field_to_version.unwrap_or(std::u32::MAX),
        convert_fun: "dummy".to_string(),
        serialized_type: "dummy".to_string(),
    };
    if deser_types.iter().any(overlap(&versions_tag_range)) {
        panic!("The version ranges of #version_as attributes may not overlap those of #savefile_versions");
    }
    for dt in deser_types.iter() {
        if dt.to >= field_from_version.unwrap_or(0) {
            panic!("The version ranges of #version_as attributes must be lower than those of the #savefile_versions attribute.");
        }
    }

    AttrsResult {
        version_from: field_from_version.unwrap_or(0),
        version_to: field_to_version.unwrap_or(std::u32::MAX),
        default_fn,
        default_val,
        ignore,
        deserialize_types: deser_types,
        introspect_key,
        introspect_ignore,
    }
}

struct FieldInfo<'a> {
    ident: Option<syn::Ident>,
    index: u32,
    ty: &'a syn::Type,
    attrs: &'a Vec<syn::Attribute>,
}
impl<'a> FieldInfo<'a> {
    /// field name for named fields, .1 or .2 for tuple fields.
    pub fn get_accessor(&self) -> TokenStream {
        match &self.ident {
            None => {
                let index = syn::Index::from(self.index as usize);
                index.to_token_stream()
            }
            Some(id) => {
                id.to_token_stream()
            }
        }
    }
}
fn compile_time_size(typ: &Type) -> Option<(usize/*size*/, usize/*alignment*/)> {
    match typ {
        Type::Path(p) => {
            if let Some(ident) = p.path.get_ident() {
                match ident.to_string().as_str() {
                    "u8" => Some((1,1)),
                    "i8" => Some((1,1)),
                    "u16" => Some((2,2)),
                    "i16" => Some((2,2)),
                    "u32" => Some((4,4)),
                    "i32" => Some((4,4)),
                    "u64" => Some((8,8)),
                    "i64" => Some((8,8)),
                    "char" => Some((4,4)),
                    "bool" => Some((1,1)),
                    "f32" => Some((4,4)),
                    "f64" => Some((8,8)),
                    _ => None,
                }
            } else {
                None
            }
        }
        Type::Tuple(t) => {
            let mut itemsize_align = None;
            let mut result_size = 0;
            if t.elems.iter().next().is_none() { //Empty tuple
                return Some((0,1));
            }
            for item in t.elems.iter() {
                let (cursize,curalign) = compile_time_size(item)?;
                if let Some(itemsize_align) = itemsize_align {
                    if itemsize_align != (cursize,curalign) {
                        // All items not the same size and have same alignment. Otherwise: Might be padding issues.
                        return None; //It could conceivably still be reprC, safe, but we're conservative here.
                    }
                } else {
                    itemsize_align = Some((cursize,curalign));
                }
                result_size += cursize;
            }
            if let Some((_itemsize, itemalign)) = itemsize_align {
                Some((result_size, itemalign))
            } else {
                None
            }
        }
        Type::Array(a) => {
            let (itemsize, itemalign) = compile_time_size(&a.elem)?;
            match &a.len {
                Expr::Lit(l) => {
                    match &l.lit {
                        Lit::Int(t) => {
                            let size : usize = t.base10_parse().ok()?;
                            Some((size*itemsize, itemalign))
                        }
                        _ => None
                    }
                }
                _ => None
            }
        }
        _ => None
    }
}
fn compile_time_check_reprc(typ: &Type) -> bool {

    match typ {
        Type::Path(p) => {
            if let Some(name) = p.path.get_ident() {
                let name = name.to_string();
                match name.as_str() {
                    "u8" => true,
                    "i8" => true,
                    "u16" => true,
                    "i16" => true,
                    "u32" => true,
                    "i32" => true,
                    "u64" => true,
                    "i64" => true,
                    "char" => true,
                    "bool" => true,
                    "f32" => true,
                    "f64" => true,
                    _ => false,
                }
            } else {
                false
            }
        }
        Type::Array(x) => {
            compile_time_check_reprc(&x.elem)
        }
        Type::Tuple(t) => {
            let mut size = None;
            for x in &t.elems {
                if !compile_time_check_reprc(x)
                {
                    return false;
                }
                let xsize = if let Some(s) = compile_time_size(x) {s} else {return false};
                if let Some(size) = size {
                    if xsize != size {
                        return false;
                    }
                } else {
                    size = Some(xsize);
                }
            }
            true
        }
        _ => false
    }
}

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

    let mut deferred_reprc : Option<(usize/*align*/,Vec<TokenStream>)> = None;
    fn realize_any_deferred(local_serializer: &TokenStream, deferred_reprc: &mut Option<(usize,Vec<TokenStream>)>, output: &mut Vec<TokenStream>) {
        let local_serializer:TokenStream = local_serializer.clone();
        if let Some((_align, deferred)) = deferred_reprc.take() {
            assert_eq!(deferred.is_empty(), false);
            let mut conditions = vec![];
            for item in deferred.windows(2) {
                let a = item[0].clone();
                let b = item[1].clone();
                if conditions.is_empty() == false {
                    conditions.push(quote!(&&));
                }
                conditions.push(quote!( std::ptr::addr_of!(#a).add(1) as *const u8 == std::ptr::addr_of!(#b) as *const u8 ));
            }
            if conditions.is_empty() {
                conditions.push(quote!( true ));
            }
            let mut fallbacks = vec![];
            for item in deferred.iter() {
                fallbacks.push(quote!(
                <_ as _savefile::prelude::Serialize>::serialize(&#item, #local_serializer)?;
                ));
            }
            if deferred.len() == 1 {
                return output.push(quote!( #(#fallbacks)* ) );
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

    let get_obj_id = |field:&FieldInfo| -> TokenStream {
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
        total_reprc_opt = quote!( );
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

fn get_extra_where_clauses(gen2: &Generics, where_clause: Option<&WhereClause>, the_trait: TokenStream) -> TokenStream {
    let extra_where_separator;
    if where_clause.is_some() {
        extra_where_separator = quote!(,);
    } else {
        extra_where_separator = quote!(where);
    }
    let mut where_clauses = vec![];
    for param in gen2.params.iter() {
        if let GenericParam::Type(t) = param {
            let t_name = &t.ident;
            let clause = quote!{#t_name : #the_trait};
            where_clauses.push(clause);
        }
    }
    let extra_where = quote!{
        #extra_where_separator #(#where_clauses),*
    };
    extra_where
}
fn savefile_derive_crate_serialize(input: DeriveInput) -> TokenStream {
    let name = input.ident;

    let generics = input.generics;

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause,quote!{_savefile::prelude::Serialize + _savefile::prelude::ReprC});


    let uses = quote_spanned! { defspan =>
        extern crate savefile as _savefile;
    };

    let serialize = quote_spanned! {defspan=>
        _savefile::prelude::Serialize
    };
    let serializer = quote_spanned! {defspan=>
        _savefile::prelude::Serializer<impl std::io::Write>
    };
    let saveerr = quote_spanned! {defspan=>
        Result<(),_savefile::prelude::SavefileError>
    };

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut output = Vec::new();
            //let variant_count = enum1.variants.len();
            /*if variant_count >= 256 {
                panic!("This library is not capable of serializing enums with 256 variants or more. Our deepest apologies, we thought no-one would ever create such an enum!");
            }*/
            let enum_size = get_enum_size(&input.attrs, enum1.variants.len());

            for (var_idx_usize, variant) in enum1.variants.iter().enumerate() {
                let var_idx_u8:u8 = var_idx_usize as u8;
                let var_idx_u16:u16 = var_idx_usize as u16;
                let var_idx_u32:u32 = var_idx_usize as u32;

                let variant_serializer = match enum_size.discriminant_size {
                    1 => quote! { serializer.write_u8(#var_idx_u8)? ; },
                    2 => quote! { serializer.write_u16(#var_idx_u16)? ; },
                    4 => quote! { serializer.write_u32(#var_idx_u32)? ; },
                    _ => unreachable!(),
                };

                let var_ident = (variant.ident).clone();
                let variant_name = quote! { #name::#var_ident };
                let variant_name_spanned = quote_spanned! { span => #variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        let field_infos: Vec<FieldInfo> = fields_named
                            .named
                            .iter()
                            .enumerate()
                            .map(|(field_index,field)| FieldInfo {
                                ident: Some(field.ident.clone().expect("Expected identifier[4]")),
                                index: field_index as u32,
                                ty: &field.ty,
                                attrs: &field.attrs,
                            })
                            .collect();

                        let (fields_serialized, fields_names) = implement_fields_serialize(field_infos, false, false);
                        output.push(quote!( #variant_name_spanned{#(#fields_names,)*} => { 
                                #variant_serializer
                                #fields_serialized 
                            } ));
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        let field_infos: Vec<FieldInfo> = fields_unnamed
                            .unnamed
                            .iter()
                            .enumerate()
                            .map(|(idx, field)| FieldInfo {
                                ident: Some(syn::Ident::new( // We bind the tuple field to a real name, like x0, x1 etc.
                                    &("x".to_string() + &idx.to_string()),
                                    Span::call_site(),
                                )),
                                index: idx as u32,
                                ty: &field.ty,
                                attrs: &field.attrs,
                            })
                            .collect();

                        let (fields_serialized, fields_names) = implement_fields_serialize(field_infos, false, false /*we've invented real names*/);

                        output.push(
                            quote!( #variant_name_spanned(#(#fields_names,)*) => { #variant_serializer ; #fields_serialized  } ),
                        );
                    }
                    &syn::Fields::Unit => {
                        output.push(quote!( #variant_name_spanned => { #variant_serializer ; } ));
                    }
                }
            }
            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #serialize for #name #ty_generics #where_clause #extra_where {

                        #[allow(unused_comparisons, unused_variables)]
                        fn serialize(&self, serializer: &mut #serializer) -> #saveerr {
                            match self {
                                #(#output,)*
                            }
                            Ok(())
                        }
                    }
                };
            }
        }
        &syn::Data::Struct(ref struc) => {
            let fields_serialize: TokenStream;
            let _field_names: Vec<TokenStream>;
            match &struc.fields {
                &syn::Fields::Named(ref namedfields) => {
                    let field_infos: Vec<FieldInfo> = namedfields
                        .named
                        .iter()
                        .enumerate()
                        .map(|(field_index,field)| FieldInfo {
                            ident: Some(field.ident.clone().expect("Identifier[5]")),
                            ty: &field.ty,
                            index: field_index as u32,
                            attrs: &field.attrs,
                        })
                        .collect();

                    let t = implement_fields_serialize(field_infos, true, false);
                    fields_serialize = t.0;
                    _field_names = t.1;
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos: Vec<FieldInfo> = fields_unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(field_index,field)| FieldInfo {
                            ident: None,
                            ty: &field.ty,
                            index: field_index as u32,
                            attrs: &field.attrs,
                        })
                        .collect();

                    let t = implement_fields_serialize(field_infos, true, true);
                    fields_serialize = t.0;
                    _field_names = t.1;
                }
                &syn::Fields::Unit => {
                    _field_names = Vec::new();
                    fields_serialize = quote! { {} };
                }
            }
            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #serialize for #name #ty_generics #where_clause #extra_where {
                        #[allow(unused_comparisons, unused_variables)]
                        fn serialize(&self, serializer: &mut #serializer)  -> #saveerr {
                            #fields_serialize
                            Ok(())
                        }
                    }
                };
            }
        }
        _ => {
            panic!("Unsupported data type");
        }
    };

    expanded
}

fn implement_deserialize(field_infos: Vec<FieldInfo>) -> Vec<TokenStream> {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let removeddef = quote_spanned! { defspan => _savefile::prelude::Removed };
    let abiremoveddef = quote_spanned! { defspan => _savefile::prelude::AbiRemoved };
    let local_deserializer = quote_spanned! { defspan => deserializer};

    let mut output = Vec::new();
    let mut min_safe_version = 0;
    for field in &field_infos {
        let field_type = &field.ty;

        let is_removed = check_is_remove(field_type);

        let verinfo = parse_attr_tag(field.attrs);
        let (field_from_version, field_to_version, default_fn, default_val) = (
            verinfo.version_from,
            verinfo.version_to,
            verinfo.default_fn,
            verinfo.default_val,
        );
        let mut exists_version_which_needs_default_value = false;
        if verinfo.ignore {
            exists_version_which_needs_default_value = true;
        } else {
            for ver in 0..verinfo.version_from {
                if !verinfo.deserialize_types.iter().any(|x| ver >= x.from && ver <= x.to) {
                    exists_version_which_needs_default_value = true;
                }
            }
        }

        let effective_default_val = if is_removed.is_removed() {
            match is_removed {
                RemovedType::Removed => quote! { #removeddef::new() },
                RemovedType::AbiRemoved => quote! { #abiremoveddef::new() },
                _ => unreachable!()
            }

        } else if let Some(defval) = default_val {
            quote! { #defval }
        } else if let Some(default_fn) = default_fn {
            quote_spanned! { span => #default_fn() }
        } else if !exists_version_which_needs_default_value {
            quote! { panic!("Unexpected unsupported file version: {}",#local_deserializer.file_version) }
        //Should be impossible
        } else {
            quote_spanned! { span => Default::default() }
        };
        if field_from_version > field_to_version {
            panic!("Version range is reversed. This is not allowed. Version must be range like 0..2, not like 2..0");
        }

        let src = if field_from_version == 0 && field_to_version == std::u32::MAX && !verinfo.ignore {
            if is_removed.is_removed() {
                panic!("The Removed type may only be used for fields which have an old version.");
                //TODO: Better message, tell user how to do this annotation
            };
            quote_spanned! { span =>
                <#field_type as _savefile::prelude::Deserialize>::deserialize(#local_deserializer)?
            }
        } else if verinfo.ignore {
            quote_spanned! { span =>
                #effective_default_val
            }
        } else {
            if field_to_version < std::u32::MAX {
                // A delete
                min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
            }
            if field_from_version < std::u32::MAX {
                // An addition
                min_safe_version = min_safe_version.max(field_from_version);
            }
            let mut version_mappings = Vec::new();
            for dt in verinfo.deserialize_types.iter() {
                let dt_from = dt.from;
                let dt_to = dt.to;
                let dt_field_type = syn::Ident::new(&dt.serialized_type, span);
                let dt_convert_fun = if dt.convert_fun.len() > 0 {
                    let dt_conv_fun = syn::Ident::new(&dt.convert_fun, span);
                    quote! { #dt_conv_fun }
                } else {
                    quote! { <#field_type>::from }
                };

                version_mappings.push(quote!{
                    if #local_deserializer.file_version >= #dt_from && #local_deserializer.file_version <= #dt_to {
                        let temp : #dt_field_type = <#dt_field_type as _savefile::prelude::Deserialize>::deserialize(#local_deserializer)?;
                        #dt_convert_fun(temp)
                    } else 
                });
            }

            quote_spanned! { span =>
                #(#version_mappings)*
                if #local_deserializer.file_version >= #field_from_version && #local_deserializer.file_version <= #field_to_version {
                    <#field_type as _savefile::prelude::Deserialize>::deserialize(#local_deserializer)?
                } else {
                    #effective_default_val
                }
            }
        };

        if let Some(ref id) = field.ident {
            let id_spanned = quote_spanned! { span => #id};
            output.push(quote!(#id_spanned : #src ));
        } else {
            output.push(quote!( #src ));
        }
    }
    output
}

fn emit_closure_helpers(
    version: u32,
    temp_trait_name: Ident, args: &ParenthesizedGenericArguments,
    ismut: bool,
    extra_definitions: &mut Vec<TokenStream>,
    fnkind: Ident
) {


    let temp_trait_name_wrapper = Ident::new(
        &format!("{}_wrapper", temp_trait_name), Span::call_site()
    );

    let mut formal_parameter_declarations = vec![];
    let mut parameter_types = vec![];
    let mut arg_names = vec![];

    for (arg_index,arg) in args.inputs.iter().enumerate() {
        let arg_name = Ident::new(&format!("x{}", arg_index), Span::call_site());
        formal_parameter_declarations.push(quote!{#arg_name : #arg});
        parameter_types.push(arg.to_token_stream());
        arg_names.push(arg_name.to_token_stream());
    }

    let ret_type;
    let ret_type_decl;

    if let ReturnType::Type(_, rettype) = &args.output {
        let typ = rettype.to_token_stream();
        ret_type = quote!{#typ};
        ret_type_decl = quote!{ -> #typ };
    } else {
        ret_type = quote!{ () };
        ret_type_decl = quote!{};
    }

    let version = Literal::u32_unsuffixed(version);

    let mutsymbol;
    let mutorconst;
    if ismut {
        mutsymbol = quote!{mut};
        mutorconst = quote!{mut};
    } else {
        mutsymbol = quote!{};
        mutorconst = quote!{const};
    }


    let expanded = quote! {

        #[savefile_abi_exportable(version=#version)]
        pub trait #temp_trait_name {
            fn docall(& #mutsymbol self, #(#formal_parameter_declarations,)*) -> #ret_type;
        }

        struct #temp_trait_name_wrapper<'a> {
            func: *#mutorconst (dyn for<'x> #fnkind( #(#parameter_types,)* ) #ret_type_decl +'a)
        }
        impl<'a> #temp_trait_name for #temp_trait_name_wrapper<'a> {
            fn docall(&#mutsymbol self, #(#formal_parameter_declarations,)*) -> #ret_type {
                unsafe { (&#mutsymbol *self.func)( #(#arg_names,)* )}
            }
        }

    };
    extra_definitions.push(expanded);
}

#[proc_macro]
pub fn savefile_abi_export(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = item.to_string();
    let symbols: Vec<_> = input.split(',').map(|x|x.trim()).collect();
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
                const ABI_ENTRY: extern "C" fn (AbiProtocol) = #abi_entry;
                type AbiInterface = dyn #trait_type;

                fn new() -> Box<Self::AbiInterface> {
                    Box::new(#implementing_type::default())
                }
            }
            #[no_mangle]
            pub extern "C" fn #abi_entry(flag: AbiProtocol) where #implementing_type: Default + #trait_type {
                unsafe { abi_entry::<#implementing_type>(flag); }
            }
        };
    };

    expanded.into()
}

enum ArgType {
    PlainData(TokenStream),
    Reference(TokenStream),
    SliceReference(TokenStream),
    TraitReference(Ident, bool/*ismut*/),
    BoxedTrait(Ident),
    Fn(
        Ident,/*Name of temporary trait generated to be able to handle Fn* as dyn TemporaryTrait. */
        TokenStream,/*full closure definition (e.g "Fn(u32)->u16")*/
        Vec<Type>,/*arg types*/
        bool/*ismut*/
    ),
}

struct MethodDefinitionComponents {
    method_metadata: TokenStream,
    callee_method_trampoline: TokenStream,
    caller_method_trampoline: TokenStream,
}

fn parse_type(version: u32, arg_name: &Ident, typ: &Type, method_name: &Ident, name_generator: &mut impl FnMut() -> String,extra_definitions: &mut Vec<TokenStream>, is_reference: bool, is_mut_ref: bool) -> ArgType {
    let rawtype;
    match typ {
        Type::Tuple(tup) if tup.elems.is_empty() => {
            rawtype = typ.to_token_stream();
            //argtype = ArgType::PlainData(typ.to_token_stream());
        }
        Type::Reference(typref) => {
            if typref.lifetime.is_some() {
                panic!("Method {}, argument {}: Specifying lifetimes is not supported.", method_name, arg_name);
            }
            if is_reference {
                panic!("Method {}, argument {}: Method arguments cannot be reference to reference in Savefile-abi. Try removing a '&' from the type: {}", method_name, arg_name, typ.to_token_stream());
            }
            return parse_type(version, arg_name, &*typref.elem, method_name, &mut *name_generator, extra_definitions, true, typref.mutability.is_some());
        }
        Type::Tuple(tuple) => {
            if tuple.elems.len() > 3 {
                panic!("Savefile presently only supports tuples up to 3 members. Either change to using a struct, or file an issue on savefile!");
            }
            rawtype = tuple.to_token_stream();
        }
        Type::Slice(slice) => {
            if !is_reference {
                panic!("Method {}, argument {}: Slices must always be behind references. Try adding a '&' to the type: {}", method_name, arg_name, typ.to_token_stream());
            }
            if is_mut_ref {
                panic!("Method {}, argument {}: Mutable refernces are not supported by Savefile-abi, except for FnMut-trait objects. {}", method_name, arg_name, typ.to_token_stream());
            }
            return ArgType::SliceReference(slice.elem.to_token_stream());
        }
        Type::TraitObject(trait_obj) => {
            if !is_reference {
                panic!("Method {}, argument {}: Trait objects must always be behind references. Try adding a '&' to the type: {}", method_name, arg_name, typ.to_token_stream());
            }
            if trait_obj.dyn_token.is_some() {
                let type_bounds:Vec<_> = trait_obj.bounds.iter().filter_map(|x|{
                    match x {
                        TypeParamBound::Trait(t) => {Some(t.path.segments.iter().last().expect("Missing bounds of Box trait object"))}
                        TypeParamBound::Lifetime(_) => {
                            panic!("Method {}, argument {}: Specifying lifetimes is not supported.", method_name, arg_name);
                        }
                    }
                }).collect();
                if type_bounds.len() == 0 {
                    panic!("Method {}, argument {}, unsupported trait object reference. Only &dyn Trait is supported. Encountered zero traits.", method_name, arg_name);
                }
                if type_bounds.len() > 1 {
                    panic!("Method {}, argument {}, unsupported Box-type. Only &dyn Trait> is supported. Encountered multiple traits: {:?}", method_name, arg_name, trait_obj);
                }
                let bound = type_bounds.into_iter().next().expect("Internal error, missing bounds");

                if bound.ident == "Fn" || bound.ident == "FnMut" || bound.ident == "FnOnce" {
                    if bound.ident == "FnOnce" {
                        panic!("Method {}, argument {}, FnOnce is not supported. Maybe you can use FnMut instead?", method_name, arg_name);
                    }

                    if bound.ident == "FnMut" && !is_mut_ref {
                        panic!("Method {}, argument {}: When using FnMut, it must be referenced using &mut, not &. Otherwise, it is impossible to call.", method_name, arg_name);
                    }
                    let fn_decl = bound.to_token_stream();
                    match &bound.arguments {
                        PathArguments::Parenthesized(pararg) => {
                            //pararg.inputs
                            let temp_name = Ident::new(&format!("{}_{}", &name_generator(), arg_name), Span::call_site());
                            emit_closure_helpers(version, temp_name.clone(), pararg, is_mut_ref, extra_definitions, bound.ident.clone());
                            return ArgType::Fn(temp_name, fn_decl, pararg.inputs.iter().map(|x|x.clone()).collect(), is_mut_ref);
                        }
                        _ => {
                            panic!("Fn/FnMut arguments must be enclosed in parenthesis")
                        }
                    }
                } else {
                    return ArgType::TraitReference(bound.ident.clone(), is_mut_ref);
                }
            } else {
                panic!("Method {}, argument {}, reference to trait objects without 'dyn' are not supported.", method_name, arg_name);
            }

        }
        Type::Path(path) => {
            let first_seg = path.path.segments.iter().next().expect("Missing path segments");
            if first_seg.ident == "Box" {
                match &first_seg.arguments {
                    PathArguments::AngleBracketed(ang) => {
                        let first_gen_arg = ang.args.iter().next().expect("Missing generic args of Box");
                        if ang.args.len() != 1 {
                            panic!("Method {}, argument {}. Savefile requires Box arguments to have exactly one generic argument, a requirement not satisfied by type: {:?}", method_name, arg_name, typ);
                        }
                        match first_gen_arg {
                            GenericArgument::Type(angargs) => {
                                match angargs {
                                    Type::TraitObject(trait_obj) => {
                                        if is_reference {
                                            panic!("Method {}, argument {}: Reference to boxed trait object is not supported by savefile. Try using a regular reference to the box content instead.", method_name, arg_name);
                                        }
                                        let type_bounds:Vec<_> = trait_obj.bounds.iter().filter_map(|x|{
                                            match x {
                                                TypeParamBound::Trait(t) => {Some(t.path.segments.iter().last().cloned().expect("Missing bounds of Box trait object").ident.clone())}
                                                TypeParamBound::Lifetime(_) => {None}
                                            }
                                        }).collect();
                                        if type_bounds.len() == 0 {
                                            panic!("Method {}, argument {}, unsupported Box-type. Only Box<dyn Trait> is supported. Encountered zero traits in Box.", method_name, arg_name);
                                        }
                                        if type_bounds.len() > 1 {
                                            panic!("Method {}, argument {}, unsupported Box-type. Only Box<dyn Trait> is supported. Encountered multiple traits in Box: {:?}", method_name, arg_name, trait_obj);
                                        }
                                        if trait_obj.dyn_token.is_none() {
                                            panic!("Method {}, argument {}, unsupported Box-type. Only Box<dyn Trait> is supported.", method_name, arg_name)
                                        }
                                        let bound = type_bounds.into_iter().next().expect("Internal error, missing bounds");
                                        return ArgType::BoxedTrait(bound);
                                    }
                                    _ =>
                                    {
                                        match parse_type(version, arg_name, angargs, method_name, &mut *name_generator, extra_definitions, is_reference, is_mut_ref) {
                                            ArgType::PlainData(_plain) => {
                                                rawtype = path.to_token_stream();
                                            }
                                            _ => { panic!("Method {}, argument {}, unsupported Box-type: {:?}", method_name, arg_name, typ); }
                                        }
                                    }
                                }
                            }
                            _ => {
                                panic!("Method {}, argument {}, unsupported Box-type: {:?}", method_name, arg_name, typ);
                            }
                        }
                    }
                    _ => {
                        panic!("Method {}, argument {}, unsupported Box-type: {:?}", method_name, arg_name, typ);
                    }
                }

            } else {
                rawtype = path.to_token_stream();
            }
        }
        _ => {
            panic!("Method {}, argument {}, unsupported type: {:?}", method_name, arg_name, typ);
        }
    }
    if !is_reference {
        ArgType::PlainData(rawtype)
    } else {
        if is_mut_ref {
            panic!("Method {}, argument {}: Mutable references are not supported by Savefile-abi (except for FnMut-trait objects): {}", method_name, arg_name, typ.to_token_stream());
        }
        ArgType::Reference(rawtype)
    }
}

fn generate_method_definitions(
    version: u32,
    trait_name: Ident,
    method_number: u16,
    method_name: Ident,
    ret_declaration: TokenStream, //May be empty, for ()-returns
    ret_type: TokenStream,
    receiver_is_mut: bool,
    args: Vec<(Ident, &Type)>,
    name_generator: &mut impl FnMut() -> String,
    extra_definitions: &mut Vec<TokenStream>
) -> MethodDefinitionComponents {
    let method_name_str = method_name.to_string();

    let mut callee_trampoline_real_method_invocation_arguments:Vec<TokenStream> = vec![];
    let mut callee_trampoline_variable_declaration = vec![];
    let mut callee_trampoline_temp_variable_declaration = vec![];
    let mut callee_trampoline_variable_deserializer = vec![];
    let mut caller_arg_serializers = vec![];
    let mut caller_fn_arg_list = vec![];
    let mut metadata_arguments = vec![];


    for (arg_index, (arg_name,typ)) in args.iter().enumerate() {

        let argtype = parse_type(version, arg_name, *typ, &method_name, &mut *name_generator, extra_definitions, false, false);

        //let num_mask = 1u64 << (method_number as u64);
        let temp_arg_name = Ident::new(&format!("temp_{}",arg_name), Span::call_site());
        let temp_arg_name2 = Ident::new(&format!("temp2_{}",arg_name), Span::call_site());
        match &argtype {
            ArgType::PlainData(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote!{#arg_name});
            }
            ArgType::Reference(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote!{&#arg_name});
                callee_trampoline_temp_variable_declaration.push(quote!{let #temp_arg_name;});
            }
            ArgType::SliceReference(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote!{&#arg_name});
                callee_trampoline_temp_variable_declaration.push(quote!{let #temp_arg_name;});
            }
            ArgType::BoxedTrait(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote!{#arg_name});
                callee_trampoline_temp_variable_declaration.push(quote!{let #temp_arg_name;});
            }
            ArgType::TraitReference(_,ismut) => {
                callee_trampoline_real_method_invocation_arguments.push(quote!{#arg_name});
                let mutsymbol = if *ismut {quote!(mut)} else {quote!{}};
                callee_trampoline_temp_variable_declaration.push(quote!{let #mutsymbol #temp_arg_name;});
            }
            ArgType::Fn(_,_,_,ismut) => {
                callee_trampoline_real_method_invocation_arguments.push(quote!{#arg_name});
                let mutsymbol = if *ismut {quote!(mut)} else {quote!{}};
                callee_trampoline_temp_variable_declaration.push(quote!{let #mutsymbol #temp_arg_name;});
                callee_trampoline_temp_variable_declaration.push(quote!{let #mutsymbol #temp_arg_name2;});
            }
        }
        callee_trampoline_variable_declaration.push(quote!{let #arg_name;});
        match &argtype {
            ArgType::Reference(arg_type) => {
                callee_trampoline_variable_deserializer.push(quote!{
                                if compatibility_mask&(1<<#arg_index) != 0 {
                                    #arg_name = unsafe { &*(deserializer.read_raw_ptr::<#arg_type>()?) };
                                } else {
                                    #temp_arg_name = <#arg_type as Deserialize>::deserialize(&mut deserializer)?;
                                    #arg_name = &#temp_arg_name;
                                }
                            });
                caller_arg_serializers.push(quote!{
                                if compatibility_mask&(1<<#arg_index) != 0 {
                                    unsafe { serializer.write_raw_ptr(#arg_name as *const #arg_type).expect("Writing argument ref") };
                                } else {
                                    #arg_name.serialize(&mut serializer).expect("Writing argument serialized");
                                }
                            });
            }
            ArgType::SliceReference(arg_type) => {
                callee_trampoline_variable_deserializer.push(quote!{
                                if compatibility_mask&(1<<#arg_index) != 0 {
                                    #arg_name = unsafe { &*(deserializer.read_raw_ptr::<[#arg_type]>()?) };
                                } else {
                                    #temp_arg_name = deserialize_slice_as_vec::<_,#arg_type>(&mut deserializer)?;
                                    #arg_name = &#temp_arg_name;
                                }
                            });
                caller_arg_serializers.push(quote!{
                                if compatibility_mask&(1<<#arg_index) != 0 {
                                    unsafe { serializer.write_raw_ptr(#arg_name as *const [#arg_type]).expect("Writing argument ref") };
                                } else {
                                    #arg_name.serialize(&mut serializer).expect("Writing argument serialized");
                                }
                            });
            }
            ArgType::PlainData(arg_type) => {
                callee_trampoline_variable_deserializer.push(quote!{
                                #arg_name = <#arg_type as Deserialize>::deserialize(&mut deserializer)?;
                            });
                caller_arg_serializers.push(quote!{
                                #arg_name.serialize(&mut serializer).expect("Serializing arg");
                            });

            }
            ArgType::BoxedTrait(trait_type) => {
                callee_trampoline_variable_deserializer.push(quote!{
                                // SAFETY
                                // Todo: Well, why exactly?
                                if compatibility_mask&(1<<#arg_index) == 0 {
                                    panic!("Function arg is not layout-compatible!")
                                }
                                #temp_arg_name = unsafe { PackagedTraitObject::deserialize(&mut deserializer)? };
                                #arg_name = Box::new(unsafe { AbiConnection::from_raw_packaged(#temp_arg_name, Owning::Owned)? } );
                            });
                caller_arg_serializers.push(quote!{
                                if compatibility_mask&(1<<#arg_index) == 0 {
                                    panic!("Function arg is not layout-compatible!")
                                }
                                PackagedTraitObject::new::<dyn #trait_type>(#arg_name).serialize(&mut serializer).expect("PackagedTraitObject");
                            });
            }
            ArgType::TraitReference(trait_type, ismut) => {
                let mutsymbol = if *ismut {quote!{mut}} else {quote!{}};
                let newsymbol = if *ismut {quote!{new_from_ptr}} else {quote!{new_from_ptr}};
                callee_trampoline_variable_deserializer.push(quote!{
                                if compatibility_mask&(1<<#arg_index) == 0 {
                                    panic!("Function arg is not layout-compatible!")
                                }
                                #temp_arg_name = unsafe { AbiConnection::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, Owning::NotOwned)? };
                                #arg_name = & #mutsymbol #temp_arg_name;
                            });
                caller_arg_serializers.push(quote!{
                                if compatibility_mask&(1<<#arg_index) == 0 {
                                    panic!("Function arg is not layout-compatible!")
                                }
                                PackagedTraitObject::#newsymbol::<dyn #trait_type>( unsafe { std::mem::transmute(#arg_name) } ).serialize(&mut serializer).expect("PackagedTraitObject");
                            });


            }
            ArgType::Fn(temp_trait_type, _,args, ismut) => {
                let mutsymbol = if *ismut {quote!{mut}} else {quote!{}};
                let mutorconst = if *ismut {quote!{mut}} else {quote!{const}};
                let newsymbol = if *ismut {quote!{new_from_ptr}} else {quote!{new_from_ptr}};

                let temp_trait_name_wrapper = Ident::new(
                    &format!("{}_wrapper", temp_trait_type), Span::call_site()
                );

                let typedarglist:Vec<TokenStream> = args.iter().enumerate().map(|(idx,typ)|
                    {
                        let id = Ident::new(&format!("x{}", idx), Span::call_site());
                        quote!{#id : #typ}
                    }
                ).collect();

                let arglist:Vec<Ident> = (0..args.len()).map(|idx|
                    {
                        let id = Ident::new(&format!("x{}", idx), Span::call_site());
                        id
                    }
                ).collect();
                callee_trampoline_variable_deserializer.push(quote!{
                        if compatibility_mask&(1<<#arg_index) == 0 {
                            panic!("Function arg is not layout-compatible!")
                        }

                        #temp_arg_name = unsafe { AbiConnection::<#temp_trait_type>::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, Owning::NotOwned)? };
                        #temp_arg_name2 = |#(#typedarglist,)*| {#temp_arg_name.docall(#(#arglist,)*)};
                        #arg_name = & #mutsymbol #temp_arg_name2;
                    });
                caller_arg_serializers.push(quote!{
                        if compatibility_mask&(1<<#arg_index) == 0 {
                            panic!("Function arg is not layout-compatible!")
                        }

                        let #mutsymbol temp = #temp_trait_name_wrapper { func: #arg_name as *#mutorconst _ };
                        let #mutsymbol temp : *#mutorconst (dyn #temp_trait_type+'_) = &#mutsymbol temp as *#mutorconst _;
                        PackagedTraitObject::#newsymbol::<(dyn #temp_trait_type+'_)>( unsafe { std::mem::transmute(temp)} ).serialize(&mut serializer).expect("PackagedTraitObject");
                    });


            }
        }
        match &argtype {
            ArgType::Reference(arg_type) => {
                caller_fn_arg_list.push(quote!{#arg_name : &#arg_type});
                metadata_arguments.push(quote!{
                                AbiMethodArgument {
                                    schema: <#arg_type as WithSchema>::schema(version),
                                    can_be_sent_as_ref: true
                                }
                            })
            }
            ArgType::SliceReference(arg_type) => {

                caller_fn_arg_list.push(quote!{#arg_name : &[#arg_type]});
                metadata_arguments.push(quote!{
                                AbiMethodArgument {
                                    schema: <[#arg_type] as WithSchema>::schema(version),
                                    can_be_sent_as_ref: true
                                }
                            })
            }
            ArgType::PlainData(arg_type) => {
                caller_fn_arg_list.push(quote!{#arg_name : #arg_type});
                metadata_arguments.push(quote!{
                                AbiMethodArgument {
                                    schema: <#arg_type as WithSchema>::schema(version),
                                    can_be_sent_as_ref: false
                                }
                            })
            }
            ArgType::BoxedTrait(trait_name) => {
                caller_fn_arg_list.push(quote!{#arg_name : Box<dyn #trait_name>});
                metadata_arguments.push(quote!{
                                AbiMethodArgument {
                                    schema: Schema::BoxedTrait(<dyn #trait_name as AbiExportable>::get_definition(version)),
                                    can_be_sent_as_ref: true
                                }
                            })
            }
            ArgType::TraitReference(trait_name, ismut) => {
                if *ismut {
                    caller_fn_arg_list.push(quote!{#arg_name : &mut dyn #trait_name });
                } else {
                    caller_fn_arg_list.push(quote!{#arg_name : &dyn #trait_name });
                }

                metadata_arguments.push(quote!{
                                AbiMethodArgument {
                                    schema: Schema::BoxedTrait(<dyn #trait_name as AbiExportable>::get_definition(version)),
                                    can_be_sent_as_ref: true,
                                }
                            })
            }
            ArgType::Fn(temp_trait_name, fndef,_,ismut) => {
                if *ismut {
                    caller_fn_arg_list.push(quote!{#arg_name : &mut dyn #fndef });
                } else {
                    caller_fn_arg_list.push(quote!{#arg_name : &dyn #fndef });
                }
                //let temp_trait_name_str = temp_trait_name.to_string();
                metadata_arguments.push(quote!{
                                {
                                    AbiMethodArgument {
                                        schema: Schema::FnClosure(#ismut, <dyn #temp_trait_name as AbiExportable >::get_definition(version)),
                                        can_be_sent_as_ref: true,
                                    }
                                }
                            })

            }
        }
    }

    let callee_real_method_invocation_except_args;
    if receiver_is_mut {
        callee_real_method_invocation_except_args = quote!{ unsafe { &mut *trait_object.as_mut_ptr::<dyn #trait_name>() }.#method_name };
    } else {
        callee_real_method_invocation_except_args = quote!{ unsafe { &*trait_object.as_const_ptr::<dyn #trait_name>() }.#method_name };
    }

    //let receiver_mut_str = receiver_mut.to_string();
    let receiver_mut = if receiver_is_mut {quote!(mut)} else {quote!{}};
    let caller_method_trampoline = quote!{
        fn #method_name(& #receiver_mut self, #(#caller_fn_arg_list,)*) #ret_declaration {
            let info: &AbiConnectionMethod = &self.template.methods[#method_number as usize];

            let Some(callee_method_number) = info.callee_method_number else {
                panic!("Method '{}' does not exist in implementation.", info.method_name);
            };

            let mut result_buffer: MaybeUninit<Result<#ret_type,SavefileError>> = MaybeUninit::<Result<#ret_type,SavefileError>>::uninit();
            let compatibility_mask = info.compatibility_mask;

            let mut data = FlexBuffer::new();
            let mut serializer = Serializer {
                writer: &mut data,
                file_version: self.template.effective_version,
            };
            serializer.write_u32(self.template.effective_version).unwrap();
            #(#caller_arg_serializers)*

            (self.template.entry)(AbiProtocol::RegularCall {
                trait_object: self.trait_object,
                compatibility_mask: compatibility_mask,
                method_number: callee_method_number,
                effective_version: self.template.effective_version,
                data: data.as_ptr() as *const u8,
                data_length: data.len(),
                abi_result: &mut result_buffer as *mut MaybeUninit<Result<#ret_type,SavefileError>> as *mut (),
                receiver: abi_result_receiver::<#ret_type>,
            });
            let resval = unsafe { result_buffer.assume_init() };

            resval.expect("Unexpected panic in invocation target")
        }
    };

    let method_metadata = quote!{
        AbiMethod {
            name: #method_name_str.to_string(),
            info: AbiMethodInfo {
                return_value: <#ret_type as WithSchema>::schema(version),
                arguments: vec![ #(#metadata_arguments,)* ],
            }
        }
    };

    let callee_method_trampoline = quote!{
                    #method_number => {
                        #(#callee_trampoline_variable_declaration)*
                        #(#callee_trampoline_temp_variable_declaration)*

                        #(#callee_trampoline_variable_deserializer)*

                        let ret = #callee_real_method_invocation_except_args( #(#callee_trampoline_real_method_invocation_arguments,)* );

                        let mut slow_temp = FlexBuffer::new();
                        let mut serializer = Serializer {
                            writer: &mut slow_temp,
                            file_version: #version,
                        };
                        serializer.write_u32(effective_version)?;
                        match ret.serialize(&mut serializer)
                        {
                            Ok(()) => {
                                let outcome = RawAbiCallResult::Success {data: slow_temp.as_ptr(), len: slow_temp.len()};
                                receiver(&outcome as *const _, abi_result);
                            }
                            Err(err) => {
                                let err_str = format!("{:?}", err);
                                let outcome = RawAbiCallResult::AbiError(AbiErrorMsg{error_msg_utf8: err_str.as_ptr(), len: err_str.len()});
                                receiver(&outcome as *const _, abi_result)
                            }
                        }

                    }

                };
    MethodDefinitionComponents {
        method_metadata,
        callee_method_trampoline,
        caller_method_trampoline,
    }
}

#[proc_macro_attribute]
pub fn savefile_abi_exportable(attr: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed: ItemTrait = syn::parse(input.clone()).expect("Expected valid rust-code");

    let mut version = None;
    for item in attr.to_string().split(',') {
        let keyvals : Vec<_> = item.split('=').collect();
        if keyvals.len() != 2 {
            panic!("savefile_abi_exportable arguments should be of form #[savefile_abi_exportable(version=0)], not '{}'", attr);
        }
        let key = keyvals[0].trim();
        let val = keyvals[1].trim();
        match key {
            "version" => {
                if version.is_some() {
                    panic!("version specified more than once");
                }
                version = Some(val.parse().expect(&format!("Version must be numeric, but was: {}", val)));
            }
            _ => panic!("Unknown savefile_abi_exportable key: '{}'", key)
        }
    }
    let version:u32 = version.unwrap_or(0);



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



    let mut method_metadata:Vec<TokenStream> = vec![];
    let mut callee_method_trampoline:Vec<TokenStream> = vec![];
    let mut caller_method_trampoline = vec![];
    let mut extra_definitions = vec![];


    for (method_number,item) in parsed.items.iter().enumerate() {

        if method_number > u16::MAX.into() {
            panic!("Savefile only supports 2^16 methods per interface. Sorry.");
        }
        let method_number = method_number as u16;


        match item {
            TraitItem::Const(c) => {
                panic!("savefile_abi_exportable does not support associated consts: {}",c.ident);
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
                        ret_type = Tuple(TypeTuple{
                            paren_token: Paren::default(),
                            elems: Default::default(),
                        }).to_token_stream();
                        ret_declaration = quote!{}
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
                            Type::Tuple(TypeTuple{elems, ..}) => {
                                if elems.len() > 3 {
                                    panic!("Savefile presently only supports tuples up to 3 members. Either change to using a struct, or file an issue on savefile!");
                                }
                                // Empty tuple!
                                ret_type = ty.to_token_stream();
                                ret_declaration = quote! { -> #ret_type }
                            }
                            _ => panic!("Unsupported type in return-position: {:?}", ty)
                        }
                    }
                }



                let self_arg = method.sig.inputs.iter().next().expect(&format!("Method {} has no arguments. This is not supported - it must at least have a self-argument.", method_name));
                if let FnArg::Receiver(recv) = self_arg {
                    if let Some(reference) = &recv.reference {
                        if reference.1.is_some() {
                            panic!("Method {} has a lifetime for 'self' argument. This is not supported", method_name);
                        }
                        if recv.mutability.is_some() {
                            receiver_is_mut = true;
                        }
                    } else {
                        panic!("Method {} takes 'self' by value. This is not supported. Use &self", method_name);
                    }
                } else {
                    panic!("Method {} must have 'self'-parameter", method_name);
                }
                let mut args = Vec::with_capacity(method.sig.inputs.len());
                for (arg_index,arg) in method.sig.inputs.iter().enumerate().skip(1) {
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
                let mut temp_name_generator = move||{
                    current_name_index += 1;
                    format!("{}_{}",name_baseplate, current_name_index)
                };

                let method_defs = generate_method_definitions(version, trait_name.clone(), method_number, method_name, ret_declaration, ret_type, receiver_is_mut, args, &mut temp_name_generator, &mut extra_definitions);
                method_metadata.push(method_defs.method_metadata);
                callee_method_trampoline.push(method_defs.callee_method_trampoline);
                caller_method_trampoline.push(method_defs.caller_method_trampoline);


            }
            TraitItem::Type(t) => {
                panic!("savefile_abi_exportable does not support associated types: {}",t.ident);
            }
            TraitItem::Macro(m) => {
                panic!("savefile_abi_exportable does not support macro items: {:?}", m);
            }
            x => panic!("Unsupported item in trait definition: {:?}", x)
        }

    }


    let abi_entry_light = Ident::new(&format!("abi_entry_light_{}",trait_name_str), Span::call_site());

    let exports_for_trait = quote!{
        
        pub extern "C" fn #abi_entry_light(flag: AbiProtocol) {
            unsafe { abi_entry_light::<dyn #trait_name>(flag); }
        }

        unsafe impl AbiExportable for dyn #trait_name {
            const ABI_ENTRY : extern "C" fn (flag: AbiProtocol)  = #abi_entry_light;
            fn get_definition( version: u32) -> AbiTraitDefinition {
                AbiTraitDefinition {
                    name: #trait_name_str.to_string(),
                    methods: vec! [ #(#method_metadata,)* ]
                }
            }

            fn get_latest_version() -> u32 {
                #version
            }

            fn call(trait_object: TraitObject, method_number: u16, effective_version:u32, compatibility_mask: u64, data: &[u8], abi_result: *mut (), receiver: extern "C" fn(outcome: *const RawAbiCallResult, result_receiver: *mut ()/*Result<T,SaveFileError>>*/)) -> Result<(),SavefileError> {

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
    // std::fs::write(format!("/home/anders/savefile/savefile-abi-min-lib/src/{}.rs",trait_name_str),expanded.to_string()).unwrap();

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

    let s = savefile_derive_crate_serialize(input.clone());

    let d = savefile_derive_crate_deserialize(input.clone());

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

    let s = savefile_derive_crate_serialize(input.clone());

    let d = savefile_derive_crate_deserialize(input.clone());

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

fn savefile_derive_crate_deserialize(input: DeriveInput) -> TokenStream {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();

    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause,quote!{_savefile::prelude::Deserialize + _savefile::prelude::ReprC});



    let deserialize = quote_spanned! {defspan=>
        _savefile::prelude::Deserialize
    };

    let uses = quote_spanned! { defspan =>
        extern crate savefile as _savefile;
    };

    let deserializer = quote_spanned! {defspan=>
        _savefile::prelude::Deserializer<impl std::io::Read>
    };

    let saveerr = quote_spanned! {defspan=>
        _savefile::prelude::SavefileError
    };

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut output = Vec::new();
            //let variant_count = enum1.variants.len();
            let enum_size = get_enum_size(&input.attrs,enum1.variants.len());

            for (var_idx_usize, variant) in enum1.variants.iter().enumerate() {
                let var_idx = Literal::u32_unsuffixed(var_idx_usize as u32);

                let var_ident = variant.ident.clone();
                let variant_name = quote! { #name::#var_ident };
                let variant_name_spanned = quote_spanned! { span => #variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        let field_infos: Vec<FieldInfo> = fields_named
                            .named
                            .iter()
                            .enumerate()
                            .map(|(field_index,field)| FieldInfo {
                                ident: Some(field.ident.clone().expect("Expected identifier [6]")),
                                ty: &field.ty,
                                index: field_index as u32,
                                attrs: &field.attrs,
                            })
                            .collect();

                        let fields_deserialized = implement_deserialize(field_infos);

                        output.push(quote!( #var_idx => #variant_name_spanned{ #(#fields_deserialized,)* } ));
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        let field_infos: Vec<FieldInfo> = fields_unnamed
                            .unnamed
                            .iter()
                            .enumerate()
                            .map(|(field_index,field)| FieldInfo {
                                ident: None,
                                ty: &field.ty,
                                index: field_index as u32,
                                attrs: &field.attrs,
                            })
                            .collect();
                        let fields_deserialized = implement_deserialize(field_infos);

                        output.push(quote!( #var_idx => #variant_name_spanned( #(#fields_deserialized,)*) ));
                    }
                    &syn::Fields::Unit => {
                        output.push(quote!( #var_idx => #variant_name_spanned ));
                    }
                }
            }

            let variant_deserializer = match enum_size.discriminant_size {
                1 => quote! { deserializer.read_u8()?  },
                2 => quote! { deserializer.read_u16()?  },
                4 => quote! { deserializer.read_u32()?  },
                _ => unreachable!(),
            };

            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses
                    impl #impl_generics #deserialize for #name #ty_generics #where_clause #extra_where {
                        #[allow(unused_comparisons, unused_variables)]
                        fn deserialize(deserializer: &mut #deserializer) -> Result<Self,#saveerr> {

                            Ok(match #variant_deserializer {
                                #(#output,)*
                                _ => return Err(_savefile::prelude::SavefileError::GeneralError{msg:format!("Corrupt file - unknown enum variant detected.")})
                            })
                        }
                    }
                };
            }
        }
        &syn::Data::Struct(ref struc) => {
            let output = match &struc.fields {
                &syn::Fields::Named(ref namedfields) => {
                    let field_infos: Vec<FieldInfo> = namedfields
                        .named
                        .iter()
                        .enumerate()
                        .map(|(field_index,field)| FieldInfo {
                            ident: Some(field.ident.clone().expect("Expected identifier[7]")),
                            index: field_index as u32,
                            ty: &field.ty,
                            attrs: &field.attrs,
                        })
                        .collect();

                    let output1 = implement_deserialize(field_infos);
                    quote! {Ok(#name {
                        #(#output1,)*
                    })}
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos: Vec<FieldInfo> = fields_unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(field_index,field)| FieldInfo {
                            ident: None,
                            index: field_index as u32,
                            ty: &field.ty,
                            attrs: &field.attrs,
                        })
                        .collect();
                    let output1 = implement_deserialize(field_infos);

                    quote! {Ok(#name (
                        #(#output1,)*
                    ))}
                }
                &syn::Fields::Unit => {
                    quote! {Ok(#name )}
                } //_ => panic!("Only regular structs supported, not tuple structs."),
            };
            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                        #uses
                        impl #impl_generics #deserialize for #name #ty_generics #where_clause #extra_where {
                        #[allow(unused_comparisons, unused_variables)]
                        fn deserialize(deserializer: &mut #deserializer) -> Result<Self,#saveerr> {
                            #output
                        }
                    }
                };
            }
        }
        _ => {
            panic!("Only regular structs are supported");
        }
    };

    expanded
}
#[allow(non_snake_case)]
fn implement_reprc_hardcoded_false(name: syn::Ident, generics: syn::Generics) -> TokenStream {
    let defspan = proc_macro2::Span::call_site();

    let dummy_const = syn::Ident::new("_", proc_macro2::Span::call_site());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause,quote!{_savefile::prelude::WithSchema});
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
fn implement_reprc(field_infos: Vec<FieldInfo>, generics: syn::Generics, name: syn::Ident, expect_fast: bool) -> TokenStream {
    let generics = generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause,quote!{_savefile::prelude::ReprC});

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
            if expect_fast{
                panic!("The #[savefile_unsafe_and_fast] attribute cannot be used for structures containing ignored fields");
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
                                "u8" =>  { size_u8 = Some(1); have_seen_explicit_size = true; },
                                "i8" =>  { size_u8 = Some(1); have_seen_explicit_size = true; },
                                "u16" => { size_u8 = Some(2); have_seen_explicit_size = true; },
                                "i16" => { size_u8 = Some(2); have_seen_explicit_size = true; },
                                "u32" => { size_u8 = Some(4); have_seen_explicit_size = true; },
                                "i32" => { size_u8 = Some(4); have_seen_explicit_size = true; },
                                "u64" |
                                "i64" => panic!("Savefile does not support enums with more than 2^32 variants."),
                                _ => panic!("Unsupported repr(X) attribute on enum: {}", size_str),
                            }
                        }
                    }
                }
            }
        }
    }
    let discriminant_size = size_u8.unwrap_or_else(||{
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
            Ok(ref meta) => {
                match meta {
                    &syn::Meta::Path(ref x) => {
                        let x = path_to_string(x);
                        if x == "savefile_unsafe_and_fast" {
                            opt_in_fast = true;
                        }
                    }
                    _ => {}
                }
            }
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
                    &syn::Fields::Unit => {
                    }
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
                    .map(|(field_index,field)| FieldInfo {
                        ident: Some(field.ident.clone().expect("Expected identifier [8]")),
                        index: field_index as u32,
                        ty: &field.ty,
                        attrs: &field.attrs,
                    })
                    .collect();

                implement_reprc(field_infos, input.generics, name,opt_in_fast)
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
            &syn::Fields::Unit =>
                implement_reprc(Vec::new(), input.generics, name,opt_in_fast),
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
    let extra_where = get_extra_where_clauses(&generics, where_clause,quote!{_savefile::prelude::Introspect});


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
            for (_var_idx, variant) in enum1.variants.iter().enumerate() {
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
                        .map(|(idx,field)| FieldInfo {
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
                        .map(|(idx,f)| FieldInfo {
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
fn implement_withschema(structname: &str, field_infos: Vec<FieldInfo>, is_enum: FieldOffsetStrategy, ty_generics: &TypeGenerics) -> Vec<TokenStream> {
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
    EnumWithKnownOffsets(usize/*variant index*/),
    EnumWithUnknownOffsets
}

#[allow(non_snake_case)]
fn savefile_derive_crate_withschema(input: DeriveInput) -> TokenStream {


    //let mut have_u8 = false;



    //let discriminant_size = discriminant_size.expect("Enum discriminant must be u8, u16 or u32. Use for example #[repr(u8)].");

    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(&generics, where_clause,quote!{_savefile::prelude::WithSchema});

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
            let max_variant_fields = enum1.variants.iter().map(|x|x.fields.len()).max().unwrap_or(0);

            let enum_size = get_enum_size(&input.attrs, enum1.variants.len());
            let need_determine_offsets =
                (max_variant_fields == 0 && enum_size.explicit_size) || (enum_size.explicit_size && enum_size.repr_c);


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

                let verinfo = parse_attr_tag2(&variant.attrs, false);
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
                            let field_name = f.ident.as_ref().expect("Enum variant with named fields *must* actually have a name").clone();
                            field_offset_extractors.push(quote!(unsafe { (#field_name as *const _ as *const u8).offset_from(base_ptr) as usize }));
                            field_pattern.push(field_name);
                            field_infos.push(FieldInfo {
                                ident: Some(f.ident.clone().expect("Expected identifier[1]")),
                                ty: &f.ty,
                                index: idx as u32,
                                attrs: &f.attrs,
                            });
                        }
                        offset_extractor_match_clause = quote!{#name::#var_ident { #(#field_pattern,)* } };
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        let mut field_pattern = vec![];
                        for (idx,f) in fields_unnamed.unnamed.iter().enumerate() {
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
                        offset_extractor_match_clause = quote!{#name::#var_ident ( #(#field_pattern,)* ) };
                    }
                    &syn::Fields::Unit => {
                        offset_extractor_match_clause = quote!{#name::#var_ident};
                        //No fields
                    }
                }
                while field_offset_extractors.len() < max_variant_fields {
                    field_offset_extractors.push(quote!{0});
                }

                variant_field_offset_extractors.push(quote!{
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

            let field_offset_impl ;
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
                field_offset_impl = quote!{};
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
                        .map(|(idx,field)| FieldInfo {
                            ident: Some(field.ident.clone().expect("Expected identifier[2]")),
                            ty: &field.ty,
                            index: idx as u32,
                            attrs: &field.attrs,
                        })
                        .collect();

                    fields = implement_withschema(&name.to_string(), field_infos, FieldOffsetStrategy::Struct, &ty_generics);
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos: Vec<FieldInfo> = fields_unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(idx,f)| FieldInfo {
                            ident: None,
                            index: idx as u32,
                            ty: &f.ty,
                            attrs: &f.attrs,
                        })
                        .collect();
                    fields = implement_withschema(&name.to_string(), field_infos, FieldOffsetStrategy::Struct, &ty_generics);
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
