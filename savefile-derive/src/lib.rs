#![recursion_limit="128"]
#![feature(proc_macro)]
extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
extern crate syn;
use proc_macro::TokenStream;
use syn::DeriveInput;
use std::iter::IntoIterator;
struct AttrsResult {
    version_from: u32,
    version_to: u32,
    default_trait: Option<String>,
    default_val: Option<quote::Tokens>,
}

fn check_is_remove(field_type: &syn::Type) -> bool {
    use quote::ToTokens;
    let mut is_remove=false;
    let mut tokens = quote::Tokens::new();
    field_type.to_tokens(&mut tokens);
    for tok in tokens.into_iter() {
        //        if tok.clone().into_tokens()==quote!( savefile::Removed ).clone().into_tokens() {
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
fn parse_attr_tag2(attrs: &Vec<syn::Attribute>, is_string_default_val: bool) -> AttrsResult {
    let mut field_from_version = 0;
    let mut field_to_version = std::u32::MAX;
    let default_trait = None;
    let mut default_val = None;
    for attr in attrs.iter() {
        if let Some(ref meta) = attr.interpret_meta() {
            match meta {
                &syn::Meta::Word(ref _x) => {
                    panic!("Unexpected savefile attribute, word.");
                }
                &syn::Meta::List(ref _x) => {
                    panic!("Unexpected savefile attribute, list.");
                }
                &syn::Meta::NameValue(ref x) => {
                    //println!("Attr name value : {:?}",x.ident.to_string());
                    if x.ident.to_string() == "default_val" {
                        let default_val_str_lit = match &x.lit {
                            &syn::Lit::Str(ref litstr) => litstr,
                            _ => {
                                panic!("Unexpected attribute value, please specify default values within quotes.");
                            }
                        };
                        default_val = if is_string_default_val {
                                    //let litstr=syn::LitStr::new(&default_val_str,span);
                                    Some(quote! { #default_val_str_lit })
                                } else {
                                    let default_evaled = default_val_str_lit.value();
                                    Some(quote!{#default_evaled})
                                };
                    };
                    if x.ident.to_string() == "versions" {
                        match &x.lit {
                            &syn::Lit::Str(ref litstr) => {
                                //println!("Literal value: {:?}",litstr.value());
                                let output: Vec<String> =
                                    litstr.value().split("..").map(|x| x.to_string()).collect();
                                if output.len() != 2 {
                                    panic!("Versions tag must contain a (possibly half-open) range, such as 0..3 or 2.. (fields present in all versions to date should not use the versions-attribute)");
                                }
                                let (a, b) = (output[0].to_string(), output[1].to_string());

                                if a.trim() == "" {
                                    field_from_version = 0;
                                } else if let Ok(a_u32) = a.parse::<u32>() {
                                    field_from_version = a_u32;
                                } else {
                                    panic!("The from version in the version tag must be an integer. Use #[versions=0..3] for example");
                                }

                                if b.trim() == "" {
                                    field_to_version = std::u32::MAX;
                                } else if let Ok(b_u32) = b.parse::<u32>() {
                                    field_to_version = b_u32;
                                } else {
                                    panic!("The to version in the version tag must be an integer. Use #[versions=0..3] for example");
                                }

                                //scan!("{}..{}",)
                            }
                            _ => panic!("Unexpected datatype for value of attribute versions"),
                        }
                    }
                }
            }
        }
    }
    AttrsResult {
        version_from: field_from_version,
        version_to: field_to_version,
        default_trait: default_trait,
        default_val: default_val,
    }
}

struct FieldInfo<'a> {
    ident : Option<syn::Ident>,
    dbg_name : String,
    ty : &'a syn::Type,
    attrs: &'a Vec<syn::Attribute>
}

fn implement_fields_serialize<'a>(field_infos:Vec<FieldInfo<'a>>, implicit_self:bool) -> (quote::Tokens,Vec<quote::Tokens>) {
    let mut min_safe_version=0;
    let mut output = Vec::new();
    let mut optsafe_outputs = Vec::new();
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::def_site();
    let local_serializer = quote_spanned! { defspan => local_serializer};
    let serialize = quote_spanned! {span=>
        Serialize
    };
    let serializer = quote_spanned! {span=>
        Serializer
    };
    let saveerr = quote_spanned! {span=>
        Result<(),SavefileError>
    };

    for ref field in &field_infos {
        {

            let verinfo = parse_attr_tag(&field.attrs, &field.ty);
            let (field_from_version, field_to_version) =
                (verinfo.version_from, verinfo.version_to);

            let id = field.ident.clone().unwrap();
            let removed=check_is_remove(&field.ty);
            let field_type = &field.ty;
            let objid = 
            if implicit_self {
                quote!{ self.#id}
            } else {
                quote!{ #id}
            };

            if field_from_version == 0 && field_to_version == std::u32::MAX {
                if removed {
                    panic!("The Removed type can only be used for removed fields. Use the version attribute.");
                }
                optsafe_outputs.push(quote_spanned!( span => <#field_type as ReprC>::repr_c_optimization_safe(#local_serializer)));
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
                if !removed {
                    optsafe_outputs.push(quote_spanned!( span => <#field_type as ReprC>::repr_c_optimization_safe(#local_serializer)));
                }
                output.push(quote!(
                        if #local_serializer.version >= #field_from_version && #local_serializer.version <= #field_to_version {
                            (#objid).serialize(#local_serializer)?;
                        }));
            }
        }
    }
    let serialize2 = quote! {
        //println!("Serializer running on {}", stringify!(#name));
        let local_serializer = serializer;
        if #min_safe_version > local_serializer.version {
                panic!("Version ranges on fields must not include memory schema version. Field version: {}, memory: {}",
                    #min_safe_version, local_serializer.version);
            }

        #(#output)*           
    };

    let fields_names =
        field_infos.iter().map(|field| {
            let fieldname = field.ident;
            quote! { ref #fieldname }
        }).collect();
    (serialize2,fields_names)

}

#[proc_macro_derive(Serialize, attributes(versions, default_val, default_trait))]
pub fn serialize(input: TokenStream) -> TokenStream {
    // Construct a string representation of the type definition
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;

    let generics = input.generics;


    let gen2=generics.clone();
    let (impl_generics, ty_generics, where_clause) = gen2.split_for_impl();

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::def_site();
    let serialize = quote_spanned! {span=>
        Serialize
    };
    let serializer = quote_spanned! {span=>
        Serializer
    };
    let saveerr = quote_spanned! {span=>
        Result<(),SavefileError>
    };

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut output = Vec::new();
            let variant_count = enum1.variants.len();
            if variant_count > 255 {
                panic!("This library is not capable of serializing enums with more than 255 variants. My deepest apologies!");
            }

            for (var_idx, ref variant) in enum1.variants.iter().enumerate() {
                let var_idx = var_idx as u8;
                let var_ident = variant.ident;
                let variant_name = quote!{ #name::#var_ident };
                let variant_name_spanned = quote_spanned! { span => &#variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {

                        let field_infos : Vec<FieldInfo> = fields_named.named.iter().map(|field|
                            FieldInfo {
                                ident:Some(field.ident.clone().unwrap()),
                                dbg_name:(&field.ident.clone().unwrap()).to_string(),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }).collect();

                        let (fields_serialized,fields_names) = implement_fields_serialize(field_infos, false);
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
                                ident:Some(syn::Ident::from("x".to_string() + &idx.to_string())),
                                dbg_name: "x".to_string() + &idx.to_string(),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }).collect();               

                        let (fields_serialized,fields_names) = implement_fields_serialize(field_infos, false);
                        
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
                impl #impl_generics #serialize for #name #ty_generics #where_clause {

                    #[allow(unused_comparisons)]
                    fn serialize(&self, serializer: &mut #serializer) -> #saveerr {
                        //println!("Serializer running on {} : {:?}", stringify!(#name), self);
                        match self {
                            #(#output,)*
                        }
                        Ok(())
                    }
                }
            }
        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {


                let field_infos : Vec<FieldInfo> = namedfields.named.iter().map(|field|
                    FieldInfo {
                        ident:Some(field.ident.clone().unwrap()),
                        dbg_name:(&field.ident.clone().unwrap()).to_string(),
                        ty:&field.ty,
                        attrs:&field.attrs
                    }).collect();


                //let check:()=fieldInfos;


                let (fields_serialize,_field_names) = implement_fields_serialize(field_infos, true);

                quote! {
                    impl #impl_generics #serialize for #name #ty_generics #where_clause {
                        #[allow(unused_comparisons)]
                        fn serialize(&self, serializer: &mut #serializer)  -> #saveerr {
                            #(#fields_serialize)*
                            Ok(())                    
                        }
                    }
                }
            }
            _ => panic!("Only regular structs supported, not tuple structs."),
        },
        _ => {
            panic!("Only regular structs are supported");
        }
    };

    //println!("Emitting: {:?}",expanded);
    expanded.into()
}

fn implement_deserialize(field_infos:Vec<FieldInfo>) -> Vec<quote::Tokens> {

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::def_site();
    let removeddef = quote_spanned! { span => Removed };
    let local_deserializer = quote_spanned! { defspan => deserializer};

    let mut output=Vec::new();
    let mut min_safe_version=0;
    for ref field in &field_infos {
        let field_type = &field.ty;

        let is_removed=check_is_remove(field_type);
        //let local_deserializer = quote_spanned! { defspan => local_deserializer};

        let verinfo = parse_attr_tag(&field.attrs, &field.ty);
        let (field_from_version, field_to_version, default_trait, default_val) = (
            verinfo.version_from,
            verinfo.version_to,
            verinfo.default_trait,
            verinfo.default_val,
        );
        let fieldname=&field.dbg_name;
        let effective_default_val = if let Some(defval) = default_val {
            quote! { str::parse(#defval).unwrap() }
        } else if let Some(deftrait) = default_trait {
            quote! { #deftrait::default() }
        } else if is_removed {
            quote! { #removeddef::new() }
        } else {
            quote! { panic!("internal error - there was no default value available for field: {}", stringify!(#fieldname) ) }
        };

        if field_from_version > field_to_version {
            panic!("Version range is reversed. This is not allowed. Version must be range like 0..2, not like 2..0");
        }

        let src = if field_from_version == 0 && field_to_version == std::u32::MAX {
            if is_removed {
                panic!("The Removed type may only be used for fields which have an old version."); //TODO: Better message, tell user how to do this annotation
            };
            quote_spanned! { span =>
                <#field_type>::deserialize(#local_deserializer)?
            }
        } else {    
            if field_to_version < std::u32::MAX { // A delete
                min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
            }                    
            if field_from_version < std::u32::MAX { // An addition
                min_safe_version = min_safe_version.max(field_from_version);
            }                    

            quote_spanned! { span =>
                if #local_deserializer.file_version >= #field_from_version && #local_deserializer.file_version <= #field_to_version {
                    <#field_type>::deserialize(#local_deserializer)?
                } else {
                    #effective_default_val
                }
            }
        };

        if let Some(id) = field.ident {
            let id = field.ident.clone();
            let id_spanned = quote_spanned! { span => #id};
            output.push(quote!(#id_spanned : #src ));
        } else {
            output.push(quote!( #src ));
        }
    }
    output
}

#[proc_macro_derive(Deserialize, attributes(versions, default_val, default_trait))]
pub fn deserialize(input: TokenStream) -> TokenStream {
    // Construct a string representation of the type definition
    let input: DeriveInput = syn::parse(input).unwrap();

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::def_site();
    let name = input.ident;


    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let deserialize = quote_spanned! {span=>
        Deserialize
    };
    let deserializer = quote_spanned! {span=>
        Deserializer
    };

    let saveerr = quote_spanned! {span=>
        SavefileError
    };

    let removeddef = quote_spanned! { span => Removed };

    let mut output = Vec::<quote::Tokens>::new();
    
    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut output = Vec::new();
            let variant_count = enum1.variants.len();
            if variant_count > 255 {
                panic!("This library is not capable of deserializing enums with more than 255 variants. My deepest apologies!");
            }

            for (var_idx, ref variant) in enum1.variants.iter().enumerate() {
                let var_idx = var_idx as u8;
                let var_ident = variant.ident;
                let variant_name = quote!{ #name::#var_ident };
                let variant_name_spanned = quote_spanned! { span => #variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        //let fields_names=fields_named.named.iter().map(|x|x.ident.unwrap());
                        
                        let field_infos : Vec<FieldInfo> = fields_named.named.iter().map(|field|
                            FieldInfo {
                                ident:Some(field.ident.clone().unwrap()),
                                dbg_name:(&field.ident.clone().unwrap()).to_string(),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }).collect();

                        let fields_deserialized=implement_deserialize(field_infos);

                        output.push(
                            quote!( #var_idx => #variant_name_spanned{ #(#fields_deserialized,)* } ),
                        );
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        //let fields_names=fields_unnamed.unnamed.iter().enumerate().map(|(idx,x)|"x".to_string()+&idx.to_string());
                         let field_infos : Vec<FieldInfo> = fields_unnamed.unnamed.iter().enumerate().map(|(idx,field)|
                            FieldInfo {
                                ident:None,
                                dbg_name:idx.to_string(),
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
                impl #impl_generics #deserialize for #name #ty_generics #where_clause {
                    #[allow(unused_comparisons)]
                    fn deserialize(deserializer: &mut #deserializer) -> Result<Self,#saveerr> {
                        //println!("Deserializer running on {}", stringify!(#name));
                        
                        Ok(match deserializer.read_u8()? {
                            #(#output,)*
                            _ => panic!("Corrupt file - unknown enum variant detected.")
                        })
                    }
                }
            }
        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {
                //let mut min_safe_version=0;
                let field_infos:Vec<FieldInfo> = namedfields.named.iter().map(
                    |field| FieldInfo {
                        ident : Some(field.ident.unwrap().clone()),
                        dbg_name:(&field.ident.clone().unwrap()).to_string(),
                        ty : &field.ty,
                        attrs: &field.attrs
                    }).collect();

                let output = implement_deserialize(field_infos);

                quote! {

                        impl #impl_generics #deserialize for #name #ty_generics #where_clause {
                        #[allow(unused_comparisons)]
                        fn deserialize(deserializer: &mut #deserializer) -> Result<Self,#saveerr> {
                            
                            //println!("Deserializer running on {}", stringify!(#name));
                            Ok(#name {
                                #(#output,)*
                            })
                        }                        
                    }
                }
            }
            _ => panic!("Only regular structs supported, not tuple structs."),
        },
        _ => {
            panic!("Only regular structs are supported");
        }
    };

    expanded.into()
}

fn implement_reprc(field_infos:Vec<FieldInfo>, generics : syn::Generics, name:syn::Ident) -> quote::Tokens {
    let generics = generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::def_site();
    let reprc = quote_spanned! {span=>
        ReprC
    };    
    let local_file_version = quote_spanned! { defspan => local_file_version};
    let mut min_safe_version=0;
    let mut optsafe_outputs = Vec::new();
    for ref field in &field_infos {

        let verinfo = parse_attr_tag(&field.attrs, &field.ty);
        let (field_from_version, field_to_version) =
            (verinfo.version_from, verinfo.version_to);

        let removed=check_is_remove(&field.ty);
        let field_type = &field.ty;
        if field_from_version == 0 && field_to_version == std::u32::MAX {
            if removed {
                panic!("The Removed type can only be used for removed fields. Use the version attribute.");
            }
            optsafe_outputs.push(quote_spanned!( span => <#field_type as ReprC>::repr_c_optimization_safe(#local_file_version)));
            
        } else {
            if field_to_version < std::u32::MAX {
                min_safe_version = min_safe_version.max(field_to_version.saturating_add(1));
            }
            if field_from_version < std::u32::MAX { // An addition
                min_safe_version = min_safe_version.max(field_from_version);
            }                    
            if !removed {
                optsafe_outputs.push(quote_spanned!( span => <#field_type as ReprC>::repr_c_optimization_safe(#local_file_version)));
            }                            
        }

    }
    quote! {
        unsafe impl #impl_generics #reprc for #name #ty_generics #where_clause {
            #[allow(unused_comparisons)]
            fn repr_c_optimization_safe(file_version:u32) -> bool {
                let local_file_version = file_version;
                file_version >= #min_safe_version
                #( && #optsafe_outputs)*
            }
        }
    }    
}

#[proc_macro_derive(ReprC, attributes(versions, default_val, default_trait))]
pub fn reprc(input: TokenStream) -> TokenStream {
    // Construct a string representation of the type definition
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;

    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::def_site();
    let reprc = quote_spanned! {span=>
        ReprC
    };

    let expanded = match &input.data {



        &syn::Data::Enum(ref enum1) => {

            let mut field_infos = Vec::<FieldInfo>::new();
            for ref variant in enum1.variants.iter() {
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {                        
                        field_infos.extend(fields_named.named.iter().map(|field|
                            FieldInfo {
                                ident:Some(field.ident.clone().unwrap()),
                                dbg_name:(&field.ident.clone().unwrap()).to_string(),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }));

                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        //let fields_names=fields_unnamed.unnamed.iter().enumerate().map(|(idx,x)|"x".to_string()+&idx.to_string());
                        field_infos.extend(fields_unnamed.unnamed.iter().enumerate().map(|(idx,field)|
                            FieldInfo {
                                ident:None,
                                dbg_name:idx.to_string(),
                                ty:&field.ty,
                                attrs:&field.attrs
                            }));
                    }
                    &syn::Fields::Unit => {
                    }
                }
            }
            implement_reprc(field_infos, input.generics, name)            

        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {

                let field_infos:Vec<FieldInfo> = namedfields.named.iter().map(
                    |field| FieldInfo {
                        ident : Some(field.ident.unwrap().clone()),
                        dbg_name:(&field.ident.clone().unwrap()).to_string(),
                        ty : &field.ty,
                        attrs: &field.attrs
                    }).collect();

                implement_reprc(field_infos, input.generics, name)
            }
            _ => panic!("Only regular structs supported, not tuple structs."),
        },
        _ => {
            panic!("Only regular structs are supported");
        }
    };

    //println!("Emitting: {:?}",expanded);
    expanded.into()
}




#[allow(non_snake_case)]
#[proc_macro_derive(WithSchema, attributes(versions, default_val, default_trait))]
pub fn withschema(input: TokenStream) -> TokenStream {
    // Construct a string representation of the type definition
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::def_site();
    let withschema = quote_spanned! {span=>
        WithSchema
    };

    let local_version = quote_spanned! { defspan => local_version};
    let SchemaStruct = quote_spanned! { span => SchemaStruct };
    let SchemaEnum = quote_spanned! { span => SchemaEnum };
    let Schema = quote_spanned! { span => Schema };
    let Field = quote_spanned! { span => Field };
    let Variant = quote_spanned! { span => Variant };
    let WithSchema = quote_spanned! { span => WithSchema };

    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => {
            let mut variants = Vec::new();
            for (var_idx, ref variant) in enum1.variants.iter().enumerate() {
                let var_idx = var_idx as u16;
                let var_ident = variant.ident;
                let variant_name = quote!{ #var_ident };
                let variant_name_spanned = quote_spanned! { span => stringify!(#variant_name).to_string()};

                let verinfo = parse_attr_tag2(&variant.attrs, false);
                let (field_from_version, field_to_version) =
                    (verinfo.version_from, verinfo.version_to);

                if field_to_version != std::u32::MAX {
                    panic!("Savefile automatic derive does not support removal of enum values.");
                }

                let mut fields=Vec::new();

                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        //let fields_names=fields_named.named.iter().map(|x|x.ident.unwrap());
                        for f in fields_named.named.iter() {
                            let ty = &f.ty;
                            if check_is_remove(ty) {
                                panic!("The Removed type is not supported for enum types");
                            };
                            let ty = quote_spanned! { span => #ty };
                            fields.push( quote!{ Box::new(<#ty>::schema(#local_version)) } );
                        };
                    }
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        //let fields_names=fields_unnamed.unnamed.iter().enumerate().map(|(idx,x)|"x".to_string()+&idx.to_string());
                        for f in fields_unnamed.unnamed.iter() {
                            let ty = &f.ty;
                            if check_is_remove(ty) {
                                panic!("The Removed type is not supported for enum types");
                            };

                            fields.push( quote!{ Box::new(<#ty>::schema(#local_version)) } );
                        };
                    }
                    &syn::Fields::Unit => {
                        //No fields
                    }
                }
                variants.push(quote!{
                    (#field_from_version,
                     #field_to_version,
                     #Variant { name: #variant_name_spanned, discriminator: #var_idx, fields: vec![#(#fields),*]}
                    )});
                //variants.push(quote!{Variant { name: #variant_name_spanned, discriminator: #var_idx, fields: vec![]}})
            }
            quote! {
                impl #impl_generics #withschema for #name #ty_generics #where_clause {

                    #[allow(unused_comparisons)]
                    fn schema(version:u32) -> #Schema {
                        let local_version = version;
                        #Schema::Enum (
                            #SchemaEnum {
                                variants : (vec![#(#variants),*]).into_iter().filter_map(|(fromver,tover,x)|{
                                    if local_version >= fromver && local_version <= tover {
                                        Some(x)
                                    } else {
                                        None
                                    }
                                }).collect()
                                //variants : vec![]
                            }
                        )
                    }
                }
            }
        }
        &syn::Data::Struct(ref struc) => match &struc.fields {
            &syn::Fields::Named(ref namedfields) => {
                let local_version = quote_spanned! { defspan => local_version};
                let mut fields=Vec::new();
                let fields1=quote_spanned! { defspan => fields1 };
                for ref field in &namedfields.named {
                    {
                        let verinfo = parse_attr_tag(&field.attrs, &field.ty);
                        let (field_from_version, field_to_version) =
                            (verinfo.version_from, verinfo.version_to);

                        let name=field.ident.unwrap();

                        let removed=check_is_remove(&field.ty);
                        let field_type = &field.ty;
                        if field_from_version == 0 && field_to_version == std::u32::MAX {
                            if removed {
                                panic!("The Removed type can only be used for removed fields. Use the version attribute.");
                            }                            
                            fields.push(quote_spanned!( span => #fields1.push(#Field { name:stringify!(#name).to_string(), value:Box::new(<#field_type as #WithSchema>::schema(#local_version))})));
                            
                        } else {
                            
                            fields.push(quote_spanned!( span => 
                                
                                if #local_version >= #field_from_version && #local_version <= #field_to_version {
                                    #fields1.push(#Field { name:stringify!(#name).to_string(), value:Box::new(<#field_type as #WithSchema>::schema(#local_version))});
                                }
                                ));
                                                        
                        }
                    }
                }
                quote! {
                    impl #impl_generics #withschema for #name #ty_generics #where_clause {
                        #[allow(unused_comparisons)]
                        fn schema(version:u32) -> #Schema {
                            let local_version = version;
                            let mut fields1 = Vec::new();
                            #(#fields;)* ;
                            #Schema::Struct(#SchemaStruct{
                                fields: fields1
                            })
                            
                        }
                    }
                }
            }
            _ => panic!("Only regular structs supported, not tuple structs."),
        },
        _ => {
            panic!("Only regular structs are supported");
        }
    };

    //println!("Emitting: {:?}",expanded);
    expanded.into()
}



