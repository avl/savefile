#![feature(proc_macro)]
extern crate proc_macro;
#[macro_use] 
extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;
use proc_macro::TokenStream;
use syn::{ DeriveInput };

struct AttrsResult {
    version_from:u32,
    version_to:u32,
    default_trait:Option<String>,
    default_val:Option<quote::Tokens>

}


fn parse_attr_tag(attrs:&Vec<syn::Attribute>, field_type: &syn::Type) -> AttrsResult {
    let mut field_from_version=0;
    let mut field_to_version=std::u32::MAX;
    let mut default_trait = None;
    let mut default_val = None;
    let span = proc_macro2::Span::call_site();
    for attr in attrs.iter() {
        if let Some(ref meta) = attr.interpret_meta() {
            match meta {
                &syn::Meta::Word(ref x) => {
                    panic!("Unexpected savegame attribute, word.");
                },
                &syn::Meta::List(ref x) => {
                    panic!("Unexpected savegame attribute, list.");
                },
                &syn::Meta::NameValue(ref x) => {
                    //println!("Attr name value : {:?}",x.ident.to_string());
                    if x.ident.to_string()=="default_val" {                                            
                        let default_val_str_lit=match &x.lit {
                            &syn::Lit::Str(ref litstr) => {
                                litstr
                            }
                            _ => {
                                panic!("Unexpected attribute value, please specify default values within quotes.");
                            }
                        };
                        default_val = match field_type {
                            &syn::Type::Path(ref typepath) => {
                                if &typepath.path.segments[0].ident=="String" {
                                    //let litstr=syn::LitStr::new(&default_val_str,span);                                
                                    Some(quote! { #default_val_str_lit } )
                                } else {
                                    let default_evaled=default_val_str_lit.value();
                                    Some(quote!{#default_evaled})
                                }
                            },
                            _ => panic!("Field type is not compatible with default_val attribute")
                        }
                    };
                    if x.ident.to_string()=="versions" {                                            
                        match &x.lit {
                            &syn::Lit::Str(ref litstr) => {
                                //println!("Literal value: {:?}",litstr.value());
                                let output:Vec<String>=litstr.value().split("..").map(|x|x.to_string()).collect();
                                if output.len()!=2 {
                                    panic!("Versions tag must contain a (possibly half-open) range, such as 0..3 or 2.. (fields present in all versions to date should not use the versions-attribute)");
                                }
                                let (a,b)=(output[0].to_string(),output[1].to_string());

                                if a.trim()=="" {
                                    field_from_version = 0;
                                } else if let Ok(a_u32) = a.parse::<u32>() {
                                    field_from_version=a_u32;
                                }
                                else
                                {
                                    panic!("The from version in the version tag must be an integer. Use #[versions=0..3] for example");
                                }

                                if b.trim()=="" {
                                    field_to_version = std::u32::MAX;
                                } else if let Ok(b_u32) = b.parse::<u32>() {
                                    field_to_version=b_u32;
                                }
                                else
                                {
                                    panic!("The to version in the version tag must be an integer. Use #[versions=0..3] for example");
                                }

                                //scan!("{}..{}",)
                            },
                            _ => panic!("Unexpected datatype for value of attribute versions")

                        }
                    }

                },
            }
        }
    }
    AttrsResult {
        version_from : field_from_version,
        version_to : field_to_version,
        default_trait : default_trait,
        default_val : default_val
    }
}

#[proc_macro_derive(Serialize, attributes(versions,default_val,default_trait))]
pub fn serialize(input: TokenStream) -> TokenStream {
    // Construct a string representation of the type definition
    let input: DeriveInput = syn::parse(input).unwrap();
    

    let name = input.ident;

	let generics = input.generics;
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let span = proc_macro2::Span::call_site();
	let serialize = quote_spanned! {span=>
	    Serialize
	};
	let serializer = quote_spanned! {span=>
	    Serializer
	};

	
	let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) =>  {
            
			let mut output=Vec::new();
            let variant_count = enum1.variants.len();
            if variant_count>255 {
                panic!("This library is not capable of serializing enums with more than 255 variants. My deepest apologies!");                
            }

        	for (var_idx,ref variant) in enum1.variants.iter().enumerate() {
                let var_idx=var_idx as u8;
                let var_ident=variant.ident;
                let variant_name=quote!{ #name::#var_ident };
                let variant_name_spanned=quote_spanned! { span => &#variant_name};
        		match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        let fields_names=fields_named.named.iter().map(|x|{let fieldname=x.ident.unwrap();quote!{ ref #fieldname } } );
                        let fields_serialized = fields_named.named.iter().map(
                            |x| { let field_name=x.ident.unwrap();quote!{ #field_name.serialize(serializer); } } ) ;
                        output.push(quote!(#variant_name_spanned{#(#fields_names,)*} => { serializer.write_u8(#var_idx); #(#fields_serialized)* } ) );
                    },
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        let fields_names=fields_unnamed.unnamed.iter().enumerate().map(|(idx,x)|
                            {
                                let fieldname=syn::Ident::from("x".to_string()+&idx.to_string());
                                quote! { ref #fieldname }
                            }   
                            );
                        let fields_serialized = (0..fields_unnamed.unnamed.len()).map(
                            |idx|{let nm=syn::Ident::from("x".to_string()+&idx.to_string());quote!{ #nm.serialize(serializer); }});
                        output.push(quote!(#variant_name_spanned(#(#fields_names,)*) => { serializer.write_u8(#var_idx); #(#fields_serialized)*  } ) );
                    },
                    &syn::Fields::Unit => {
                        output.push(quote!( #variant_name_spanned => { serializer.write_u8(#var_idx) } ) );
                    },
        			_ => panic!("Unnamed fields not supported")
        		}
        	}
        	quote! {
			    	impl #impl_generics #serialize for #name #ty_generics #where_clause {
				    	fn serialize(&self, serializer: &mut #serializer) {
			            	//println!("Serializer running on {} : {:?}", stringify!(#name), self);
			            	match self {
			            		#(#output,)*
			            	}
				    	}
			        }
			    }

        },
        &syn::Data::Struct(ref struc) =>
        	match &struc.fields {
        		&syn::Fields::Named(ref namedfields) => {
        			let mut output=Vec::new();
        			for ref field in &namedfields.named {
                        
                        {
                            let verinfo=parse_attr_tag(&field.attrs, &field.ty);
                            let (field_from_version,field_to_version,default_trait,default_val) = 
                                (verinfo.version_from,verinfo.version_to,verinfo.default_trait,verinfo.default_val);
                                
                            let id=field.ident.unwrap();
                            output.push(quote!(
                                if server >= #field_from_version && server <= #field_to_version {
                                    self.#id.serialize(serializer);
                                }                            ));
                        }

        			}
        			quote! {
				    	impl #impl_generics #serialize for #name #ty_generics #where_clause {
					    	fn serialize(&self, serializer: &mut #serializer) {
				            	//println!("Serializer running on {}", stringify!(#name));
                                let server=serializer.version;
				            	#(#output)*
					    	}
				        }
				    }
        		}
        		,
        		_ => panic!("Only regular structs supported, not tuple structs.")
        	}
        _ => {
        	panic!("Only regular structs are supported");
        }
    };

    //println!("Emitting: {:?}",expanded);
 	expanded.into()
}


#[proc_macro_derive(Deserialize)]
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

	let mut output=Vec::new();
    let expanded = match &input.data {
        &syn::Data::Enum(ref enum1) => 
        {
            let mut output=Vec::new();
            let variant_count = enum1.variants.len();
            if variant_count>255 {
                panic!("This library is not capable of deserializing enums with more than 255 variants. My deepest apologies!");                
            }

            for (var_idx,ref variant) in enum1.variants.iter().enumerate() {
                let var_idx=var_idx as u8;
                let var_ident=variant.ident;
                let variant_name=quote!{ #name::#var_ident };
                let variant_name_spanned=quote_spanned! { span => #variant_name};
                match &variant.fields {
                    &syn::Fields::Named(ref fields_named) => {
                        
                        //let fields_names=fields_named.named.iter().map(|x|x.ident.unwrap());
                        let fields_deserialized = fields_named.named.iter().map(
                            |f|{
                                let ty=&f.ty;
                                let ty=quote_spanned! { span => #ty };
                                let field_name=f.ident.unwrap();
                                let field_name=quote_spanned! { span => #field_name};
                                quote!{ #field_name: #ty::deserialize(deserializer) } 
                            });
                        output.push(quote!( #var_idx => #variant_name_spanned{ #(#fields_deserialized,)* } ) );
                        
                    },
                    &syn::Fields::Unnamed(ref fields_unnamed) => {
                        //let fields_names=fields_unnamed.unnamed.iter().enumerate().map(|(idx,x)|"x".to_string()+&idx.to_string());
                        let fields_deserialized = fields_unnamed.unnamed.iter().map(
                            |f|{let ty=&f.ty;quote!{ #ty::deserialize(deserializer) }});
                        output.push(quote!( #var_idx => #variant_name_spanned( #(#fields_deserialized,)*) ) );
                    },
                    &syn::Fields::Unit => {
                        output.push(quote!( #var_idx => #variant_name_spanned ) );
                    },
                    _ => panic!("Unnamed fields not supported")
                }
            }
            quote! {
                impl #impl_generics #deserialize for #name #ty_generics #where_clause {
                    fn deserialize(deserializer: &mut #deserializer) -> Self {
                        //println!("Deserializer running on {}", stringify!(#name));
                        match deserializer.read_u8() {
                            #(#output,)*
                            _ => panic!("Corrupt file - unknown enum variant detected.")
                        }
                    }
                }
            }
        },
        &syn::Data::Struct(ref struc) =>
        	match &struc.fields {
        		&syn::Fields::Named(ref namedfields) => {
        			for ref field in &namedfields.named {
;        				let id = field.ident.unwrap();
    					let field_type = &field.ty;
    					let id_spanned=quote_spanned! { span => #id};
    					let local_deserializer = quote_spanned! { defspan => local_deserializer};
                                

                        let verinfo=parse_attr_tag(&field.attrs, &field.ty);
                        let (field_from_version,field_to_version,default_trait,default_val) = 
                            (verinfo.version_from,verinfo.version_to,verinfo.default_trait,verinfo.default_val);

                        let effective_default_val = if let Some(defval) = default_val {
                            quote! { str::parse(#defval).unwrap() }
                        } else if let Some(deftrait) = default_trait {
                            quote! { #deftrait::default() }
                        } else {
                            quote! { panic!("internal error - there was no default value available for field.") }
                        };


    					let src = 

                            if field_from_version==0 && field_to_version==std::u32::MAX {
                                quote_spanned! { span => 
                                        #field_type::deserialize(#local_deserializer)
                                    }
                            } else {

                                quote_spanned! { span => 
                                if #local_deserializer.file_version >= #field_from_version && #local_deserializer.file_version <= #field_to_version {
                                    #field_type::deserialize(#local_deserializer)
                                } else { 
                                    #effective_default_val                                
                                }}
                            };



                        output.push(
                            quote!(#id_spanned : #src ));
        				

        			};
                    quote! {

                            impl #impl_generics #deserialize for #name #ty_generics #where_clause {
                            fn deserialize(deserializer: &mut #deserializer) -> Self {
                                let local_deserializer = deserializer;
                                //println!("Deserializer running on {}", stringify!(#name));
                                #name {
                                    #(#output,)*
                                }                           
                            }
                        }
                    }                    
        		}
                ,
        		_ => panic!("Only regular structs supported, not tuple structs.")
        	}
        _ => {
        	panic!("Only regular structs are supported");
        }
    };


 	expanded.into()
}
