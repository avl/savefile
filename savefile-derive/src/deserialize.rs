use common::{check_is_remove, get_extra_where_clauses, parse_attr_tag, FieldInfo, RemovedType};
use get_enum_size;
use proc_macro2::{Literal, TokenStream};
use syn::spanned::Spanned;
use syn::DeriveInput;

fn implement_deserialize(field_infos: Vec<FieldInfo>) -> Vec<TokenStream> {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();
    let removeddef = quote_spanned! { defspan => _savefile::prelude::Removed };
    let abiremoveddef = quote_spanned! { defspan => _savefile::prelude::AbiRemoved };
    let local_deserializer = quote_spanned! { defspan => deserializer};

    let mut output = Vec::new();
    //let mut min_safe_version = 0;
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
                _ => unreachable!(),
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
            abort!(
                field.field_span,
                "Version range is reversed. This is not allowed. Version must be range like 0..2, not like 2..0"
            );
        }

        let src = if field_from_version == 0 && field_to_version == std::u32::MAX && !verinfo.ignore {
            if is_removed.is_removed() {
                abort!(
                    field_type.span(),
                    "The Removed type may only be used for fields which have an old version."
                );
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
            //min_safe_version = min_safe_version.max(verinfo.min_safe_version());
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

                version_mappings.push(quote! {
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

pub fn savefile_derive_crate_deserialize(input: DeriveInput) -> TokenStream {
    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();

    let name = input.ident;

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(
        &generics,
        where_clause,
        quote! {_savefile::prelude::Deserialize + _savefile::prelude::ReprC},
    );

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
            let enum_size = get_enum_size(&input.attrs, enum1.variants.len());

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
                            .map(|(field_index, field)| FieldInfo {
                                ident: Some(field.ident.clone().expect("Expected identifier [6]")),
                                field_span: field.ident.as_ref().unwrap().span(),
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
                            .map(|(field_index, field)| FieldInfo {
                                ident: None,
                                field_span: field.ty.span(),
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
                    #[automatically_derived]
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
                        .map(|(field_index, field)| FieldInfo {
                            ident: Some(field.ident.clone().expect("Expected identifier[7]")),
                            field_span: field.ident.as_ref().unwrap().span(),
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
                        .map(|(field_index, field)| FieldInfo {
                            ident: None,
                            field_span: field.ty.span(),
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
                        #[automatically_derived]
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
            abort_call_site!("Only regular structs are supported");
        }
    };

    expanded
}
