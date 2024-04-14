use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::ToTokens;
use syn::{GenericArgument, ParenthesizedGenericArguments, PathArguments, ReturnType, Type, TypeParamBound};

fn emit_closure_helpers(
    version: u32,
    temp_trait_name: Ident,
    args: &ParenthesizedGenericArguments,
    ismut: bool,
    extra_definitions: &mut Vec<TokenStream>,
    fnkind: Ident,
) {
    let temp_trait_name_wrapper = Ident::new(&format!("{}_wrapper", temp_trait_name), Span::call_site());

    let mut formal_parameter_declarations = vec![];
    let mut parameter_types = vec![];
    let mut arg_names = vec![];

    for (arg_index, arg) in args.inputs.iter().enumerate() {
        let arg_name = Ident::new(&format!("x{}", arg_index), Span::call_site());
        formal_parameter_declarations.push(quote! {#arg_name : #arg});
        parameter_types.push(arg.to_token_stream());
        arg_names.push(arg_name.to_token_stream());
    }

    let ret_type;
    let ret_type_decl;

    if let ReturnType::Type(_, rettype) = &args.output {
        let typ = rettype.to_token_stream();
        ret_type = quote! {#typ};
        ret_type_decl = quote! { -> #typ };
    } else {
        ret_type = quote! { () };
        ret_type_decl = quote! {};
    }

    let version = Literal::u32_unsuffixed(version);

    let mutsymbol;
    let mutorconst;
    if ismut {
        mutsymbol = quote! {mut};
        mutorconst = quote! {mut};
    } else {
        mutsymbol = quote! {};
        mutorconst = quote! {const};
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

enum ArgType {
    PlainData(TokenStream),
    Reference(TokenStream),
    SliceReference(TokenStream),
    TraitReference(Ident, bool /*ismut*/),
    BoxedTrait(Ident),
    Fn(
        Ident,       /*Name of temporary trait generated to be able to handle Fn* as dyn TemporaryTrait. */
        TokenStream, /*full closure definition (e.g "Fn(u32)->u16")*/
        Vec<Type>,   /*arg types*/
        bool,        /*ismut*/
    ),
}

pub(crate) struct MethodDefinitionComponents {
    pub(crate) method_metadata: TokenStream,
    pub(crate) callee_method_trampoline: TokenStream,
    pub(crate) caller_method_trampoline: TokenStream,
}

fn parse_type(
    version: u32,
    arg_name: &Ident,
    typ: &Type,
    method_name: &Ident,
    name_generator: &mut impl FnMut() -> String,
    extra_definitions: &mut Vec<TokenStream>,
    is_reference: bool,
    is_mut_ref: bool,
) -> ArgType {
    let rawtype;
    match typ {
        Type::Tuple(tup) if tup.elems.is_empty() => {
            rawtype = typ.to_token_stream();
            //argtype = ArgType::PlainData(typ.to_token_stream());
        }
        Type::Reference(typref) => {
            if typref.lifetime.is_some() {
                panic!(
                    "Method {}, argument {}: Specifying lifetimes is not supported.",
                    method_name, arg_name
                );
            }
            if is_reference {
                panic!("Method {}, argument {}: Method arguments cannot be reference to reference in Savefile-abi. Try removing a '&' from the type: {}", method_name, arg_name, typ.to_token_stream());
            }
            return parse_type(
                version,
                arg_name,
                &*typref.elem,
                method_name,
                &mut *name_generator,
                extra_definitions,
                true,
                typref.mutability.is_some(),
            );
        }
        Type::Tuple(tuple) => {
            if tuple.elems.len() > 3 {
                panic!("Savefile presently only supports tuples up to 3 members. Either change to using a struct, or file an issue on savefile!");
            }
            rawtype = tuple.to_token_stream();
        }
        Type::Slice(slice) => {
            if !is_reference {
                panic!(
                    "Method {}, argument {}: Slices must always be behind references. Try adding a '&' to the type: {}",
                    method_name,
                    arg_name,
                    typ.to_token_stream()
                );
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
                let type_bounds: Vec<_> = trait_obj
                    .bounds
                    .iter()
                    .filter_map(|x| match x {
                        TypeParamBound::Trait(t) => Some(
                            t.path
                                .segments
                                .iter()
                                .last()
                                .expect("Missing bounds of Box trait object"),
                        ),
                        TypeParamBound::Lifetime(_) => {
                            panic!(
                                "Method {}, argument {}: Specifying lifetimes is not supported.",
                                method_name, arg_name
                            );
                        }
                    })
                    .collect();
                if type_bounds.len() == 0 {
                    panic!("Method {}, argument {}, unsupported trait object reference. Only &dyn Trait is supported. Encountered zero traits.", method_name, arg_name);
                }
                if type_bounds.len() > 1 {
                    panic!("Method {}, argument {}, unsupported Box-type. Only &dyn Trait> is supported. Encountered multiple traits: {:?}", method_name, arg_name, trait_obj);
                }
                let bound = type_bounds.into_iter().next().expect("Internal error, missing bounds");

                if bound.ident == "Fn" || bound.ident == "FnMut" || bound.ident == "FnOnce" {
                    if bound.ident == "FnOnce" {
                        panic!(
                            "Method {}, argument {}, FnOnce is not supported. Maybe you can use FnMut instead?",
                            method_name, arg_name
                        );
                    }

                    if bound.ident == "FnMut" && !is_mut_ref {
                        panic!("Method {}, argument {}: When using FnMut, it must be referenced using &mut, not &. Otherwise, it is impossible to call.", method_name, arg_name);
                    }
                    let fn_decl = bound.to_token_stream();
                    match &bound.arguments {
                        PathArguments::Parenthesized(pararg) => {
                            //pararg.inputs
                            let temp_name =
                                Ident::new(&format!("{}_{}", &name_generator(), arg_name), Span::call_site());
                            emit_closure_helpers(
                                version,
                                temp_name.clone(),
                                pararg,
                                is_mut_ref,
                                extra_definitions,
                                bound.ident.clone(),
                            );
                            return ArgType::Fn(
                                temp_name,
                                fn_decl,
                                pararg.inputs.iter().map(|x| x.clone()).collect(),
                                is_mut_ref,
                            );
                        }
                        _ => {
                            panic!("Fn/FnMut arguments must be enclosed in parenthesis")
                        }
                    }
                } else {
                    return ArgType::TraitReference(bound.ident.clone(), is_mut_ref);
                }
            } else {
                panic!(
                    "Method {}, argument {}, reference to trait objects without 'dyn' are not supported.",
                    method_name, arg_name
                );
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
                            GenericArgument::Type(angargs) => match angargs {
                                Type::TraitObject(trait_obj) => {
                                    if is_reference {
                                        panic!("Method {}, argument {}: Reference to boxed trait object is not supported by savefile. Try using a regular reference to the box content instead.", method_name, arg_name);
                                    }
                                    let type_bounds: Vec<_> = trait_obj
                                        .bounds
                                        .iter()
                                        .filter_map(|x| match x {
                                            TypeParamBound::Trait(t) => Some(
                                                t.path
                                                    .segments
                                                    .iter()
                                                    .last()
                                                    .cloned()
                                                    .expect("Missing bounds of Box trait object")
                                                    .ident
                                                    .clone(),
                                            ),
                                            TypeParamBound::Lifetime(_) => None,
                                        })
                                        .collect();
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
                                _ => {
                                    match parse_type(
                                        version,
                                        arg_name,
                                        angargs,
                                        method_name,
                                        &mut *name_generator,
                                        extra_definitions,
                                        is_reference,
                                        is_mut_ref,
                                    ) {
                                        ArgType::PlainData(_plain) => {
                                            rawtype = path.to_token_stream();
                                        }
                                        _ => {
                                            panic!(
                                                "Method {}, argument {}, unsupported Box-type: {:?}",
                                                method_name, arg_name, typ
                                            );
                                        }
                                    }
                                }
                            },
                            _ => {
                                panic!(
                                    "Method {}, argument {}, unsupported Box-type: {:?}",
                                    method_name, arg_name, typ
                                );
                            }
                        }
                    }
                    _ => {
                        panic!(
                            "Method {}, argument {}, unsupported Box-type: {:?}",
                            method_name, arg_name, typ
                        );
                    }
                }
            } else {
                rawtype = path.to_token_stream();
            }
        }
        _ => {
            panic!(
                "Method {}, argument {}, unsupported type: {:?}",
                method_name, arg_name, typ
            );
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

pub(super) fn generate_method_definitions(
    version: u32,
    trait_name: Ident,
    method_number: u16,
    method_name: Ident,
    ret_declaration: TokenStream, //May be empty, for ()-returns
    ret_type: TokenStream,
    receiver_is_mut: bool,
    args: Vec<(Ident, &Type)>,
    name_generator: &mut impl FnMut() -> String,
    extra_definitions: &mut Vec<TokenStream>,
) -> MethodDefinitionComponents {
    let method_name_str = method_name.to_string();

    let mut callee_trampoline_real_method_invocation_arguments: Vec<TokenStream> = vec![];
    let mut callee_trampoline_variable_declaration = vec![];
    let mut callee_trampoline_temp_variable_declaration = vec![];
    let mut callee_trampoline_variable_deserializer = vec![];
    let mut caller_arg_serializers = vec![];
    let mut caller_fn_arg_list = vec![];
    let mut metadata_arguments = vec![];

    for (arg_index, (arg_name, typ)) in args.iter().enumerate() {
        let argtype = parse_type(
            version,
            arg_name,
            *typ,
            &method_name,
            &mut *name_generator,
            extra_definitions,
            false,
            false,
        );

        //let num_mask = 1u64 << (method_number as u64);
        let temp_arg_name = Ident::new(&format!("temp_{}", arg_name), Span::call_site());
        let temp_arg_name2 = Ident::new(&format!("temp2_{}", arg_name), Span::call_site());
        match &argtype {
            ArgType::PlainData(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote! {#arg_name});
            }
            ArgType::Reference(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote! {&#arg_name});
                callee_trampoline_temp_variable_declaration.push(quote! {let #temp_arg_name;});
            }
            ArgType::SliceReference(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote! {&#arg_name});
                callee_trampoline_temp_variable_declaration.push(quote! {let #temp_arg_name;});
            }
            ArgType::BoxedTrait(_) => {
                callee_trampoline_real_method_invocation_arguments.push(quote! {#arg_name});
                callee_trampoline_temp_variable_declaration.push(quote! {let #temp_arg_name;});
            }
            ArgType::TraitReference(_, ismut) => {
                callee_trampoline_real_method_invocation_arguments.push(quote! {#arg_name});
                let mutsymbol = if *ismut {
                    quote!(mut)
                } else {
                    quote! {}
                };
                callee_trampoline_temp_variable_declaration.push(quote! {let #mutsymbol #temp_arg_name;});
            }
            ArgType::Fn(_, _, _, ismut) => {
                callee_trampoline_real_method_invocation_arguments.push(quote! {#arg_name});
                let mutsymbol = if *ismut {
                    quote!(mut)
                } else {
                    quote! {}
                };
                callee_trampoline_temp_variable_declaration.push(quote! {let #mutsymbol #temp_arg_name;});
                callee_trampoline_temp_variable_declaration.push(quote! {let #mutsymbol #temp_arg_name2;});
            }
        }
        callee_trampoline_variable_declaration.push(quote! {let #arg_name;});
        match &argtype {
            ArgType::Reference(arg_type) => {
                callee_trampoline_variable_deserializer.push(quote! {
                    if compatibility_mask&(1<<#arg_index) != 0 {
                        #arg_name = unsafe { &*(deserializer.read_raw_ptr::<#arg_type>()?) };
                    } else {
                        #temp_arg_name = <#arg_type as Deserialize>::deserialize(&mut deserializer)?;
                        #arg_name = &#temp_arg_name;
                    }
                });
                caller_arg_serializers.push(quote! {
                            if compatibility_mask&(1<<#arg_index) != 0 {
                                unsafe { serializer.write_raw_ptr(#arg_name as *const #arg_type).expect("Writing argument ref") };
                            } else {
                                #arg_name.serialize(&mut serializer).expect("Writing argument serialized");
                            }
                        });
            }
            ArgType::SliceReference(arg_type) => {
                callee_trampoline_variable_deserializer.push(quote! {
                    if compatibility_mask&(1<<#arg_index) != 0 {
                        #arg_name = unsafe { &*(deserializer.read_raw_ptr::<[#arg_type]>()?) };
                    } else {
                        #temp_arg_name = deserialize_slice_as_vec::<_,#arg_type>(&mut deserializer)?;
                        #arg_name = &#temp_arg_name;
                    }
                });
                caller_arg_serializers.push(quote! {
                            if compatibility_mask&(1<<#arg_index) != 0 {
                                unsafe { serializer.write_raw_ptr(#arg_name as *const [#arg_type]).expect("Writing argument ref") };
                            } else {
                                #arg_name.serialize(&mut serializer).expect("Writing argument serialized");
                            }
                        });
            }
            ArgType::PlainData(arg_type) => {
                callee_trampoline_variable_deserializer.push(quote! {
                    #arg_name = <#arg_type as Deserialize>::deserialize(&mut deserializer)?;
                });
                caller_arg_serializers.push(quote! {
                    #arg_name.serialize(&mut serializer).expect("Serializing arg");
                });
            }
            ArgType::BoxedTrait(trait_type) => {
                callee_trampoline_variable_deserializer.push(quote! {
                    // SAFETY
                    // Todo: Well, why exactly?
                    if compatibility_mask&(1<<#arg_index) == 0 {
                        panic!("Function arg is not layout-compatible!")
                    }
                    #temp_arg_name = unsafe { PackagedTraitObject::deserialize(&mut deserializer)? };
                    #arg_name = Box::new(unsafe { AbiConnection::from_raw_packaged(#temp_arg_name, Owning::Owned)? } );
                });
                caller_arg_serializers.push(quote! {
                            if compatibility_mask&(1<<#arg_index) == 0 {
                                panic!("Function arg is not layout-compatible!")
                            }
                            PackagedTraitObject::new::<dyn #trait_type>(#arg_name).serialize(&mut serializer).expect("PackagedTraitObject");
                        });
            }
            ArgType::TraitReference(trait_type, ismut) => {
                let mutsymbol = if *ismut {
                    quote! {mut}
                } else {
                    quote! {}
                };
                let newsymbol = if *ismut {
                    quote! {new_from_ptr}
                } else {
                    quote! {new_from_ptr}
                };
                callee_trampoline_variable_deserializer.push(quote! {
                            if compatibility_mask&(1<<#arg_index) == 0 {
                                panic!("Function arg is not layout-compatible!")
                            }
                            #temp_arg_name = unsafe { AbiConnection::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, Owning::NotOwned)? };
                            #arg_name = & #mutsymbol #temp_arg_name;
                        });
                caller_arg_serializers.push(quote! {
                            if compatibility_mask&(1<<#arg_index) == 0 {
                                panic!("Function arg is not layout-compatible!")
                            }
                            PackagedTraitObject::#newsymbol::<dyn #trait_type>( unsafe { std::mem::transmute(#arg_name) } ).serialize(&mut serializer).expect("PackagedTraitObject");
                        });
            }
            ArgType::Fn(temp_trait_type, _, args, ismut) => {
                let mutsymbol = if *ismut {
                    quote! {mut}
                } else {
                    quote! {}
                };
                let mutorconst = if *ismut {
                    quote! {mut}
                } else {
                    quote! {const}
                };
                let newsymbol = if *ismut {
                    quote! {new_from_ptr}
                } else {
                    quote! {new_from_ptr}
                };

                let temp_trait_name_wrapper = Ident::new(&format!("{}_wrapper", temp_trait_type), Span::call_site());

                let typedarglist: Vec<TokenStream> = args
                    .iter()
                    .enumerate()
                    .map(|(idx, typ)| {
                        let id = Ident::new(&format!("x{}", idx), Span::call_site());
                        quote! {#id : #typ}
                    })
                    .collect();

                let arglist: Vec<Ident> = (0..args.len())
                    .map(|idx| {
                        let id = Ident::new(&format!("x{}", idx), Span::call_site());
                        id
                    })
                    .collect();
                callee_trampoline_variable_deserializer.push(quote! {
                    if compatibility_mask&(1<<#arg_index) == 0 {
                        panic!("Function arg is not layout-compatible!")
                    }

                    #temp_arg_name = unsafe { AbiConnection::<#temp_trait_type>::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, Owning::NotOwned)? };
                    #temp_arg_name2 = |#(#typedarglist,)*| {#temp_arg_name.docall(#(#arglist,)*)};
                    #arg_name = & #mutsymbol #temp_arg_name2;
                });
                caller_arg_serializers.push(quote! {
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
                caller_fn_arg_list.push(quote! {#arg_name : &#arg_type});
                metadata_arguments.push(quote! {
                    AbiMethodArgument {
                        schema: <#arg_type as WithSchema>::schema(version),
                        can_be_sent_as_ref: true
                    }
                })
            }
            ArgType::SliceReference(arg_type) => {
                caller_fn_arg_list.push(quote! {#arg_name : &[#arg_type]});
                metadata_arguments.push(quote! {
                    AbiMethodArgument {
                        schema: <[#arg_type] as WithSchema>::schema(version),
                        can_be_sent_as_ref: true
                    }
                })
            }
            ArgType::PlainData(arg_type) => {
                caller_fn_arg_list.push(quote! {#arg_name : #arg_type});
                metadata_arguments.push(quote! {
                    AbiMethodArgument {
                        schema: <#arg_type as WithSchema>::schema(version),
                        can_be_sent_as_ref: false
                    }
                })
            }
            ArgType::BoxedTrait(trait_name) => {
                caller_fn_arg_list.push(quote! {#arg_name : Box<dyn #trait_name>});
                metadata_arguments.push(quote! {
                    AbiMethodArgument {
                        schema: Schema::BoxedTrait(<dyn #trait_name as AbiExportable>::get_definition(version)),
                        can_be_sent_as_ref: true
                    }
                })
            }
            ArgType::TraitReference(trait_name, ismut) => {
                if *ismut {
                    caller_fn_arg_list.push(quote! {#arg_name : &mut dyn #trait_name });
                } else {
                    caller_fn_arg_list.push(quote! {#arg_name : &dyn #trait_name });
                }

                metadata_arguments.push(quote! {
                    AbiMethodArgument {
                        schema: Schema::BoxedTrait(<dyn #trait_name as AbiExportable>::get_definition(version)),
                        can_be_sent_as_ref: true,
                    }
                })
            }
            ArgType::Fn(temp_trait_name, fndef, _, ismut) => {
                if *ismut {
                    caller_fn_arg_list.push(quote! {#arg_name : &mut dyn #fndef });
                } else {
                    caller_fn_arg_list.push(quote! {#arg_name : &dyn #fndef });
                }
                //let temp_trait_name_str = temp_trait_name.to_string();
                metadata_arguments.push(quote! {
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
        callee_real_method_invocation_except_args =
            quote! { unsafe { &mut *trait_object.as_mut_ptr::<dyn #trait_name>() }.#method_name };
    } else {
        callee_real_method_invocation_except_args =
            quote! { unsafe { &*trait_object.as_const_ptr::<dyn #trait_name>() }.#method_name };
    }

    //let receiver_mut_str = receiver_mut.to_string();
    let receiver_mut = if receiver_is_mut {
        quote!(mut)
    } else {
        quote! {}
    };
    let caller_method_trampoline = quote! {
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

    let method_metadata = quote! {
        AbiMethod {
            name: #method_name_str.to_string(),
            info: AbiMethodInfo {
                return_value: <#ret_type as WithSchema>::schema(version),
                arguments: vec![ #(#metadata_arguments,)* ],
            }
        }
    };

    let callee_method_trampoline = quote! {
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
