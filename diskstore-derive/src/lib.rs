#![feature(proc_macro)]
extern crate proc_macro;
extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;
use proc_macro::TokenStream;
use syn::{ DeriveInput };

#[proc_macro_derive(Serialize)]
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
        				if let Some(ref id) = field.ident {
        					output.push(quote!(self.#id.serialize(serializer);));
        				}

        			}
        			quote! {
				    	impl #impl_generics #serialize for #name #ty_generics #where_clause {
					    	fn serialize(&self, serializer: &mut #serializer) {
				            	//println!("Serializer running on {}", stringify!(#name));
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
    					let src = quote_spanned! { span => #field_type::deserialize(#local_deserializer)};
    					output.push(quote!(#id_spanned : #src));
        				

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
