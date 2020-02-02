#![recursion_limit="128"]

//#![feature(proc_macro)]

extern crate proc_macro;

extern crate proc_macro2;
#[macro_use]
extern crate quote;
extern crate syn;
use proc_macro2::TokenStream;
use proc_macro2::Span;
use syn::{DeriveInput, Ident};
use std::iter::IntoIterator;

#[derive(Debug)]
struct VersionRange {
    from:u32,
    to:u32,
    convert_fun : String,
    serialized_type : String,
}

#[derive(Debug)]
struct AttrsResult {
    version_from: u32,
    version_to: u32,
    ignore:bool,
    default_fn: Option<syn::Ident>,
    default_val: Option<TokenStream>,
    deserialize_types : Vec<VersionRange>,
}

fn check_is_remove(field_type: &syn::Type) -> bool {
    use quote::ToTokens;
    let mut is_remove=false;
    let mut tokens = TokenStream::new();
    field_type.to_tokens(&mut tokens);
    for tok in tokens.into_iter() {
        if tok.to_string()=="Removed" { //TODO: This is not robust, since it's based on text matching
            is_remove=true;
        }
    }
    is_remove
}


fn parse_attr_tag(attrs: &Vec<syn::Attribute>, field_type: &syn::Type) -> AttrsResult {
    let is_string=match field_type {
        &syn::Type::Path(ref typepath) => {
            if &typepath.path.segments[0].ident == "String" { true } else {false}
        },
        _ => false
    };
    parse_attr_tag2(attrs, is_string)

}

fn overlap<'a>(b:&'a VersionRange) -> impl Fn(&'a VersionRange) -> bool {
    assert!(b.to >= b.from);
    move |a:&'a VersionRange| {
        assert!(a.to >= a.from);
        let no_overlap = a.to < b.from || a.from > b.to;
        !no_overlap        
    }
}


fn parse_attr_tag2(attrs: &Vec<syn::Attribute>, is_string_default_val: bool) -> AttrsResult {
    let mut field_from_version = None;
    let mut field_to_version = None;
    let mut default_fn = None;
    let mut default_val = None;
    let mut ignore = false;
    let mut deser_types = Vec::new();
    for attr in attrs.iter() {
        if let Some(ref meta) = attr.interpret_meta() {
            match meta {
                &syn::Meta::Word(ref x) => {
                    if x.to_string() == "savefile_ignore" {
                        ignore=true;
                    }

                }
                &syn::Meta::List(ref _x) => {

                }
                &syn::Meta::NameValue(ref x) => {
                    if x.ident.to_string() == "savefile_default_val" {
                        let default_val_str_lit = match &x.lit {
                            &syn::Lit::Str(ref litstr) => litstr,
                            _ => {
                                panic!("Unexpected attribute value, please specify default values within quotes.");
                            }
                        };
                        default_val = if is_string_default_val {
                                    Some(quote! { #default_val_str_lit })
                                } else {
                                    let default_evaled = default_val_str_lit.value();
                                    Some(quote!{#default_evaled})
                                };
                    };
                    if x.ident.to_string() == "savefile_default_fn" {
                        let default_fn_str_lit = match &x.lit {
                            &syn::Lit::Str(ref litstr) => litstr,
                            _ => {
                                panic!("Unexpected attribute value, please specify savefile_default_fn method names within quotes.");
                            }
                        };
                        default_fn = Some(syn::Ident::new(&default_fn_str_lit.value(),proc_macro2::Span::call_site()));
                                
                    };

                    if x.ident.to_string() == "savefile_ignore" {
                        ignore=true;                            

                    };
                    if x.ident.to_string() == "savefile_versions_as" {
                        match &x.lit {
                            &syn::Lit::Str(ref litstr2) => {
                                let output2 : Vec<String> = litstr2.value().splitn(3,":").map(|x| x.to_string()).collect();
                                if output2.len() != 3 && output2.len()!=2 {
                                    panic!("The #savefile_versions_as tag must contain a version range and a deserialization type, such as : #[savefile_versions_as=0..3:MyStructType]");
                                }
                                let litstr = &output2[0];

                                let convert_fun:String;
                                let version_type:String;

                                if output2.len()==2 {
                                    convert_fun = "".to_string();
                                    version_type = output2[1].to_string();
                                } else {
                                    convert_fun = output2[1].to_string();
                                    version_type = output2[2].to_string();    
                                }

                                
                                

                                let output: Vec<String> =
                                    litstr.split("..").map(|x| x.to_string()).collect();
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
                                    from : from_ver,
                                    to : to_ver,
                                    convert_fun : convert_fun.to_string(),
                                    serialized_type : version_type.to_string()
                                };
                                if deser_types.iter().any(overlap(&item)) {
                                    panic!("#savefile_versions_as attributes may not specify overlapping ranges");
                                }
                                deser_types.push(item);

                            }
                            _ => panic!("Unexpected datatype for value of attribute savefile_versions_as"),
                        }
                    }

                    if x.ident.to_string() == "savefile_versions" {
                        match &x.lit {
                            &syn::Lit::Str(ref litstr) => {
                                let output: Vec<String> =
                                    litstr.value().split("..").map(|x| x.to_string()).collect();
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
    }

    let versions_tag_range = VersionRange {
        from : field_from_version.unwrap_or(0),
        to : field_to_version.unwrap_or(std::u32::MAX),
        convert_fun : "dummy".to_string(),
        serialized_type: "dummy".to_string()};
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
        deserialize_types : deser_types
    }
}

struct FieldInfo<'a> {
    ident : Option<syn::Ident>,
    ty : &'a syn::Type,
    attrs: &'a Vec<syn::Attribute>
}

fn implement_fields_serialize<'a>(field_infos:Vec<FieldInfo<'a>>, implicit_self:bool, index:bool) -> (TokenStream,Vec<TokenStream>) {
    let mut min_safe_version=0;
    let mut output = Vec::new();
    
    let defspan = proc_macro2::Span::call_site();
    let span = proc_macro2::Span::call_site();
    let local_serializer = quote_spanned! { defspan => local_serializer};
    let mut index_number = 0;
    for ref field in &field_infos {
        {

            let verinfo = parse_attr_tag(&field.attrs, &field.ty);

            if verinfo.ignore {
                continue;
            }
            let (field_from_version, field_to_version) =
                (verinfo.version_from, verinfo.version_to);

            let removed=check_is_remove(&field.ty);
                        
            let objid = if index {
                assert!(implicit_self);
                let id = syn::Index { index:index_number, span:span };
                index_number+=1;
                quote!{ self.#id}
            } else {
                let id = field.ident.clone().unwrap();
                if implicit_self {
                    quote!{ self.#id}
                } else {
                    quote!{ #id}
                }
            };

            if field_from_version == 0 && field_to_version == std::u32::MAX {
                if removed {
                    panic!("The Removed type can only be used for removed fields. Use the savefile_versions attribute.");
                }
                output.push(quote!(
                    (#objid).serialize(#local_serializer)?;
                    ));
            } else {
                if field_to_version < std::u32::MAX {
                    min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
                }
                if field_from_version < std::u32::MAX { // An addition
                    min_safe_version = min_safe_version.max(field_from_version);
                }                    
                output.push(quote!(
                        if #local_serializer.version >= #field_from_version && #local_serializer.version <= #field_to_version {
                            (#objid).serialize(#local_serializer)?;
                        }));
            }
        }
    }
    let serialize2 = quote! {
        let local_serializer = serializer;
        if #min_safe_version > local_serializer.version {
                panic!("Version ranges on fields must not include memory schema version. Field version: {}, memory version: {}",
                    #min_safe_version.saturating_sub(1), local_serializer.version);
            }

        #(#output)*           
    };

    let fields_names =
        field_infos.iter().map(|field| {
            let fieldname = field.ident.clone();
            quote! { ref #fieldname }
        }).collect();
    (serialize2,fields_names)

}

fn serialize(input: DeriveInput) -> TokenStream {

    let name = input.ident;

    let generics = input.generics;

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();

    let gen2=generics.clone();
    let (impl_generics, ty_generics, where_clause) = gen2.split_for_impl();

    let uses = quote_spanned! { defspan =>
            extern crate savefile as _savefile;            
        };


    let serialize = quote_spanned! {defspan=>
        _savefile::prelude::Serialize
    };
    let serializer = quote_spanned! {defspan=>
        _savefile::prelude::Serializer
    };
    let saveerr = quote_spanned! {defspan=>
        Result<(),_savefile::prelude::SavefileError>
    };
    let magic=format!("_IMPL_SAVEFILE_SERIALIZE_FOR_{}", &name).to_string();    
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
                let variant_name = quote!{ #name::#var_ident };
                let variant_name_spanned = quote_spanned! { span => &#variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {

                        let field_infos : Vec<FieldInfo> = fields_named.named.iter().map(|field|
                            FieldInfo {
                                ident:Some(field.ident.clone().unwrap()),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }).collect();

                        let (fields_serialized,fields_names) = implement_fields_serialize(field_infos, false, false);
                        output.push(
                            quote!( #variant_name_spanned{#(#fields_names,)*} => { 
                                serializer.write_u8(#var_idx)?; 
                                #fields_serialized 
                            } ),
                        );
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {

                        let field_infos : Vec<FieldInfo> = fields_unnamed.unnamed.iter().enumerate().map(|(idx,field)|
                            FieldInfo {
                                ident:Some(syn::Ident::new(&("x".to_string() + &idx.to_string()),Span::call_site())),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }).collect();               

                        let (fields_serialized,fields_names) = implement_fields_serialize(field_infos, false, false);
                        
                        output.push(
                            quote!( #variant_name_spanned(#(#fields_names,)*) => { serializer.write_u8(#var_idx)?; #fields_serialized  } ),
                        );
                    }
                    &syn::Fields::Unit => {
                        output.push(
                            quote!( #variant_name_spanned => { serializer.write_u8(#var_idx)? } ),
                        );
                    }
                }
            }
            quote! {
                #[allow(non_upper_case_globals)] 
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #serialize for #name #ty_generics #where_clause {

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
            let fields_serialize:TokenStream;
            let _field_names:Vec<TokenStream>;
            match &struc.fields {
                &syn::Fields::Named(ref namedfields) => {
                    let field_infos : Vec<FieldInfo> = namedfields.named.iter().map(|field|
                        FieldInfo {
                            ident:Some(field.ident.clone().unwrap()),
                            ty:&field.ty,
                            attrs:&field.attrs
                        }).collect();


                    let t=implement_fields_serialize(field_infos, true, false);
                    fields_serialize = t.0;
                    _field_names = t.1;
                },
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos : Vec<FieldInfo> = fields_unnamed.unnamed.iter().map(|field|
                        FieldInfo {
                            ident:None,
                            ty:&field.ty,
                            attrs:&field.attrs
                        }).collect();               

                    let t = implement_fields_serialize(field_infos, true, true);
                    fields_serialize = t.0;
                    _field_names = t.1;
                },
                &syn::Fields::Unit => {
                    _field_names=Vec::new();
                    fields_serialize = quote!{ {} };
                }
            }
            quote! {
                #[allow(non_upper_case_globals)] 
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #serialize for #name #ty_generics #where_clause {
                        #[allow(unused_comparisons, unused_variables)]
                        fn serialize(&self, serializer: &mut #serializer)  -> #saveerr {
                            #(#fields_serialize)*
                            Ok(())                    
                        }
                    }
                };
            }            
        }
        ,
        _ => {
            panic!("Unsupported data type");
        }
    };

    expanded
}

fn implement_deserialize(field_infos:Vec<FieldInfo>) -> Vec<TokenStream> {

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let removeddef = quote_spanned! { defspan => _savefile::prelude::Removed };
    let local_deserializer = quote_spanned! { defspan => deserializer};

    let mut output=Vec::new();
    let mut min_safe_version=0;
    for ref field in &field_infos {
        let field_type = &field.ty;

        let is_removed=check_is_remove(field_type);

        let verinfo = parse_attr_tag(&field.attrs, &field.ty);
        let (field_from_version, field_to_version, default_fn, default_val) = (
            verinfo.version_from,
            verinfo.version_to,
            verinfo.default_fn,
            verinfo.default_val,
        );
        let mut exists_version_which_needs_default_value=false;
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
            quote! { str::parse(#defval).unwrap() }
        } else if let Some(default_fn) = default_fn {
            quote_spanned! { span => #default_fn() }
        } else if !exists_version_which_needs_default_value {
            quote! { panic!("Unexpected unsupported file version: {}",#local_deserializer.file_version) } //Should be impossible    
        } else {
            quote_spanned! { span => Default::default() }        
        };
        if field_from_version > field_to_version {
            panic!("Version range is reversed. This is not allowed. Version must be range like 0..2, not like 2..0");
        }

        let src = if field_from_version == 0 && field_to_version == std::u32::MAX  && !verinfo.ignore {
            if is_removed {
                panic!("The Removed type may only be used for fields which have an old version."); //TODO: Better message, tell user how to do this annotation
            };
            quote_spanned! { span =>
                <#field_type>::deserialize(#local_deserializer)?
            }
        } else if verinfo.ignore {            
            quote_spanned! { span => 
                #effective_default_val
            }
        } else {    
            if field_to_version < std::u32::MAX { // A delete
                min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
            }                    
            if field_from_version < std::u32::MAX { // An addition
                min_safe_version = min_safe_version.max(field_from_version);
            }        
            let mut version_mappings=Vec::new();            
            for dt in verinfo.deserialize_types.iter() {
                let dt_from = dt.from;
                let dt_to = dt.to;
                let dt_field_type = syn::Ident::new(&dt.serialized_type, span);
                let dt_convert_fun =
                    if dt.convert_fun.len() > 0 {
                        let dt_conv_fun = syn::Ident::new(&dt.convert_fun, span);
                        quote! { #dt_conv_fun }
                    }
                    else {
                        quote! { <#field_type>::from }
                    };
                    
                 

                version_mappings.push(quote!{
                    if #local_deserializer.file_version >= #dt_from && #local_deserializer.file_version <= #dt_to {
                        let temp : #dt_field_type = <#dt_field_type>::deserialize(#local_deserializer)?;
                        #dt_convert_fun(temp)
                    } else 
                });
            }

            quote_spanned! { span =>
                #(#version_mappings)*
                if #local_deserializer.file_version >= #field_from_version && #local_deserializer.file_version <= #field_to_version {
                    <#field_type>::deserialize(#local_deserializer)?
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

#[proc_macro_derive(Savefile, attributes(savefile_versions, savefile_versions_as, savefile_ignore, savefile_default_val, savefile_default_fn))]
pub fn savefile(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let s=serialize(input.clone());

    let d=deserialize(input.clone());

    let w=withschema(input.clone());

    let i=introspect(input);

    let expanded=quote! {
        #s

        #d

        #w

        #i
    };

    expanded.into()
}
#[proc_macro_derive(SavefileIntrospectOnly, attributes(savefile_versions, savefile_versions_as, savefile_ignore, savefile_default_val, savefile_default_fn))]
pub fn savefile_introspect_only(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let i=introspect(input);

    let expanded=quote! {
        #i
    };

    expanded.into()
}

fn deserialize(input: DeriveInput) -> TokenStream {



    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    
    let name = input.ident;


    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let deserialize = quote_spanned! {defspan=>
        _savefile::prelude::Deserialize
    };

    let uses = quote_spanned! { defspan =>
            extern crate savefile as _savefile;            
        };

    let deserializer = quote_spanned! {defspan=>
        _savefile::prelude::Deserializer
    };

    let saveerr = quote_spanned! {defspan=>
        _savefile::prelude::SavefileError
    };

    
    let magic=format!("_IMPL_SAVEFILE_DESERIALIZE_FOR_{}", &name).to_string();    
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
                let variant_name = quote!{ #name::#var_ident };
                let variant_name_spanned = quote_spanned! { span => #variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        
                        let field_infos : Vec<FieldInfo> = fields_named.named.iter().map(|field|
                            FieldInfo {
                                ident:Some(field.ident.clone().unwrap()),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }).collect();

                        let fields_deserialized=implement_deserialize(field_infos);

                        output.push(
                            quote!( #var_idx => #variant_name_spanned{ #(#fields_deserialized,)* } ),
                        );
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                         let field_infos : Vec<FieldInfo> = fields_unnamed.unnamed.iter().map(|field|
                            FieldInfo {
                                ident:None,
                                ty:&field.ty,
                                attrs:&field.attrs
                            }).collect();                                     
                        let fields_deserialized=implement_deserialize(field_infos);
                            
                        output.push(
                            quote!( #var_idx => #variant_name_spanned( #(#fields_deserialized,)*) ),
                        );
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
                    impl #impl_generics #deserialize for #name #ty_generics #where_clause {
                        #[allow(unused_comparisons, unused_variables)]
                        fn deserialize(deserializer: &mut #deserializer) -> Result<Self,#saveerr> {
                            
                            Ok(match deserializer.read_u8()? {
                                #(#output,)*
                                _ => panic!("Corrupt file - unknown enum variant detected.")
                            })
                        }
                    }
                };
            }
        }
        &syn::Data::Struct(ref struc) => {
            let output = match &struc.fields {
                &syn::Fields::Named(ref namedfields) => {
                    let field_infos:Vec<FieldInfo> = namedfields.named.iter().map(
                        |field| FieldInfo {
                            ident : Some(field.ident.clone().unwrap()),
                            ty : &field.ty,
                            attrs: &field.attrs
                        }).collect();

                    let output1=implement_deserialize(field_infos);
                    quote!{Ok(#name {
                                #(#output1,)*
                            })}

                },
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                     let field_infos : Vec<FieldInfo> = fields_unnamed.unnamed.iter().map(|field|
                        FieldInfo {
                            ident:None,
                            ty:&field.ty,
                            attrs:&field.attrs
                        }).collect();                                     
                    let output1=implement_deserialize(field_infos);

                    quote!{Ok(#name (
                                #(#output1,)*
                            ))}
                },
                &syn::Fields::Unit => {
                    let output1=implement_deserialize(Vec::new());

                    quote!{Ok(#name (
                                #(#output1,)*
                            ))}

                }
                //_ => panic!("Only regular structs supported, not tuple structs."),
            };
            quote! {
                #[allow(non_upper_case_globals)] 
                const #dummy_const: () = {
                        #uses
                        impl #impl_generics #deserialize for #name #ty_generics #where_clause {
                        #[allow(unused_comparisons, unused_variables)]
                        fn deserialize(deserializer: &mut #deserializer) -> Result<Self,#saveerr> {                            
                            #output
                        }                        
                    }
                };
            }
        },
        _ => {
            panic!("Only regular structs are supported");
        }
    };

    expanded
}

#[allow(non_snake_case)]
fn implement_reprc(field_infos:Vec<FieldInfo>, generics : syn::Generics, name:syn::Ident) -> TokenStream {
    let generics = generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let reprc = quote_spanned! {defspan=>
        _savefile::prelude::ReprC
    };    
    let local_file_version = quote_spanned! { defspan => local_file_version};
    let WithSchema =  quote_spanned! { defspan => _savefile::prelude::WithSchema};
    let mut min_safe_version=0;
    let mut optsafe_outputs = Vec::new();
    let uses = quote_spanned! { defspan =>
            extern crate savefile as _savefile;            
        };
    let magic=format!("_IMPL_SAVEFILE_REPRC_FOR_{}", &name).to_string();    
    let dummy_const = syn::Ident::new(&magic, proc_macro2::Span::call_site());

    for ref field in &field_infos {

        let verinfo = parse_attr_tag(&field.attrs, &field.ty);
        if verinfo.ignore {
            panic!("The ReprC attribute cannot be derived for structures containing ignored fields");
        }
        let (field_from_version, field_to_version) =
            (verinfo.version_from, verinfo.version_to);

        let removed=check_is_remove(&field.ty);
        let field_type = &field.ty;
        if field_from_version == 0 && field_to_version == std::u32::MAX {
            if removed {
                panic!("The Removed type can only be used for removed fields. Use the savefile_version attribute to mark a field as only existing in previous versions.");
            }
            optsafe_outputs.push(quote_spanned!( span => <#field_type as ReprC>::repr_c_optimization_safe(#local_file_version)));
            
        } else {
            if field_to_version < std::u32::MAX {
                min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
            }
            
            min_safe_version = min_safe_version.max(field_from_version);
                                
            if !removed {
                optsafe_outputs.push(quote_spanned!( span => <#field_type as ReprC>::repr_c_optimization_safe(#local_file_version)));
            }                            
        }

    }
    quote! {
        
        #[allow(non_upper_case_globals)] 
        const #dummy_const: () = {
            extern crate std;
            #uses
            unsafe impl #impl_generics #reprc for #name #ty_generics #where_clause {
                #[allow(unused_comparisons,unused_variables, unused_variables)]
                fn repr_c_optimization_safe(file_version:u32) -> bool {
                    // The following is a debug_assert because it is slightly expensive, and the entire
                    // point of the ReprC trait is to speed things up.
                    if cfg!(debug_assertions) {
                        if Some(std::mem::size_of::<#name>()) != <#name as #WithSchema>::schema(file_version).serialized_size() {
                            panic!("Size mismatch for struct {}. In memory size: {}, schema size: {:?}. Maybe use repr(C)?",
                                stringify!(#name),
                                std::mem::size_of::<#name>(),
                                <#name as #WithSchema>::schema(file_version).serialized_size());
                        }
                    }
                    let local_file_version = file_version;
                    file_version >= #min_safe_version
                    #( && #optsafe_outputs)*
                }
            }
        };
    }    
}


fn get_enum_size(attrs:&Vec<syn::Attribute>) -> Option<u32> {
    use quote::ToTokens;
    let mut size_u32:Option<u32> = None;
    for attr in attrs.iter() {
        if let Some(ref meta) = attr.interpret_meta() {
            match meta {
                &syn::Meta::NameValue(ref _x) => {
                }
                &syn::Meta::Word(ref _x) => {                    
                }
                &syn::Meta::List(ref metalist) => {

                    if &metalist.ident.to_string() == &"repr" {
                        for x in &metalist.nested {
                            let size_str : String = match *x {
                                syn::NestedMeta::Meta(ref inner_x) => {
                                    match inner_x {
                                        &syn::Meta::NameValue(ref _x) => {
                                            panic!("Unsupported repr-attribute: repr({:?})",x.clone().into_token_stream());
                                        }
                                        &syn::Meta::Word(ref lit_word) => {
                                            lit_word.to_string()
                                        }
                                        &syn::Meta::List(ref _metalist) => {
                                            panic!("Unsupported repr-attribute: repr({:?})",x.clone().into_token_stream());
                                        }
                                    }                                    
                                },
                                syn::NestedMeta::Literal(ref lit) => {
                                    match lit {
                                        &syn::Lit::Str(ref litstr) => litstr.value(),
                                        _ => {
                                            panic!("Unsupported repr-attribute: repr({:?})",x.clone().into_token_stream());
                                        }
                                    }
                                },
                            };
                            size_u32=match size_str.as_ref() {
                                "u8" => Some(1),
                                "i8" => Some(1),
                                "u16" => Some(2),
                                "i16" => Some(2),
                                "u32" => Some(4),
                                "i32" => Some(4),
                                "u64" => Some(8),
                                "i64" => Some(8),
                                _ => panic!("Unsupported repr(X) attribute on enum: {}",size_str)
                            }
                        }

                    }
                    
                }
            }
        }
    }
    size_u32
}
#[proc_macro_derive(ReprC, attributes(savefile_versions, savefile_versions_as, savefile_ignore, savefile_default_val, savefile_default_fn))]
pub fn reprc(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();
    
    let name = input.ident;

    let expanded = match &input.data {


        &syn::Data::Enum(ref enum1) => {
            let enum_size = get_enum_size(&input.attrs);
            if let Some(enum_size) = enum_size {
                if enum_size != 1 {
                    panic!("The ReprC trait assumes that the enum representation is u8 or i8. Savefile does not support enums with more than 255 variants. Sorry.");
                }
            }  
            
            let field_infos = Vec::<FieldInfo>::new();
            for ref variant in enum1.variants.iter() {
                match &variant.fields {
                    &syn::Fields::Named(ref _fields_named) => {                        
                        panic!("The ReprC trait cannot be derived for enums with fields.");
                    }
                    &syn::Fields::Unnamed(ref _fields_unnamed) => {
                        panic!("The ReprC trait cannot be derived for enums with fields.");
                    }
                    &syn::Fields::Unit => {
                        if enum_size.is_none() {
                            panic!("Enums which derive the ReprC trait must specify the enum size using the repr-attribute, like #[repr(u8)].");
                        }
                    }
                }
            }
            implement_reprc(field_infos, input.generics, name)            

        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {

                let field_infos:Vec<FieldInfo> = namedfields.named.iter().map(
                    |field| FieldInfo {
                        ident : Some(field.ident.clone().unwrap()),
                        ty : &field.ty,
                        attrs: &field.attrs
                    }).collect();

                implement_reprc(field_infos, input.generics, name)
            }
            &syn::Fields::Unnamed(ref fields_unnamed) => {
                let field_infos : Vec<FieldInfo> = fields_unnamed.unnamed.iter().enumerate().map(|(idx,field)|
                    FieldInfo {
                        ident:Some(syn::Ident::new(&("x".to_string() + &idx.to_string()),Span::call_site())),
                        ty:&field.ty,
                        attrs:&field.attrs
                    }).collect();

                implement_reprc(field_infos, input.generics, name)
            }
            &syn::Fields::Unit => {
                implement_reprc(Vec::new(), input.generics, name)
            }

        },
        _ => {
            panic!("Unsupported data type");
        }
    };

    expanded.into()
}

#[allow(non_snake_case)]
fn implement_introspect(field_infos:Vec<FieldInfo>, need_self:bool) -> (Vec<TokenStream>,Vec<TokenStream>) {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();

    //let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    //let Introspect = quote_spanned! { defspan => _savefile::prelude::Introspect };
    //let fields1=quote_spanned! { defspan => fields1 };
    let index1=quote_spanned! { defspan => index };
    let introspect_item = quote_spanned! {defspan=>
        _savefile::prelude::introspect_item
    };

    let mut fields = Vec::new();
    let mut fields_names = Vec::new();

    for (idx,ref field) in field_infos.iter().enumerate() {
        if need_self {

            let fieldname;
            let fieldname_raw;

            if let Some(id) = field.ident.clone() {
                fieldname = quote!{&self.#id};
                fieldname_raw = quote!{#id};
            } else {
                let idd = syn::Index::from(idx);
                fieldname = quote! {&self.#idd};
                fieldname_raw = quote!{#idd};
            }
            fields.push(quote_spanned!( span => if #index1 == #idx { return Some(#introspect_item(stringify!(#fieldname_raw).to_string(), #fieldname))}));
            fields_names.push(fieldname_raw);

        } else {

            if let Some(id) = field.ident.clone() {
                let fieldname ;
                let quoted_fieldname;
                let raw_fieldname = id.to_string();
                fieldname = id; //field.ident.clone().map(|x|x).unwrap_or(Ident::new(&format!("v{}",idx),span));
                quoted_fieldname = quote! { #fieldname };
                fields.push(quote_spanned!( span => if #index1 == #idx { return Some(#introspect_item(#raw_fieldname.to_string(), #quoted_fieldname))}));
                fields_names.push(quoted_fieldname);

            } else {
                let fieldname ;
                let quoted_fieldname;
                let raw_fieldname = idx.to_string();
                fieldname = Ident::new(&format!("v{}",idx),span);
                quoted_fieldname = quote! { #fieldname };
                fields.push(quote_spanned!( span => if #index1 == #idx { return Some(#introspect_item(#raw_fieldname.to_string(), #quoted_fieldname))}));
                fields_names.push(quoted_fieldname);

            }
        }


    }

    (fields_names,fields)
}


#[allow(non_snake_case)]
fn introspect(input: DeriveInput) -> TokenStream {

    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

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

    let magic=format!("_IMPL_SAVEFILE_INTROSPECT_FOR_{}", &name).to_string();
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
                let variant_name = quote!{ #var_ident };
                let variant_name_spanned = quote_spanned! { span => #variant_name};

                let mut field_infos=Vec::new();

                let return_value_name = format!("{}::{}",name,var_ident.to_string());
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        for f in fields_named.named.iter() {
                            field_infos.push(FieldInfo {
                                ident:Some(f.ident.clone().unwrap()),
                                ty:&f.ty,
                                attrs:&f.attrs
                            });
                        };
                        let (fields_names,fields) = implement_introspect(field_infos, false);
                        let fields_names2 = fields_names.clone();
                        let fields_names3 = fields_names.clone();
                        let num_fields = fields_names3.len();
                        variants.push(
                            quote!( #name::#variant_name_spanned{#(#fields_names,)*} => {
                                #(#fields;)*
                            } )
                        );
                        value_variants.push(
                            quote!( #name::#variant_name_spanned{#(#fields_names2,)*} => {
                                #return_value_name.to_string()
                            } )
                        );
                        len_variants.push(
                            quote!( #name::#variant_name_spanned{#(#fields_names3,)*} => {
                                #num_fields
                            } )
                        );

                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        for f in fields_unnamed.unnamed.iter() {
                            field_infos.push(FieldInfo {
                                ident:None,
                                ty:&f.ty,
                                attrs:&f.attrs
                            });
                        };
                        let (fields_names,fields) = implement_introspect(field_infos, false);
                        let fields_names2 = fields_names.clone();
                        let fields_names3 = fields_names.clone();
                        let num_fields = fields_names3.len();

                        variants.push(
                            quote!( #name::#variant_name_spanned(#(#fields_names,)*) => { #(#fields;)* } )
                        );
                        value_variants.push(
                            quote!( #name::#variant_name_spanned(#(#fields_names2,)*) => #return_value_name.to_string() )
                        );
                        len_variants.push(
                            quote!( #name::#variant_name_spanned(#(#fields_names3,)*) => {
                                #num_fields
                            } )
                        );

                    }
                    &syn::Fields::Unit => {
                        //No fields
                        variants.push(quote!{
                            #name::#variant_name_spanned => {}
                        });
                        value_variants.push(
                            quote!( #name::#variant_name_spanned => #return_value_name.to_string() )
                        );
                        len_variants.push(
                            quote!( #name::#variant_name_spanned => 0)
                        );

                    }
                }


                //variants.push(quote!{})



            }
            quote! {
                #[allow(non_upper_case_globals)]
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #introspect for #name #ty_generics #where_clause {

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

                    let field_infos:Vec<FieldInfo> = namedfields.named.iter().map(
                        |field| FieldInfo {
                            ident : Some(field.ident.clone().unwrap()),
                            ty : &field.ty,
                            attrs: &field.attrs
                        }).collect();

                    fields = implement_introspect(field_infos, true);
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos:Vec<FieldInfo> = fields_unnamed.unnamed.iter().map(|f| {
                        FieldInfo {
                            ident:None,
                            ty:&f.ty,
                            attrs:&f.attrs
                        }}).collect();

                    fields = implement_introspect(field_infos, true);
                }
                &syn::Fields::Unit => {
                    fields=(Vec::new(),Vec::new());
                }
            }
            let fields1 = fields.1;
            let field_count = fields1.len();
            quote! {
                #[allow(non_upper_case_globals)]
                const #dummy_const: () = {
                    #uses

                    impl #impl_generics #introspect for #name #ty_generics #where_clause {
                        #[allow(unused_comparisons)]
                        #[allow(unused_mut, unused_variables)]
                        fn introspect_value(&self) -> String {
                            stringify!(#name).to_string()
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
        },
        _ => {
            panic!("Unsupported datatype");
        }
    };

    expanded

}


#[allow(non_snake_case)]
fn implement_withschema(field_infos:Vec<FieldInfo>) -> Vec<TokenStream> {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let local_version = quote_spanned! { defspan => local_version};
    let Field = quote_spanned! { defspan => _savefile::prelude::Field };
    let WithSchema = quote_spanned! { defspan => _savefile::prelude::WithSchema };
    let fields1=quote_spanned! { defspan => fields1 };

    let mut fields = Vec::new();
    for (idx,ref field) in field_infos.iter().enumerate() {

        let verinfo = parse_attr_tag(&field.attrs, &field.ty);
        if verinfo.ignore {
            continue;
        }
        let (field_from_version, field_to_version) =
            (verinfo.version_from, verinfo.version_to);

        let name=if let Some(name) = field.ident.clone() {
            (&name).to_string()
        } else {
            idx.to_string()
        };
        let removed=check_is_remove(&field.ty);
        let field_type = &field.ty;
        if field_from_version == 0 && field_to_version == std::u32::MAX {
            if removed {
                panic!("The Removed type can only be used for removed fields. Use the savefile_version attribute.");
            }
            fields.push(quote_spanned!( span => #fields1.push(#Field { name:#name.to_string(), value:Box::new(<#field_type as #WithSchema>::schema(#local_version))})));

        } else {

            let mut version_mappings=Vec::new();
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
fn withschema(input: DeriveInput) -> TokenStream {

    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

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
    
    let magic=format!("_IMPL_SAVEFILE_WITHSCHEMA_FOR_{}", &name).to_string();    
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
                let variant_name = quote!{ #var_ident };
                let variant_name_spanned = quote_spanned! { span => stringify!(#variant_name).to_string()};

                let verinfo = parse_attr_tag2(&variant.attrs, false);
                let (field_from_version, field_to_version) =
                    (verinfo.version_from, verinfo.version_to);

                if field_to_version != std::u32::MAX {
                    panic!("Savefile automatic derive does not support removal of enum values.");
                }

                let mut field_infos=Vec::new();

                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        for f in fields_named.named.iter() {
                            field_infos.push(FieldInfo {
                                ident:Some(f.ident.clone().unwrap()),
                                ty:&f.ty,
                                attrs:&f.attrs
                            });
                        };
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        for f in fields_unnamed.unnamed.iter() {
                            field_infos.push(FieldInfo {
                                ident:None,
                                ty:&f.ty,
                                attrs:&f.attrs
                            });
                        };
                    }
                    &syn::Fields::Unit => {
                        //No fields
                    }
                }

                let fields=implement_withschema(field_infos);

                variants.push(quote!{
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

                    impl #impl_generics #withschema for #name #ty_generics #where_clause {

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

                    let field_infos:Vec<FieldInfo> = namedfields.named.iter().map(
                        |field| FieldInfo {
                            ident : Some(field.ident.clone().unwrap()),
                            ty : &field.ty,
                            attrs: &field.attrs
                        }).collect();

                    fields = implement_withschema(field_infos);
                }
                &syn::Fields::Unnamed(ref fields_unnamed) => {
                    let field_infos:Vec<FieldInfo> = fields_unnamed.unnamed.iter().map(|f| {
                        FieldInfo {
                            ident:None,
                            ty:&f.ty,
                            attrs:&f.attrs
                        }}).collect();                    
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

                    impl #impl_generics #withschema for #name #ty_generics #where_clause {
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
        },
        _ => {
            panic!("Unsupported datatype");
        }
    };

    expanded
}



