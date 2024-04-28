use proc_macro2::{Span, TokenStream};
use syn::DeriveInput;

use common::{get_extra_where_clauses, parse_attr_tag, FieldInfo};
use get_enum_size;
use implement_fields_serialize;
use syn::spanned::Spanned;

pub(super) fn savefile_derive_crate_serialize(input: DeriveInput) -> TokenStream {
    let name = input.ident;
    let name_str = name.to_string();

    let generics = input.generics;

    let span = proc_macro2::Span::call_site();
    let defspan = proc_macro2::Span::call_site();

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_where = get_extra_where_clauses(
        &generics,
        where_clause,
        quote! {_savefile::prelude::Serialize + _savefile::prelude::ReprC},
    );

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
                let var_idx_u8: u8 = var_idx_usize as u8;
                let var_idx_u16: u16 = var_idx_usize as u16;
                let var_idx_u32: u32 = var_idx_usize as u32;

                let verinfo = parse_attr_tag(&variant.attrs);
                let (field_from_version, field_to_version) = (verinfo.version_from, verinfo.version_to);

                let variant_serializer = match enum_size.discriminant_size {
                    1 => quote! { serializer.write_u8(#var_idx_u8)? ; },
                    2 => quote! { serializer.write_u16(#var_idx_u16)? ; },
                    4 => quote! { serializer.write_u32(#var_idx_u32)? ; },
                    _ => unreachable!(),
                };

                let var_ident = (variant.ident).clone();
                let variant_name = quote! { #name::#var_ident };
                let variant_name_str = var_ident.to_string();
                let variant_name_spanned = quote_spanned! { span => #variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        let field_infos: Vec<FieldInfo> = fields_named
                            .named
                            .iter()
                            .enumerate()
                            .map(|(field_index, field)| FieldInfo {
                                ident: Some(field.ident.clone().expect("Expected identifier[4]")),
                                field_span: field.ident.as_ref().unwrap().span(),
                                index: field_index as u32,
                                ty: &field.ty,
                                attrs: &field.attrs,
                            })
                            .collect();

                        let (fields_serialized, fields_names) =
                            implement_fields_serialize(field_infos, false, false /*we've invented real names*/);
                        output.push(quote!( #variant_name_spanned{#(#fields_names,)*} => {
                            if serializer.file_version < #field_from_version || serializer.file_version > #field_to_version {
                                panic!("Enum {}, variant {} is not present in version {}", #name_str, #variant_name_str, serializer.file_version);
                            }
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
                                field_span: field.ty.span(),
                                ident: Some(syn::Ident::new(
                                    // We bind the tuple field to a real name, like x0, x1 etc.
                                    &("x".to_string() + &idx.to_string()),
                                    Span::call_site(),
                                )),
                                index: idx as u32,
                                ty: &field.ty,
                                attrs: &field.attrs,
                            })
                            .collect();

                        let (fields_serialized, fields_names) =
                            implement_fields_serialize(field_infos, false, false /*we've invented real names*/);

                        output.push(
                            quote!(

                            #variant_name_spanned(#(#fields_names,)*) => {
                                if serializer.file_version < #field_from_version || serializer.file_version > #field_to_version {
                                    panic!("Enum {}, variant {} is not present in version {}", #name_str, #variant_name_str, serializer.file_version);
                                }
                                #variant_serializer ; #fields_serialized
                            }
                        ),
                        );
                    }
                    &syn::Fields::Unit => {
                        output.push(quote!( #variant_name_spanned => {
                        if serializer.file_version < #field_from_version || serializer.file_version > #field_to_version {
                            panic!("Enum {}, variant {} is not present in version {}", #name_str, #variant_name_str, serializer.file_version);
                        }
                        #variant_serializer ; } ));
                    }
                }
            }
            quote! {
                #[allow(non_upper_case_globals)]
                #[allow(clippy::double_comparisons)]
                #[allow(clippy::manual_range_contains)]
                const #dummy_const: () = {
                    #uses

                    #[automatically_derived]
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
                        .map(|(field_index, field)| FieldInfo {
                            ident: Some(field.ident.clone().expect("Identifier[5]")),
                            field_span: field.ident.as_ref().unwrap().span(),
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
                        .map(|(field_index, field)| FieldInfo {
                            field_span: field.ty.span(),
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

                    #[automatically_derived]
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
            abort_call_site!("Unsupported data type");
        }
    };

    expanded
}
