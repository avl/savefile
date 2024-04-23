use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{ToTokens};
use syn::{GenericArgument, ParenthesizedGenericArguments, Path, PathArguments, ReturnType, Type, TypeParamBound};
use common::{compile_time_check_reprc, compile_time_size};



const POINTER_SIZE:usize = std::mem::size_of::<*const ()>();
#[allow(unused)]
const FAT_POINTER_SIZE:usize = 2*POINTER_SIZE;
#[allow(unused)]
const FAT_POINTER_ALIGNMENT:usize = POINTER_SIZE;

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

pub(crate) enum ArgType {
    PlainData(Type),
    Reference(TokenStream),
    SliceReference(TokenStream),
    Str,
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

pub(crate) fn parse_box_type(version:u32, path: &Path, method_name: &Ident, arg_name: &str, typ: &Type,
                  name_generator: &mut impl FnMut() -> String,
                  extra_definitions: &mut Vec<TokenStream>,
                  is_reference: bool,
                  is_mut_ref: bool,
) -> ArgType
{
    if path.segments.len()!=1 {
        panic!("Savefile does not support types named 'Box', unless they are the standard type Box, and it must be specified as 'Box', without any namespace");
    }
    let last_seg = path.segments.iter().last().unwrap();
    match &last_seg.arguments {
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
                                return ArgType::PlainData(typ.clone());
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
}

#[allow(clippy::too_many_arguments)]
fn parse_type(
    version: u32,
    arg_name: &str,
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
            rawtype = typ;
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
                &typref.elem,
                method_name,
                &mut *name_generator,
                extra_definitions,
                true,
                typref.mutability.is_some()
            );
        }
        Type::Tuple(tuple) => {
            if tuple.elems.len() > 3 {
                panic!("Savefile presently only supports tuples up to 3 members. Either change to using a struct, or file an issue on savefile!");
            }
            rawtype = typ;
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
                    .map(|x| match x {
                        TypeParamBound::Trait(t) => t
                            .path
                            .segments
                            .iter()
                            .last()
                            .expect("Missing bounds of Box trait object"),
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
                                pararg.inputs.iter().cloned().collect(),
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
            let last_seg = path.path.segments.iter().last().expect("Missing path segments");
            if last_seg.ident == "str" {
                if path.path.segments.len()!=1 {
                    panic!("Savefile does not support types named 'str', unless they are the standard type str, and it must be specified as 'str', without any namespace");
                }
                if !is_reference {
                    panic!("Savefile does not support the type 'str' (but it does support '&str').");
                }
                return ArgType::Str;
            }
            else
            if last_seg.ident == "Box" {
                if is_reference {
                    panic!("Savefile does not support reference to Box. This is also generally not very useful, just use a regular reference for arguments.");
                }
                return parse_box_type(version,&path.path, method_name, arg_name, typ, name_generator, extra_definitions, is_reference, is_mut_ref);
            } else {
                rawtype = typ;
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
        ArgType::PlainData(rawtype.clone())
    } else {
        if is_mut_ref {
            panic!("Method {}, argument {}: Mutable references are not supported by Savefile-abi (except for FnMut-trait objects): {}", method_name, arg_name, typ.to_token_stream());
        }
        ArgType::Reference(rawtype.to_token_stream())
    }
}

struct TypeInstruction {
    callee_trampoline_real_method_invocation_argument1: TokenStream,
    callee_trampoline_temp_variable_declaration1: TokenStream,
    callee_trampoline_variable_deserializer1: TokenStream,
    caller_arg_serializer1: TokenStream,
    caller_fn_arg1: TokenStream,
    schema: TokenStream,
    can_be_sent_as_ref: bool,

    known_size_align1: Option<(usize,usize)>,
    /// The type that this parameter is primarily deserialized into, on the
    /// deserialized side (i.e, callee for arguments, caller for return value);
    deserialized_type: TokenStream,
}


impl ArgType {
    fn get_instruction(&self, arg_index: Option<usize>, arg_name: &Ident) -> TypeInstruction {
        let temp_arg_name = Ident::new(&format!("temp_{}", arg_name), Span::call_site());

        let layout_compatible = if let Some(arg_index) = arg_index {
            quote!(compatibility_mask&(1<<#arg_index) != 0)
        } else {
            quote!( false )
        };
        match self {
            ArgType::Reference(arg_type) => {

                TypeInstruction {
                    callee_trampoline_real_method_invocation_argument1: quote! {&#arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {let #temp_arg_name;},
                    deserialized_type: quote!{#arg_type},
                    callee_trampoline_variable_deserializer1: quote! {
                        if #layout_compatible {
                            unsafe { &*(deserializer.read_raw_ptr::<#arg_type>()?) }
                        } else {
                            #temp_arg_name = <#arg_type as Deserialize>::deserialize(&mut deserializer)?;
                            &#temp_arg_name
                        }
                    },
                    caller_arg_serializer1: quote! {
                        if #layout_compatible {
                            unsafe { serializer.write_raw_ptr(#arg_name as *const #arg_type).expect("Writing argument ref") };
                            Ok(())
                        } else {
                            #arg_name.serialize(&mut serializer)
                        }
                    },
                    schema: quote!{<#arg_type as WithSchema>::schema(version)},
                    caller_fn_arg1: quote! {#arg_name : &#arg_type},
                    can_be_sent_as_ref: true,

                    known_size_align1: None,
                }
            }
            ArgType::Str => {

                TypeInstruction {
                    callee_trampoline_real_method_invocation_argument1: quote! {&#arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {let #temp_arg_name;},
                    deserialized_type: quote!{String},
                    callee_trampoline_variable_deserializer1: quote! {
                        if #layout_compatible {
                            unsafe { &*(deserializer.read_raw_ptr::<str>()?) }
                        } else {
                            #temp_arg_name = String::deserialize(&mut deserializer)?;
                            &#temp_arg_name
                        }
                    },
                    caller_arg_serializer1: quote! {
                        if #layout_compatible {
                            unsafe { serializer.write_raw_ptr(#arg_name as *const str) }
                        } else {
                            (#arg_name.to_string()).serialize(&mut serializer)
                        }
                    },
                    caller_fn_arg1: quote! {#arg_name : &str},
                    schema: quote!( <&str as WithSchema>::schema(version) ),
                    can_be_sent_as_ref: true,

                    known_size_align1: None,
                }
            }
            ArgType::SliceReference(arg_type) => {

                TypeInstruction {
                    callee_trampoline_real_method_invocation_argument1: quote! {&#arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {let #temp_arg_name;},
                    deserialized_type: quote!{Vec<_>},
                    callee_trampoline_variable_deserializer1: quote! {
                        if #layout_compatible {
                            unsafe { &*(deserializer.read_raw_ptr::<[#arg_type]>()?) }
                        } else {
                            #temp_arg_name = deserialize_slice_as_vec::<_,#arg_type>(&mut deserializer)?;
                            &#temp_arg_name
                        }
                    },
                    caller_arg_serializer1: quote! {
                        if #layout_compatible  {
                            unsafe { serializer.write_raw_ptr(#arg_name as *const [#arg_type]) }
                        } else {
                            (&#arg_name).serialize(&mut serializer)
                        }
                    },
                    caller_fn_arg1: quote! {#arg_name : &[#arg_type]},
                    schema: quote!( <&[#arg_type] as WithSchema>::schema(version) ),
                    can_be_sent_as_ref: true,
                    known_size_align1: None,
                }
            }
            ArgType::PlainData(arg_type) => {
                TypeInstruction {
                    deserialized_type: quote!{#arg_type},
                    callee_trampoline_real_method_invocation_argument1: quote! {#arg_name},
                    callee_trampoline_temp_variable_declaration1: quote!(),
                    callee_trampoline_variable_deserializer1: quote! {
                        <#arg_type as Deserialize>::deserialize(&mut deserializer)?
                    },
                    caller_arg_serializer1: quote! {
                        #arg_name.serialize(&mut serializer)
                    },
                    caller_fn_arg1: quote! {#arg_name : #arg_type},
                    can_be_sent_as_ref: false,

                    schema: quote!( <#arg_type as WithSchema>::schema(version) ),
                    known_size_align1: if compile_time_check_reprc(arg_type) {
                        compile_time_size(arg_type)
                    } else { None },
                }
            }
            ArgType::BoxedTrait(trait_name) => {
                let trait_type = trait_name;
                TypeInstruction {
                    deserialized_type: quote!{Box<AbiConnection<dyn #trait_name>>},
                    callee_trampoline_real_method_invocation_argument1: quote! {#arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {let #temp_arg_name;},
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            #temp_arg_name = unsafe { PackagedTraitObject::deserialize(&mut deserializer)? };
                            Box::new(unsafe { AbiConnection::<dyn #trait_name>::from_raw_packaged(#temp_arg_name, Owning::Owned)? } )
                        }
                    },
                    caller_arg_serializer1: quote! {
                        PackagedTraitObject::new::<dyn #trait_type>(#arg_name).serialize(&mut serializer)
                    },
                    caller_fn_arg1: quote! {#arg_name : Box<dyn #trait_name>},
                    schema: quote!( Schema::BoxedTrait(<dyn #trait_name as AbiExportable>::get_definition(version)) ),
                    can_be_sent_as_ref: true,

                    known_size_align1: None,
                }
            }
            ArgType::TraitReference(trait_name, ismut) => {

                let trait_type = trait_name;

                let newsymbol = quote! {new_from_ptr};

                let mutsymbol = if *ismut {
                    quote!(mut)
                } else {
                    quote! {}
                };

                TypeInstruction {
                    deserialized_type: quote!{unreachable!()},
                    callee_trampoline_real_method_invocation_argument1: quote! {#arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {let #mutsymbol #temp_arg_name;},
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            if !#layout_compatible {
                                panic!("Function arg is not layout-compatible!")
                            }
                            #temp_arg_name = unsafe { AbiConnection::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, Owning::NotOwned)? };
                            & #mutsymbol #temp_arg_name
                        }
                    },
                    caller_arg_serializer1: quote! {
                        {
                            if !#layout_compatible {
                                panic!("Function arg is not layout-compatible!")
                            }
                            PackagedTraitObject::#newsymbol::<dyn #trait_type>( unsafe { std::mem::transmute(#arg_name) } ).serialize(&mut serializer)
                        }
                    },
                    caller_fn_arg1: if *ismut {
                        quote! {#arg_name : &mut dyn #trait_name }
                    } else {
                        quote! {#arg_name : &dyn #trait_name }
                    },
                    schema: quote!( Schema::BoxedTrait(<dyn #trait_name as AbiExportable>::get_definition(version)) ),
                    can_be_sent_as_ref: true,

                    known_size_align1: Some((FAT_POINTER_SIZE+POINTER_SIZE,FAT_POINTER_ALIGNMENT)),
                }
            }
            ArgType::Fn(temp_trait_name, fndef, args, ismut) => {
                let temp_arg_name2 = Ident::new(&format!("temp2_{}", arg_name), Span::call_site());

                let temp_trait_type = temp_trait_name;

                let temp_trait_name_wrapper = Ident::new(&format!("{}_wrapper", temp_trait_type), Span::call_site());

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
                let newsymbol = quote! {new_from_ptr};

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

                TypeInstruction {
                    deserialized_type: quote!{unreachable!()},
                    callee_trampoline_real_method_invocation_argument1: quote! {#arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {
                        let #mutsymbol #temp_arg_name;
                        let #mutsymbol #temp_arg_name2;
                    },
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            if !#layout_compatible {
                                panic!("Function arg is not layout-compatible!")
                            }

                            #temp_arg_name = unsafe { AbiConnection::<#temp_trait_type>::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, Owning::NotOwned)? };
                            #temp_arg_name2 = |#(#typedarglist,)*| {#temp_arg_name.docall(#(#arglist,)*)};
                            & #mutsymbol #temp_arg_name2
                        }
                    },
                    caller_arg_serializer1: quote! {
                        {
                            if !#layout_compatible {
                                panic!("Function arg is not layout-compatible!")
                            }

                            let #mutsymbol temp = #temp_trait_name_wrapper { func: #arg_name as *#mutorconst _ };
                            let #mutsymbol temp : *#mutorconst (dyn #temp_trait_type+'_) = &#mutsymbol temp as *#mutorconst _;
                            PackagedTraitObject::#newsymbol::<(dyn #temp_trait_type+'_)>( unsafe { std::mem::transmute(temp)} ).serialize(&mut serializer)
                        }
                    },
                    caller_fn_arg1: if *ismut {
                        quote! {#arg_name : &mut dyn #fndef }
                    } else {
                        quote! {#arg_name : &dyn #fndef }
                    },
                    schema: quote!( Schema::FnClosure(#ismut, <dyn #temp_trait_name as AbiExportable >::get_definition(version)) ),
                    can_be_sent_as_ref: true,
                    known_size_align1: Some((FAT_POINTER_SIZE+POINTER_SIZE,FAT_POINTER_ALIGNMENT)),
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_method_definitions(
    version: u32,
    trait_name: Ident,
    method_number: u16,
    method_name: Ident,
    ret_declaration: TokenStream, //May be empty, for ()-returns
    ret_type: Type,
    no_return: bool, //Returns ()
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

    let mut compile_time_known_size = Some(0);
    for (arg_index, (arg_name, typ)) in args.iter().enumerate() {
        let argtype = parse_type(
            version,
            &arg_name.to_string(),
            typ,
            &method_name,
            &mut *name_generator,
            extra_definitions,
            false,
            false,
        );
        callee_trampoline_variable_declaration.push(quote! {let #arg_name;});

        let instruction = argtype.get_instruction(Some(arg_index), arg_name);

        callee_trampoline_real_method_invocation_arguments.push(instruction.callee_trampoline_real_method_invocation_argument1);
        callee_trampoline_temp_variable_declaration.push(instruction.callee_trampoline_temp_variable_declaration1);

        let deserializer_expression = instruction.callee_trampoline_variable_deserializer1;

        callee_trampoline_variable_deserializer.push( quote!( #arg_name = #deserializer_expression ; ) );
        let arg_serializer = instruction.caller_arg_serializer1;
        caller_arg_serializers.push(
            quote!{
                #arg_serializer.expect("Failed while serializing");
            }
        );
        caller_fn_arg_list.push(instruction.caller_fn_arg1);
        let schema = instruction.schema;
        let can_be_sent_as_ref = instruction.can_be_sent_as_ref;
        metadata_arguments.push(quote!{
                        AbiMethodArgument {
                            schema: #schema,
                            can_be_sent_as_ref: #can_be_sent_as_ref
                        }
        });
        if let Some(total_size) = &mut compile_time_known_size {
            if let Some((known_size,_known_align)) = instruction.known_size_align1 {
                *total_size += known_size;
            } else {
                compile_time_known_size = None;
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
    let return_value_schema;


    let caller_return_type;
    let ret_deserializer ;
    let ret_temp_decl;
    let ret_serialize;

    let result_default;
    if no_return {
        return_value_schema = quote!( <() as WithSchema>::schema(0) );
        ret_deserializer = quote!( () ); //Zero-sized, no deserialize actually needed
        ret_serialize = quote!( () );
        caller_return_type = quote!( () );
        ret_temp_decl = quote!();
        result_default = quote!( MaybeUninit::<Result<#ret_type,SavefileError>>::new(Ok(())) ); //Safe, does not need drop and does not allocate
    } else {
        let parsed_ret_type = parse_type(version, "___retval",&ret_type,&method_name,name_generator,extra_definitions,false,false);
        let instruction = parsed_ret_type.get_instruction(None, &Ident::new("ret", Span::call_site()));
        caller_return_type = instruction.deserialized_type;
        return_value_schema = instruction.schema;
        ret_deserializer = instruction.callee_trampoline_variable_deserializer1;
        let ret_serializer = instruction.caller_arg_serializer1;
        ret_temp_decl = instruction.callee_trampoline_temp_variable_declaration1;

        ret_serialize = quote!( #ret_serializer );

        result_default = quote!( MaybeUninit::<Result<#caller_return_type,SavefileError>>::uninit() );
    };


    let arg_buffer;
    let data_as_ptr;
    let data_length;
    if let Some(compile_time_known_size) = compile_time_known_size {
        // If we have simple type such as u8, u16 etc, we can sometimes
        // know at compile-time what the size of the args will be.
        // If the rust-compiler offered 'introspection', we could do this
        // for many more types. But we can at least do it for the most simple.

        let compile_time_known_size = compile_time_known_size + 4; //Space for 'version'
        arg_buffer = quote!{
            let mut rawdata = [0u8;#compile_time_known_size];
            let mut data = Cursor::new(&mut rawdata[..]);
        };
        data_as_ptr = quote!( rawdata[..].as_ptr() );
        data_length = quote!( #compile_time_known_size );
    } else {
        arg_buffer = quote!( let mut data = FlexBuffer::new(); );
        data_as_ptr = quote!( data.as_ptr() as *const u8 );
        data_length = quote!( data.len() );

    }


    let _ = ret_deserializer;
    let _ = caller_return_type;
    let caller_method_trampoline = quote! {
        fn #method_name(& #receiver_mut self, #(#caller_fn_arg_list,)*) #ret_declaration {
            let info: &AbiConnectionMethod = &self.template.methods[#method_number as usize];

            let Some(callee_method_number) = info.callee_method_number else {
                panic!("Method '{}' does not exist in implementation.", info.method_name);
            };

            let mut result_buffer = #result_default;
            let compatibility_mask = info.compatibility_mask;

            #arg_buffer

            let mut serializer = Serializer {
                writer: &mut data,
                file_version: self.template.effective_version,
            };
            serializer.write_u32(self.template.effective_version).unwrap();
            #(#caller_arg_serializers)*

            unsafe {

                unsafe extern "C" fn abi_result_receiver(
                    outcome: *const RawAbiCallResult,
                    result_receiver: *mut (),
                ) {
                    let outcome = unsafe { &*outcome };
                    let result_receiver = unsafe { &mut *(result_receiver as *mut std::mem::MaybeUninit<Result<#caller_return_type, SavefileError>>) };
                    result_receiver.write(
                        parse_return_value_impl(outcome, |mut deserializer|{

                            #ret_temp_decl
                            Ok(#ret_deserializer)
                            //T::deserialize(deserializer)
                        })
                    );
                }

            (self.template.entry)(AbiProtocol::RegularCall {
                trait_object: self.trait_object,
                compatibility_mask: compatibility_mask,
                method_number: callee_method_number,
                effective_version: self.template.effective_version,
                data: #data_as_ptr,
                data_length: #data_length,
                abi_result: &mut result_buffer as *mut _ as *mut (),
                receiver: abi_result_receiver,
            });
            }
            let resval = unsafe { result_buffer.assume_init() };

            resval.expect("Unexpected panic in invocation target")
        }
    };

    let method_metadata = quote! {
        AbiMethod {
            name: #method_name_str.to_string(),
            info: AbiMethodInfo {
                return_value: #return_value_schema,
                arguments: vec![ #(#metadata_arguments,)* ],
            }
        }
    };



    let handle_retval;
    if no_return {
        handle_retval = quote!();
    } else {

        let ret_buffer;
        let data_as_ptr;
        let data_length;
        let known_size = compile_time_check_reprc(&ret_type).then_some(compile_time_size(&ret_type)).flatten();
        if let Some((compile_time_known_size,_align)) = known_size {
            // If we have simple type such as u8, u16 etc, we can sometimes
            // know at compile-time what the size of the args will be.
            // If the rust-compiler offered 'introspection', we could do this
            // for many more types. But we can at least do it for the most simple.

            let compile_time_known_size = compile_time_known_size + 4; //Space for 'version'
            ret_buffer = quote!{
            let mut rawdata = [0u8;#compile_time_known_size];
            let mut data = Cursor::new(&mut rawdata[..]);
        };
            data_as_ptr = quote!( rawdata[..].as_ptr() );
            data_length = quote!( #compile_time_known_size );
        } else {
            ret_buffer = quote!( let mut data = FlexBuffer::new(); );
            data_as_ptr = quote!( data.as_ptr() as *const u8 );
            data_length = quote!( data.len() );

        }

        handle_retval = quote!{
            #ret_buffer
            let mut serializer = Serializer {
                writer: &mut data,
                file_version: #version,
            };
            serializer.write_u32(effective_version)?;
            match #ret_serialize
            {
                Ok(()) => {
                    let outcome = RawAbiCallResult::Success {data: #data_as_ptr, len: #data_length};
                    unsafe { receiver(&outcome as *const _, abi_result) }
                }
                Err(err) => {
                    let err_str = format!("{:?}", err);
                    let outcome = RawAbiCallResult::AbiError(AbiErrorMsg{error_msg_utf8: err_str.as_ptr(), len: err_str.len()});
                    unsafe { receiver(&outcome as *const _, abi_result) }
                }
            }
        }
    }

    let callee_method_trampoline = quote! {
        #method_number => {
            #(#callee_trampoline_variable_declaration)*
            #(#callee_trampoline_temp_variable_declaration)*

            #(#callee_trampoline_variable_deserializer)*

            let ret = #callee_real_method_invocation_except_args( #(#callee_trampoline_real_method_invocation_arguments,)* );

            #handle_retval

        }

    };
    MethodDefinitionComponents {
        method_metadata,
        callee_method_trampoline,
        caller_method_trampoline,
    }
}
