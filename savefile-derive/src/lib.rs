#![recursion_limit = "128"]
#![deny(warnings)]
//! This crate allows automatic derivation of the Savefile-traits: Serialize, Deserialize, WithSchema, ReprC and Introspect .
//! The documentatino for this is found in the Savefile crate documentation.

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
extern crate syn;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use std::iter::IntoIterator;
use syn::{DeriveInput, Expr, GenericParam, Generics, Ident, Lit, Type, WhereClause};
use syn::__private::bool;

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

fn check_is_remove(field_type: &syn::Type) -> bool {
    use quote::ToTokens;
    let mut is_remove = false;
    let mut tokens = TokenStream::new();
    field_type.to_tokens(&mut tokens);
    for tok in tokens.into_iter() {
        if tok.to_string() == "Removed" {
            //TODO: This is not robust, since it's based on text matching
            is_remove = true;
        }
    }
    is_remove
}

fn parse_attr_tag(attrs: &Vec<syn::Attribute>) -> AttrsResult {
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
    path.segments.last().unwrap().ident.to_string()
}

fn parse_attr_tag2(attrs: &Vec<syn::Attribute>, _is_string_default_val: bool) -> AttrsResult {
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
                    &syn::Meta::Path(ref x) => {
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
                                    default_val = Some(quote! { str::parse(#litstr).unwrap() })
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
                                        litstr2.value().splitn(3, ":").map(|x| x.to_string()).collect();
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
                                    if field_to_version.unwrap() < field_from_version.unwrap() {
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
        default_fn: default_fn,
        default_val: default_val,
        ignore: ignore,
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
            compile_time_check_reprc(&*x.elem)
        }
        Type::Tuple(t) => {
            let mut size = None;
            for x in &t.elems {
                if !compile_time_check_reprc(&x)
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

fn implement_fields_serialize<'a>(
    field_infos: Vec<FieldInfo<'a>>,
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
            let deferred_from = iter.next().unwrap();
            let deferred_to = iter.last().unwrap_or(deferred_from.clone());

            output.push(
                quote!(
                    unsafe {
                        if #(#conditions)* {
                         #local_serializer.raw_write_region(self,&#deferred_from,&#deferred_to, local_serializer.version)?;
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
                span: span,
            };
            quote! { self.#id}
        } else {
            let id = field.ident.clone().unwrap();
            if implicit_self {
                quote! { self.#id}
            } else {
                quote! { *#id}
            }
        };
        objid
    };



    for ref field in &field_infos {
        {
            let verinfo = parse_attr_tag(&field.attrs);

            if verinfo.ignore {
                continue;
            }
            let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

            let removed = check_is_remove(&field.ty);

            let type_size_align = compile_time_size(&field.ty);
            let compile_time_reprc = compile_time_check_reprc(&field.ty) && type_size_align.is_some();

            let obj_id = get_obj_id(field);

            if field_from_version == 0 && field_to_version == std::u32::MAX {
                if removed {
                    panic!(
                        "The Removed type can only be used for removed fields. Use the savefile_versions attribute."
                    );
                }

                if compile_time_reprc {
                    let (_cursize, curalign) = type_size_align.unwrap();
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
                if #local_serializer.version >= #field_from_version && #local_serializer.version <= #field_to_version {
                    <_ as _savefile::prelude::Serialize>::serialize(&#obj_id, #local_serializer)?;
                }));
            }
        }
    }
    realize_any_deferred(&local_serializer, &mut deferred_reprc, &mut output);

    //let contents = format!("//{:?}",output);

    let total_reprc_opt: TokenStream;
    if field_infos.is_empty() == false {
        let first_field = get_obj_id(field_infos.first().unwrap());
        let last_field = get_obj_id(field_infos.last().unwrap());
        total_reprc_opt = quote!( unsafe { #local_serializer.raw_write_region(self,&#first_field, &#last_field, local_serializer.version)?; } );
    } else {
        total_reprc_opt = quote!( );
    }

    let serialize2 = quote! {
        let local_serializer = serializer;

        if #min_safe_version > local_serializer.version {
                panic!("Version ranges on fields must not include memory schema version. Field version: {}, memory version: {}",
                    #min_safe_version.saturating_sub(1), local_serializer.version);
            }

        if unsafe { <Self as #reprc>::repr_c_optimization_safe(local_serializer.version).is_yes() } {
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
    let magic = format!("_IMPL_SAVEFILE_SERIALIZE_FOR_{}", &name).to_string();
    let dummy_const = syn::Ident::new(&magic, proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut output = Vec::new();
            let variant_count = enum1.variants.len();
            if variant_count > 255 {
                panic!("This library is not capable of serializing enums with more than 255 variants. Our deepest apologies, we thought no-one would ever create such an enum!");
            }

            for (var_idx, ref variant) in enum1.variants.iter().enumerate() {
                let var_idx = var_idx as u8;
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
                                ident: Some(field.ident.clone().unwrap()),
                                index: field_index as u32,
                                ty: &field.ty,
                                attrs: &field.attrs,
                            })
                            .collect();

                        let (fields_serialized, fields_names) = implement_fields_serialize(field_infos, false, false);
                        output.push(quote!( #variant_name_spanned{#(#fields_names,)*} => { 
                                serializer.write_u8(#var_idx)?; 
                                #fields_serialized 
                            } ));
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        let field_infos: Vec<FieldInfo> = fields_unnamed
                            .unnamed
                            .iter()
                            .enumerate()
                            .map(|(idx, field)| FieldInfo {
                                ident: Some(syn::Ident::new(
                                    &("x".to_string() + &idx.to_string()),
                                    Span::call_site(),
                                )),
                                index: idx as u32,
                                ty: &field.ty,
                                attrs: &field.attrs,
                            })
                            .collect();

                        let (fields_serialized, fields_names) = implement_fields_serialize(field_infos, false, false);

                        output.push(
                            quote!( #variant_name_spanned(#(#fields_names,)*) => { serializer.write_u8(#var_idx)?; #fields_serialized  } ),
                        );
                    }
                    &syn::Fields::Unit => {
                        output.push(quote!( #variant_name_spanned => { serializer.write_u8(#var_idx)? } ));
                    }
                }
            }
            quote! {
                #[allow(non_upper_case_globals)]
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
                            ident: Some(field.ident.clone().unwrap()),
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
    let local_deserializer = quote_spanned! { defspan => deserializer};

    let mut output = Vec::new();
    let mut min_safe_version = 0;
    for ref field in &field_infos {
        let field_type = &field.ty;

        let is_removed = check_is_remove(field_type);

        let verinfo = parse_attr_tag(&field.attrs);
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

        let effective_default_val = if is_removed {
            quote! { #removeddef::new() }
        } else if let Some(defval) = default_val {
            quote! { #defval } //str::parse(#defval).unwrap() }
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
            if is_removed {
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
    let input: DeriveInput = syn::parse(input).unwrap();

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
    let input: DeriveInput = syn::parse(input).unwrap();

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
    let input: DeriveInput = syn::parse(input).unwrap();

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

    let magic = format!("_IMPL_SAVEFILE_DESERIALIZE_FOR_{}", &name).to_string();
    let dummy_const = syn::Ident::new(&magic, proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut output = Vec::new();
            let variant_count = enum1.variants.len();
            if variant_count > 255 {
                panic!("This library is not capable of deserializing enums with more than 255 variants. Our deepest apologies, we thought no-one would ever use more than 255 variants!");
            }

            for (var_idx, ref variant) in enum1.variants.iter().enumerate() {
                let var_idx = var_idx as u8;
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
                                ident: Some(field.ident.clone().unwrap()),
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

            quote! {
                #[allow(non_upper_case_globals)]
                const #dummy_const: () = {
                    #uses
                    impl #impl_generics #deserialize for #name #ty_generics #where_clause #extra_where {
                        #[allow(unused_comparisons, unused_variables)]
                        fn deserialize(deserializer: &mut #deserializer) -> Result<Self,#saveerr> {

                            Ok(match deserializer.read_u8()? {
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
                            ident: Some(field.ident.clone().unwrap()),
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

    let magic = format!("_IMPL_SAVEFILE_REPRC_FOR_{}", &name).to_string();
    let dummy_const = syn::Ident::new(&magic, proc_macro2::Span::call_site());
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
    let magic = format!("_IMPL_SAVEFILE_REPRC_FOR_{}", &name).to_string();
    let dummy_const = syn::Ident::new(&magic, proc_macro2::Span::call_site());

    for field in field_infos.windows(2) {
        let field_name1 = field[0].ident.as_ref().expect("Field was expected to have a name");
        let field_name2 = field[1].ident.as_ref().expect("Field was expected to have a name");
        optsafe_outputs.push(quote!( (#spanof!(#name #ty_generics, #field_name1).end == #spanof!(#name #ty_generics, #field_name2).start )));

    }
    if field_infos.len() > 0 {
        if field_infos.len() == 1 {
            optsafe_outputs.push(quote!(  (#spanof!( #name #ty_generics, ..).start == 0 )));
            optsafe_outputs.push(quote!(  (#spanof!( #name #ty_generics, ..).end == std::mem::size_of::<Self>() )));

        } else {
            let first = field_infos.first().unwrap().ident.as_ref().unwrap();
            let last = field_infos.last().unwrap().ident.as_ref().unwrap();
            optsafe_outputs.push(quote!( (#spanof!(#name #ty_generics, #first).start == 0 )));
            optsafe_outputs.push(quote!( (#spanof!(#name #ty_generics,#last).end == std::mem::size_of::<Self>() )));
        }

    }

    for ref field in &field_infos {
        let verinfo = parse_attr_tag(&field.attrs);
        if verinfo.ignore {
            if expect_fast{
                panic!("The #[savefile_unsafe_and_fast] attribute cannot be used for structures containing ignored fields");
            } else {
                return implement_reprc_hardcoded_false(name, generics);
            }
        }
        let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

        let removed = check_is_remove(&field.ty);
        let field_type = &field.ty;
        if field_from_version == 0 && field_to_version == std::u32::MAX {
            if removed {
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

            if !removed {
                optsafe_outputs.push(
                    quote_spanned!( span => <#field_type as #reprc>::repr_c_optimization_safe(#local_file_version).is_yes()),
                );
            }
        }
    }


    quote! {

        #[allow(non_upper_case_globals)]
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

fn get_enum_size(attrs: &Vec<syn::Attribute>) -> Option<u32> {

    let mut size_u32: Option<u32> = None;
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
                                    &syn::Meta::Path(ref path) => path_to_string(&path),
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
                            size_u32 = match size_str.as_ref() {
                                "u8" => Some(1),
                                "i8" => Some(1),
                                "u16" => Some(2),
                                "i16" => Some(2),
                                "u32" => Some(4),
                                "i32" => Some(4),
                                "u64" => Some(8),
                                "i64" => Some(8),
                                _ => panic!("Unsupported repr(X) attribute on enum: {}", size_str),
                            }
                        }
                    }
                }
            }
        }
    }
    size_u32
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

    panic!("The #[derive(ReprC)] style of unsafe performance opt-in has been changed. Add a #[savefile_unsafe_and_fast] attribute on a new line, after the #[derive(Savefile)] attribute instead.")
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
            if enum1.variants.len() > 256 {
                if opt_in_fast {
                    panic!("The #[savefile_unsafe_and_fast] attribute assumes that the enum representation is u8 or i8. Savefile does not support enums with more than 256 variants. Sorry.");
                }
                return implement_reprc_hardcoded_false(name, input.generics);
            }
            let enum_size = get_enum_size(&input.attrs);
            if let Some(enum_size) = enum_size {
                if enum_size != 1 {
                    if opt_in_fast {
                        panic!("The #[savefile_unsafe_and_fast] attribute assumes that the enum representation is u8 or i8. Savefile does not support enums with more than 256 variants. Sorry.");
                    }
                    return implement_reprc_hardcoded_false(name, input.generics);
                }
            }

            let field_infos = Vec::<FieldInfo>::new();
            for ref variant in enum1.variants.iter() {
                match &variant.fields {
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
                        if enum_size.is_none() {
                            if opt_in_fast {
                                panic!("Enums which use the #[savefile_unsafe_and_fast] attribute must specify the enum size using the repr-attribute, like #[repr(u8)].");
                            }
                            return implement_reprc_hardcoded_false(name, input.generics);
                        }
                    }
                }
            }
            implement_reprc(field_infos, input.generics, name,opt_in_fast)
        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {
                let field_infos: Vec<FieldInfo> = namedfields
                    .named
                    .iter()
                    .enumerate()
                    .map(|(field_index,field)| FieldInfo {
                        ident: Some(field.ident.clone().unwrap()),
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
                        ident: Some(syn::Ident::new(
                            &("x".to_string() + &idx.to_string()),
                            Span::call_site(),
                        )),
                        index: idx as u32,
                        ty: &field.ty,
                        attrs: &field.attrs,
                    })
                    .collect();

                if field_infos.len() == 1 {
                    implement_reprc(field_infos, input.generics, name,opt_in_fast)
                } else {
                    if opt_in_fast {
                        panic!("Enums with unnamed fields don't support #[savefile_unsafe_and_fast] attribute.");
                    }
                    return implement_reprc_hardcoded_false(name, input.generics);
                }
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

    expanded.into()
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
    let introspect_item = quote_spanned! {defspan=>
        _savefile::prelude::introspect_item
    };

    let mut fields = Vec::new();
    let mut fields_names = Vec::new();
    let mut introspect_key = None;
    let mut index_number = 0usize;
    for (idx, ref field) in field_infos.iter().enumerate() {
        let verinfo = parse_attr_tag(&field.attrs);
        if verinfo.introspect_key {
            if introspect_key.is_some() {
                panic!("Type had more than one field with savefile_introspect_key - attribute");
            }
        }
        if verinfo.introspect_ignore {
            continue;
        }
        if need_self {
            let fieldname;
            let fieldname_raw;

            if let Some(id) = field.ident.clone() {
                fieldname = quote! {&self.#id};
                fieldname_raw = quote! {#id};
            } else {
                let idd = syn::Index::from(idx);
                fieldname = quote! {&self.#idd};
                fieldname_raw = quote! {#idd};
            }
            fields.push(quote_spanned!( span => if #index1 == #index_number { return Some(#introspect_item(stringify!(#fieldname_raw).to_string(), #fieldname))}));
            if verinfo.introspect_key {
                let fieldname_raw2 = fieldname_raw.clone();
                introspect_key = Some(quote! {self.#fieldname_raw2});
            }
            fields_names.push(fieldname_raw);
        } else {
            if let Some(id) = field.ident.clone() {
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

    let magic = format!("_IMPL_SAVEFILE_INTROSPECT_FOR_{}", &name).to_string();
    let dummy_const = syn::Ident::new(&magic, proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut variants = Vec::new();
            let mut value_variants = Vec::new();
            let mut len_variants = Vec::new();
            for (var_idx, ref variant) in enum1.variants.iter().enumerate() {
                if var_idx > 255 {
                    panic!("Savefile does not support enums with more than 255 total variants. Sorry.");
                }
                //let var_idx = var_idx as u8;
                let var_ident = variant.ident.clone();
                let variant_name = quote! { #var_ident };
                let variant_name_spanned = quote_spanned! { span => #variant_name};

                let mut field_infos = Vec::new();

                let return_value_name_str = format!("{}::{}", name, var_ident.to_string());
                let return_value_name = quote!(#return_value_name_str);
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        for (idx, f) in fields_named.named.iter().enumerate() {
                            field_infos.push(FieldInfo {
                                ident: Some(f.ident.clone().unwrap()),
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
                            ident: Some(field.ident.clone().unwrap()),
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
fn implement_withschema(field_infos: Vec<FieldInfo>) -> Vec<TokenStream> {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let local_version = quote_spanned! { defspan => local_version};
    let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    let WithSchema = quote_spanned! { defspan => _savefile::prelude::WithSchema };
    let fields1 = quote_spanned! { defspan => fields1 };

    let mut fields = Vec::new();
    for (idx, ref field) in field_infos.iter().enumerate() {
        let verinfo = parse_attr_tag(&field.attrs);
        if verinfo.ignore {
            continue;
        }
        let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

        let name = if let Some(name) = field.ident.clone() {
            (&name).to_string()
        } else {
            idx.to_string()
        };
        let removed = check_is_remove(&field.ty);
        let field_type = &field.ty;
        if field_from_version == 0 && field_to_version == std::u32::MAX {
            if removed {
                panic!("The Removed type can only be used for removed fields. Use the savefile_version attribute.");
            }
            fields.push(quote_spanned!( span => #fields1.push(#Field { name:#name.to_string(), value:Box::new(<#field_type as #WithSchema>::schema(#local_version))})));
        } else {
            let mut version_mappings = Vec::new();
            for dt in verinfo.deserialize_types.iter() {
                let dt_from = dt.from;
                let dt_to = dt.to;
                let dt_field_type = syn::Ident::new(&dt.serialized_type, span);
                version_mappings.push(quote!{
                    if #local_version >= #dt_from && local_version <= #dt_to {
                        #fields1.push(#Field { name:#name.to_string(), value:Box::new(<#dt_field_type as #WithSchema>::schema(#local_version))});
                    }
                });
            }

            fields.push(quote_spanned!( span =>
                #(#version_mappings)*

                if #local_version >= #field_from_version && #local_version <= #field_to_version {
                    #fields1.push(#Field { name:#name.to_string(), value:Box::new(<#field_type as #WithSchema>::schema(#local_version))});
                }
                ));
        }
    }
    fields
}

#[allow(non_snake_case)]
fn savefile_derive_crate_withschema(input: DeriveInput) -> TokenStream {
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
    };

    let SchemaStruct = quote_spanned! { defspan => _savefile::prelude::SchemaStruct };
    let SchemaEnum = quote_spanned! { defspan => _savefile::prelude::SchemaEnum };
    let Schema = quote_spanned! { defspan => _savefile::prelude::Schema };
    let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    let Variant = quote_spanned! { defspan => _savefile::prelude::Variant };

    let magic = format!("_IMPL_SAVEFILE_WITHSCHEMA_FOR_{}", &name).to_string();
    let dummy_const = syn::Ident::new(&magic, proc_macro2::Span::call_site());

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut variants = Vec::new();
            for (var_idx, ref variant) in enum1.variants.iter().enumerate() {
                if var_idx > 255 {
                    panic!("Savefile does not support enums with more than 255 total variants. Sorry.");
                }
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

                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        for (idx,f) in fields_named.named.iter().enumerate() {
                            field_infos.push(FieldInfo {
                                ident: Some(f.ident.clone().unwrap()),
                                ty: &f.ty,
                                index: idx as u32,
                                attrs: &f.attrs,
                            });
                        }
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        for (idx,f) in fields_unnamed.unnamed.iter().enumerate() {
                            field_infos.push(FieldInfo {
                                ident: None,
                                index: idx as u32,
                                ty: &f.ty,
                                attrs: &f.attrs,
                            });
                        }
                    }
                    &syn::Fields::Unit => {
                        //No fields
                    }
                }

                let fields = implement_withschema(field_infos);

                variants.push(quote! {
                (#field_from_version,
                 #field_to_version,
                 #Variant { name: #variant_name_spanned, discriminator: #var_idx, fields:
                    {
                        let mut fields1 = Vec::<#Field>::new();
                        #(#fields;)*
                        fields1
                    }}
                )});
            }
            quote! {
                #[allow(non_upper_case_globals)]
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #withschema for #name #ty_generics #where_clause #extra_where {

                        #[allow(unused_mut)]
                        #[allow(unused_comparisons, unused_variables)]
                        fn schema(version:u32) -> #Schema {
                            let local_version = version;
                            #Schema::Enum (
                                #SchemaEnum {
                                    dbg_name : stringify!(#name).to_string(),
                                    variants : (vec![#(#variants),*]).into_iter().filter_map(|(fromver,tover,x)|{
                                        if local_version >= fromver && local_version <= tover {
                                            Some(x)
                                        } else {
                                            None
                                        }
                                    }).collect()
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
                            ident: Some(field.ident.clone().unwrap()),
                            ty: &field.ty,
                            index: idx as u32,
                            attrs: &field.attrs,
                        })
                        .collect();

                    fields = implement_withschema(field_infos);
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
                    fields = implement_withschema(field_infos);
                }
                &syn::Fields::Unit => {
                    fields = Vec::new();
                }
            }
            quote! {
                #[allow(non_upper_case_globals)]
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

    expanded
}
