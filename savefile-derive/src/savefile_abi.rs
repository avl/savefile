use crate::common::{compile_time_check_reprc, compile_time_size};
use crate::savefile_abi::WrapperKey::FutureWrapper;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::ToTokens;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::AtomicU64;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Colon2;
use syn::{
    GenericArgument, Lifetime, Path, PathArguments, PathSegment, ReturnType, TraitBoundModifier, Type, TypeParamBound,
    TypeTuple,
};

const POINTER_SIZE: usize = std::mem::size_of::<*const ()>();
#[allow(unused)]
const FAT_POINTER_SIZE: usize = 2 * POINTER_SIZE;
#[allow(unused)]
const FAT_POINTER_ALIGNMENT: usize = POINTER_SIZE;

const MEGA_FAT_POINTER: (usize, usize) = (FAT_POINTER_SIZE + POINTER_SIZE, POINTER_SIZE);
const FAT_POINTER: (usize, usize) = (2 * POINTER_SIZE, POINTER_SIZE);

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub enum WrapperKey {
    FnWrapper(FnWrapperKey),
    FutureWrapper(FutureWrapperKey),
}

#[derive(Debug, Clone)]
struct FutureWrapperKey {
    output_type: TokenStream,
    send: bool,
    sync: bool,
    unpin: bool,
}

impl PartialEq for FutureWrapperKey {
    fn eq(&self, other: &Self) -> bool {
        self.output_type.to_string() == other.output_type.to_string()
    }
}
impl Eq for FutureWrapperKey {}
impl Hash for FutureWrapperKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.output_type.to_string().hash(state);
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub(crate) struct FnWrapperKey {
    fnkind: Ident,
    ret: Type,
    args: Vec<Type>,
    ismut: bool,
    owning: bool,
    sync: bool,
    send: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ClosureFutureWrapperNames {
    trait_name: Ident,
    wrapper_struct_name: Ident,
}
fn get_type(ret_type: &ReturnType) -> Type {
    match ret_type {
        ReturnType::Default => Type::Tuple(TypeTuple {
            paren_token: Default::default(),
            elems: Punctuated::new(),
        }),
        ReturnType::Type(_, typ) => (**typ).clone(),
    }
}

fn compile_time_abi_check_size(typ: &Type) -> Option<(usize, usize)> {
    if compile_time_check_reprc(typ) {
        return compile_time_size(typ);
    }

    match typ {
        Type::Path(path) => {
            if safe_is_well_known(&path.path, ["std", "option", "Option"]) {
                match &path.path.segments.last().unwrap().arguments {
                    PathArguments::AngleBracketed(typ) => {
                        if typ.args.len() == 1 {
                            match &typ.args[0] {
                                GenericArgument::Lifetime(_) => {}
                                GenericArgument::Type(ty) => {
                                    if let Some(sub) = compile_time_abi_check_size(ty) {
                                        if sub == (0, 1) {
                                            return Some((1, 1)); //size align of Option<()> is 1,1
                                        }
                                    }
                                }
                                GenericArgument::Const(_) => {}
                                GenericArgument::Binding(_) => {}
                                GenericArgument::Constraint(_) => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    None
}

static ID_GEN: AtomicU64 = AtomicU64::new(0);

fn emit_future_helpers(
    wrapped_in_pin: bool,
    output_type: TokenStream,
    extra_definitions: &mut HashMap<WrapperKey, (ClosureFutureWrapperNames, TokenStream)>,
    send: bool,
    sync: bool,
    unpin: bool,
) -> ClosureFutureWrapperNames {
    let key = WrapperKey::FutureWrapper(FutureWrapperKey {
        output_type: output_type.clone(),
        send,
        sync,
        unpin,
    });
    if let Some((names, _)) = extra_definitions.get(&key) {
        return names.clone();
    }

    let cnt = ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let futurer_wrapper_trait_name = Ident::new(&format!("__{}_future_wrapper", cnt), Span::call_site());
    let futurer_wrapper_struct_name = Ident::new(&format!("__{}_future_wrapper_struct", cnt), Span::call_site());
    let names = ClosureFutureWrapperNames {
        wrapper_struct_name: futurer_wrapper_struct_name.clone(),
        trait_name: futurer_wrapper_trait_name.clone(),
    };

    let futureWrapper = futurer_wrapper_trait_name;

    let unpin_impl = unpin.then(|| {
        quote! {
            impl Unpin for #futurer_wrapper_struct_name {
            }
        }
    });
    let unpin_bound = unpin.then(|| {
        quote! {
            + Unpin
        }
    });

    let send_impl = send.then(|| {
        quote! {
            unsafe impl Send for #futurer_wrapper_struct_name {
            }
        }
    });
    let send_bound = send.then(|| {
        quote! {
            + Send
        }
    });

    let sync_impl = sync.then(|| {
        quote! {
            unsafe impl Sync for #futurer_wrapper_struct_name {
            }
        }
    });
    let sync_bound = sync.then(|| {
        quote! {
            + Sync
        }
    });

    let box_fut_type = if wrapped_in_pin {
        quote! {
            Pin<Box<dyn Future<Output = #output_type> #send_bound #sync_bound #unpin_bound>>
        }
    } else {
        quote! {
            Box<dyn Future<Output = #output_type> #send_bound #sync_bound #unpin_bound>
        }
    };

    let sized = wrapped_in_pin.then(|| {
        quote! {
            : Sized
        }
    });

    let self_ref = if wrapped_in_pin {
        quote!(self)
    } else {
        quote!(unsafe { self.map_unchecked_mut(|s| &mut **s) })
    };

    let output = quote! {

        pub struct #futurer_wrapper_struct_name {
            future: AbiConnection<dyn #futureWrapper>
        }

        impl Unpin for #futurer_wrapper_struct_name {

        }

        #[savefile_abi_exportable(version = 0)]
        pub trait #futureWrapper {
            fn abi_poll(self: Pin<&mut Self>, waker: Box<dyn Fn()+Send+Sync>) -> ::std::option::Option<#output_type>;
        }
        #send_impl
        #sync_impl

        impl Future for #futurer_wrapper_struct_name {
            type Output = #output_type;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let mut waker = cx.waker().clone();
                //let mut pinned = std::pin::pin!(&mut self.future);
                match unsafe { self.map_unchecked_mut(|s|&mut s.future)}.abi_poll(Box::new(move ||{waker.wake_by_ref();})) {
                    Some(temp) => {
                        Poll::Ready(temp)
                    }
                    None => {
                        Poll::Pending
                    }
                }
            }
        }
        impl #futureWrapper for #box_fut_type {
            fn abi_poll(self: Pin<&mut Self>, waker: Box<dyn Fn()+Send+Sync>) -> ::std::option::Option<#output_type> {
                let waker = Waker::from(Arc::new(AbiWaker::new(waker)));
                let mut context = Context::from_waker(&waker);

                match #self_ref.poll(&mut context) {
                    Poll::Ready(t) => {
                        Some(t)
                    }
                    Poll::Pending => {
                        None
                    }
                }
            }
        }
    };
    extra_definitions.insert(key, (names.clone(), output));
    names
}

fn emit_closure_helpers(
    version: u32,
    args: &[Type],
    return_type: ReturnType,
    ismut: bool,
    extra_definitions: &mut HashMap<WrapperKey, (ClosureFutureWrapperNames, TokenStream)>,
    fnkind: &Ident, //Fn or FnMut
    owning: bool,
    sync: bool,
    send: bool,
) -> ClosureFutureWrapperNames /*wrapper name*/ {
    let key = WrapperKey::FnWrapper(FnWrapperKey {
        fnkind: fnkind.clone(),
        ismut,
        owning,
        args: args.iter().cloned().collect(),
        ret: get_type(&return_type),
        sync,
        send,
    });

    let syncsend_bound = match (sync, send) {
        (false, false) => quote!(),
        (true, false) => quote!( + Sync),
        (true, true) => quote!( + Sync + Send),
        (false, true) => quote!( + Send),
    };
    let syncsend_traitbound = match (sync, send) {
        (false, false) => quote!(),
        (true, false) => quote!( : Sync),
        (true, true) => quote!(  : Sync + Send),
        (false, true) => quote!( : Send),
    };

    if let Some((names, _)) = extra_definitions.get(&key) {
        return names.clone();
    }
    let cnt = ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp_trait_name = Ident::new(
        &format!("__{}_{}", cnt, if owning { "owning_" } else { "" }),
        Span::call_site(),
    );
    let temp_trait_name_wrapper = Ident::new(&format!("{}wrapper", temp_trait_name), Span::call_site());
    let names = ClosureFutureWrapperNames {
        wrapper_struct_name: temp_trait_name_wrapper.clone(),
        trait_name: temp_trait_name.clone(),
    };

    let mut formal_parameter_declarations = vec![];
    let mut parameter_types = vec![];
    let mut arg_names = vec![];

    for (arg_index, arg) in args.iter().enumerate() {
        let arg_name = Ident::new(&format!("x{}", arg_index), Span::call_site());
        formal_parameter_declarations.push(quote! {#arg_name : #arg});
        parameter_types.push(arg.to_token_stream());
        arg_names.push(arg_name.to_token_stream());
    }

    let ret_type;
    let ret_type_decl;

    if let ReturnType::Type(_, temp_type) = &return_type {
        let typ = &*temp_type;
        ret_type = quote! { #typ };
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

    let funcdef = if owning {
        quote!( Box <(dyn for<'x> #fnkind( #(#parameter_types,)* ) #ret_type_decl #syncsend_bound +'a)> )
    } else {
        quote!( *#mutorconst (dyn for<'x> #fnkind( #(#parameter_types,)* ) #ret_type_decl #syncsend_bound +'a) )
    };

    let expanded = quote! {

        #[savefile_abi_exportable(version=#version)]
        pub trait #temp_trait_name #syncsend_traitbound {
            fn docall(& #mutsymbol self, #(#formal_parameter_declarations,)*) -> #ret_type;
        }

        struct #temp_trait_name_wrapper<'a> {
            func: #funcdef
        }
        impl<'a> #temp_trait_name for #temp_trait_name_wrapper<'a> {
            fn docall(&#mutsymbol self, #(#formal_parameter_declarations,)*) -> #ret_type {
                unsafe { (&#mutsymbol *self.func)( #(#arg_names,)* )}
            }
        }

    };
    extra_definitions.insert(key, (names.clone(), expanded));
    return names;
}

#[derive(Debug)]
pub(crate) enum ArgType {
    PlainData(Type),
    Reference(
        Box<ArgType>,
        bool,             /*ismut (only trait objects can be mut here)*/
        Option<Lifetime>, /*lifetime*/
    ),
    Str(bool /*static*/),
    Boxed(Box<ArgType>),
    Slice(Box<ArgType>),
    Result(Box<ArgType>, Box<ArgType>),
    Trait(TokenStream, bool /*ismut self*/),
    Future(
        /*pin*/ bool,
        TokenStream,
        /*send*/ bool,
        /*sync*/ bool,
        /*unpin*/ bool,
    ),
    Fn(
        TokenStream, /*full closure definition (e.g "Fn(u32)->u16")*/
        Vec<Type>,   /*arg types*/
        ReturnType,  //Ret-type
        bool,        /*ismut (FnMut)*/
        bool,        /*sync*/
        bool,        /*send*/ //TODO: Create struct here
    ),
}

pub(crate) struct MethodDefinitionComponents {
    pub(crate) method_metadata: TokenStream,
    pub(crate) callee_method_trampoline: TokenStream,
    pub(crate) caller_method_trampoline: TokenStream,
}

pub(crate) fn parse_box_type(
    version: u32,
    path: &Path,
    method_name: &Ident,
    is_return_value: bool,
    arg_name: &str,
    typ: &Type,
    name_generator: &mut impl FnMut() -> String,
    extra_definitions: &mut HashMap<WrapperKey, (ClosureFutureWrapperNames, TokenStream)>,
    is_reference: bool,
    is_mut_ref: bool,
    is_box: bool,
) -> ArgType {
    let location;
    if is_return_value {
        location = format!("In return value of method '{}'", method_name);
    } else {
        location = format!("Method '{}', argument {}", method_name, arg_name);
    }

    if path.segments.len() != 1 {
        abort!(path.span(), "Savefile does not support types named 'Box', unless they are the standard type Box, and it must be specified as 'Box', without any namespace");
    }
    if is_reference {
        abort!(
            path.span(),
            "{}. Savefile does not support references to Boxes. Just supply a reference to the inner type: {}",
            location,
            typ.to_token_stream()
        );
    }

    let last_seg = path.segments.iter().last().unwrap();
    match &last_seg.arguments {
        PathArguments::AngleBracketed(ang) => {
            let first_gen_arg = ang.args.iter().next().expect("Missing generic args of Box");
            if ang.args.len() != 1 {
                abort!(ang.span(), "{}. Savefile requires Box arguments to have exactly one generic argument, a requirement not satisfied by type: {}", location, typ.to_token_stream());
            }

            match first_gen_arg {
                GenericArgument::Type(angargs) => {
                    match parse_type(
                        version,
                        arg_name,
                        angargs,
                        method_name,
                        is_return_value,
                        &mut *name_generator,
                        extra_definitions,
                        false,
                        is_mut_ref,
                        true,
                    ) {
                        ArgType::Future(pin, output, send, sync, unpin) => {
                            ArgType::Future(pin, output, send, sync, unpin)
                        }
                        ArgType::Result(_, _) => {
                            abort!(
                                first_gen_arg.span(),
                                "{}. Savefile does not support boxed results. Try boxing the contents of the result instead. I.e, instead of Box<Result<A,B>>, try Result<Box<A>,Box<B>>. Type encountered: {}",
                                location,
                                typ.to_token_stream()
                            )
                        }
                        ArgType::Boxed(_) => {
                            abort!(
                                first_gen_arg.span(),
                                "{}. Savefile does not support a Box containing another Box: {}",
                                location,
                                typ.to_token_stream()
                            )
                        }
                        ArgType::PlainData(_) | ArgType::Str(_) => {
                            return ArgType::PlainData(typ.clone()); //Box<plaintype> is itself a plaintype. So handle it as such. It can matter, if Box<T> implements Serializable, when T does not. (example: str)
                        }
                        ArgType::Slice(slicetype) => match &*slicetype {
                            ArgType::PlainData(_) => {
                                return ArgType::Slice(slicetype);
                            }
                            _x => abort!(
                                angargs.span(),
                                "{}. Savefile does not support a Box containing a slice of anything complex, like: {}",
                                location,
                                typ.to_token_stream()
                            ),
                        },
                        ArgType::Reference(_, _, _) => {
                            abort!(first_gen_arg.span(), "{}. Savefile does not support a Box containing a reference, like: {} (boxing a reference is generally a useless thing to do))", location, typ.to_token_stream());
                        }
                        x @ ArgType::Trait(_, _) | x @ ArgType::Fn(_, _, _, _, _, _) => ArgType::Boxed(Box::new(x)),
                    }
                }
                _ => {
                    abort!(
                        typ.span(),
                        "{}, unsupported Box-type: {}",
                        location,
                        typ.to_token_stream()
                    );
                }
            }
        }
        _ => {
            abort!(
                typ.span(),
                "{}, unsupported Box-type: {}",
                location,
                typ.to_token_stream()
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
    is_return_value: bool,
    name_generator: &mut impl FnMut() -> String,
    extra_definitions: &mut HashMap<WrapperKey, (ClosureFutureWrapperNames, TokenStream)>,
    is_reference: bool,
    is_mut_ref: bool,
    is_boxed: bool,
) -> ArgType {
    let location;
    if is_return_value {
        location = format!("In return value of method '{}'", method_name);
    } else {
        location = format!("Method '{}', argument {}", method_name, arg_name);
    }

    let rawtype;
    match typ {
        Type::Tuple(tup) if tup.elems.is_empty() => {
            rawtype = typ;
        }
        Type::Reference(typref) => {
            let is_static_lifetime;
            let mut lifetime = None;

            match &typref.lifetime {
                Some(rlifetime) if rlifetime.ident == "static" => {
                    lifetime = Some(rlifetime.clone());
                    is_static_lifetime = true;
                }
                Some(rlifetime) => {
                    lifetime = Some(rlifetime.clone());
                    is_static_lifetime = false;
                }
                None => {
                    is_static_lifetime = false;
                }
            }
            if is_reference || is_boxed {
                abort!(typref.and_token.span(), "{}: Method arguments cannot be reference to reference in savefile-abi. Try removing a '&' from the type: {}", location, typ.to_token_stream());
            }

            let inner = parse_type(
                version,
                arg_name,
                &typref.elem,
                method_name,
                is_return_value,
                &mut *name_generator,
                extra_definitions,
                true,
                typref.mutability.is_some(),
                is_boxed,
            );
            if let ArgType::Str(_) = inner {
                return ArgType::Str(is_static_lifetime); //Str is a special case, it is always a reference
            }
            return ArgType::Reference(Box::new(inner), typref.mutability.is_some(), lifetime);
        }
        Type::Tuple(tuple) => {
            if tuple.elems.len() > 3 {
                abort!(tuple.span(), "Savefile presently only supports tuples up to 3 members. Either change to using a struct, or file an issue on savefile!");
            }
            rawtype = typ;
        }
        Type::Slice(slice) => {
            if !is_reference || is_boxed {
                abort!(
                    slice.span(),
                    "{}: Slices must always be behind references. Try adding a '&' to the type: {}",
                    location,
                    typ.to_token_stream()
                );
            }
            if is_mut_ref {
                abort!(
                    typ.span(),
                    "{}: Mutable references are not supported by savefile-abi, except for FnMut-trait objects. {}",
                    location,
                    typ.to_token_stream()
                );
            }
            let argtype = parse_type(
                version,
                arg_name,
                &slice.elem,
                method_name,
                is_return_value,
                &mut *name_generator,
                extra_definitions,
                is_reference,
                is_mut_ref,
                is_boxed,
            );
            return ArgType::Slice(Box::new(argtype));
        }
        Type::TraitObject(trait_obj) => {
            if !is_reference && !is_boxed {
                abort!(
                    trait_obj.span(),
                    "{}: Trait objects must always be behind references or boxes. Try adding a '&' to the type: {}",
                    location,
                    typ.to_token_stream()
                );
            }
            if trait_obj.dyn_token.is_some() {
                let mut sync = false;
                let mut send = false;
                let mut unpin = false;
                let mut async_trait_lifetime = false;
                let type_bounds: Vec<_> = trait_obj
                    .bounds
                    .iter()
                    .map(|x| match x {
                        TypeParamBound::Trait(t) => Some(
                            t.path
                                .segments
                                .iter()
                                .last()
                                .expect("Missing bounds of Box trait object"),
                        ),
                        TypeParamBound::Lifetime(lt) => {
                            if lt.ident == "async_trait" {
                                async_trait_lifetime = true;
                                None
                            } else {
                                abort!(
                                    lt.span(),
                                    "{}: Specifying lifetimes is not supported by Savefile-Abi: {}",
                                    location,
                                    x.to_token_stream()
                                );
                            }
                        }
                    })
                    .filter_map(|x| x)
                    .filter(|seg| {
                        if seg.ident == "Sync" {
                            sync = true;
                            return false;
                        }
                        if seg.ident == "Send" {
                            send = true;
                            return false;
                        }
                        if seg.ident == "Unpin" {
                            unpin = true;
                            return false; //TODO: IS this safe?
                        }
                        true
                    })
                    .collect();
                if type_bounds.len() == 0 {
                    abort!(trait_obj.bounds.span(), "{}, unsupported trait object reference. Only &dyn Trait is supported. Encountered zero traits.", location);
                }
                if type_bounds.len() > 1 {
                    abort!(
                        trait_obj.bounds.span(),
                        "{}, unsupported Box-type. Only &dyn Trait> is supported. Encountered multiple traits: {:?}",
                        location,
                        trait_obj
                    );
                }
                let bound = type_bounds.into_iter().next().expect("Internal error, missing bounds");

                if bound.ident == "Fn" || bound.ident == "FnMut" || bound.ident == "FnOnce" {
                    if async_trait_lifetime {
                        abort!(
                            bound.ident.span(),
                            "{}, Savefile does presently not support lifetimes on Fn* closures.",
                            location,
                        );
                    }
                    if unpin {
                        abort!(
                            bound.ident.span(),
                            "{}, Savefile does presently not support Unpin bounds on closure types, only on futures.",
                            location,
                        );
                    }
                    if bound.ident == "FnOnce" {
                        abort!(
                            bound.ident.span(),
                            "{}, FnOnce is presently not supported by savefile-abi. Maybe you can use FnMut instead?",
                            location,
                        );
                    }
                    if bound.ident == "Fn" && is_mut_ref {
                        abort!(bound.ident.span(), "{}: Mutable references to Fn are not supported by savefile-abi. Try using a non-mutable reference instead..", location);
                    }

                    if bound.ident == "FnMut" && !is_boxed && !is_mut_ref {
                        abort!(bound.ident.span(), "{}: When using FnMut, it must be referenced using &mut or Box<..>, not &. Otherwise, it is impossible to call.", location);
                    }
                    let fn_decl = bound.to_token_stream();
                    match &bound.arguments {
                        PathArguments::Parenthesized(pararg) => {
                            /*let temp_name =
                            Ident::new(&format!("{}_{}", &name_generator(), arg_name), Span::call_site());*/
                            return ArgType::Fn(
                                fn_decl,
                                pararg.inputs.iter().cloned().collect(),
                                pararg.output.clone(),
                                bound.ident == "FnMut",
                                sync,
                                send,
                            );
                        }
                        _ => {
                            abort!(
                                bound.arguments.span(),
                                "Fn/FnMut arguments must be enclosed in parenthesis"
                            )
                        }
                    }
                } else if bound.ident == "Future" {
                    let bound = trait_obj.bounds.iter().next().unwrap();
                    match bound {
                        TypeParamBound::Trait(t) => {
                            if t.lifetimes.is_some() {
                                abort!(t.span(), "{}: Savefile does not support lifetimes in Futures", location);
                            }
                            match t.modifier {
                                TraitBoundModifier::None => {}
                                TraitBoundModifier::Maybe(q) => {
                                    abort!(q.span(), "{}: Unexpected ?-token", location)
                                }
                            }
                            if !is_boxed {
                                abort!(
                                    bound.span(),
                                    "{}: Savefile only supports boxed futures, not unboxed futures.",
                                    location
                                );
                            }
                            if !is_well_known(&t.path.segments, ["std", "future", "Future"]) {
                                abort!(
                                    bound.span(),
                                    "{}: Boxed futures can only implement the Future trait, not: {}",
                                    location,
                                    t.path.to_token_stream()
                                );
                            }
                            let seg = t.path.segments.last().unwrap();
                            match &seg.arguments {
                                PathArguments::AngleBracketed(arg) => {
                                    if arg.args.len() != 1 {
                                        abort!(
                                            arg.args.span(),
                                            "{}: Futures must have a a single Output bound.",
                                            location
                                        );
                                    }
                                    let output = &arg.args[0];

                                    match output {
                                        GenericArgument::Binding(t) => {
                                            if t.ident != "Output" {
                                                abort!(seg.ident.span(), "{}: Futures must have a a single binding, named Output (Future<Output=?>).", location);
                                            }
                                            return ArgType::Future(false, t.ty.to_token_stream(), send, sync, unpin);
                                        }
                                        GenericArgument::Lifetime(_)
                                        | GenericArgument::Const(_)
                                        | GenericArgument::Type(_)
                                        | GenericArgument::Constraint(_) => {
                                            abort!(output.span(), "{}: Futures must have a a single associated type binding, named 'Output' (Future<Output=?>).", location);
                                        }
                                    }
                                }
                                PathArguments::None | PathArguments::Parenthesized(_) => {
                                    abort!(
                                        seg.arguments.span(),
                                        "{}: Future must have arguments within angle brackets: Future<...>. Not: {}",
                                        location,
                                        seg.to_token_stream()
                                    );
                                }
                            }
                        }
                        TypeParamBound::Lifetime(_) => {
                            abort!(
                                bound.span(),
                                "{}: Savefile does not support lifetimes in Futures",
                                location
                            );
                        }
                    }
                    abort!(
                        trait_obj.span(),
                        "{}: Future did not have output type specified. Try adding Future<Output=mytype>.",
                        location
                    );
                } else {
                    if async_trait_lifetime {
                        abort!(
                            bound.ident.span(),
                            "{}, Savefile does presently not support lifetimes on arbitrary trait types, only on futures.",
                            location,
                        );
                    }
                    if unpin {
                        abort!(
                            bound.ident.span(),
                            "{}, Savefile does presently not support Unpin bounds on arbitrary trait types, only on futures.",
                            location,
                        );
                    }

                    if sync {
                        abort!(trait_obj.span(), "{}: Savefile does not support Send- or Sync-bounds on individual references to traits. Please make {} inherit Sync instead of adding the bound here, like so: trait {} : Sync.",
                            location, bound.ident, bound.ident);
                    }
                    if send {
                        abort!(trait_obj.span(), "{}: Savefile does not support Send- or Sync-bounds on individual references to traits. Please make {} inherit Send instead of adding the bound here, like so: trait {} : Send.",
                            location, bound.ident, bound.ident);
                    }
                    return ArgType::Trait(bound.to_token_stream(), is_mut_ref);
                }
            } else {
                abort!(
                    trait_obj.span(),
                    "{}, reference to trait objects without 'dyn' are not supported.",
                    location,
                );
            }
        }
        Type::Path(path) => {
            let last_seg = path.path.segments.iter().last().expect("Missing path segments");
            if last_seg.ident == "str" {
                if path.path.segments.len() != 1 {
                    abort!(path.path.segments.span(), "Savefile does not support types named 'str', unless they are the standard type str, and it must be specified as 'str', without any namespace");
                }
                if !is_reference {
                    abort!(
                        path.span(),
                        "Savefile does not support the type 'str' (but it does support '&str')."
                    );
                }
                return ArgType::Str(false); // This is a hack. ArgType::Str means '&str' everywhere but here, where it means 'str'
            } else if last_seg.ident == "Pin" {
                if !is_well_known(&path.path.segments, ["std", "pin", "Pin"]) {
                    abort!(path.path.segments.span(), "Savefile does not support types named 'Pin', unless it is the standard type Pin, and it must be specified as 'Pin', without any namespace");
                }
                if is_reference {
                    abort!(
                        last_seg.ident.span(),
                        "Savefile does not presently support reference to Pin"
                    );
                }
                match &last_seg.arguments {
                    PathArguments::Parenthesized(_) | PathArguments::None => {
                        abort!(last_seg.arguments.span(), "Savefile does not support types named 'Pin', unless they are the standard type Pin, and it must be specified as 'Pin<T>', with one type parameters within angle-brackets.");
                    }
                    PathArguments::AngleBracketed(params) => {
                        let argvec: Vec<_> = params.args.iter().collect();
                        if argvec.len() != 1 {
                            abort!(last_seg.arguments.span(), "Savefile does not support types named 'Pin', unless they are the standard type Pin, and it must be specified as 'Pin<T>', with a single type parameter within angle-brackets. Got {} type arguments", argvec.len());
                        }
                        let arg = argvec[0];
                        match arg {
                            GenericArgument::Type(argtyp) => {
                                let ty = parse_type(
                                    version,
                                    arg_name,
                                    argtyp,
                                    method_name,
                                    is_return_value,
                                    &mut *name_generator,
                                    extra_definitions,
                                    is_reference,
                                    is_mut_ref,
                                    is_boxed,
                                );
                                if let ArgType::Future(pin, typ, send, sync, unpin) = ty {
                                    if pin {
                                        abort!(arg.span(), "Savefile only supports Pin<Box<Future>>, not Pin<Pin<..>> or any other usage of Pin");
                                    }
                                    return ArgType::Future(true, typ, send, sync, unpin);
                                } else {
                                    abort!(
                                        arg.span(),
                                        "Savefile only supports Pin<Box<Future>>, not any other usage of Pin"
                                    );
                                }
                            }
                            GenericArgument::Lifetime(_) => {
                                abort!(arg.span(), "Savefile does not support lifetime specifications.");
                            }
                            GenericArgument::Const(_) => {
                                abort!(arg.span(), "Savefile does not support const in this location.");
                            }
                            GenericArgument::Binding(_) => {
                                abort!(arg.span(), "Savefile does not support the syntax expressed here.");
                            }
                            GenericArgument::Constraint(_) => {
                                abort!(arg.span(), "Savefile does not support constraints at this position.");
                            }
                        }
                        abort!(
                            last_seg.arguments.span(),
                            "Savefile: Unexpected error processing type Pin"
                        );
                    }
                }
            } else if last_seg.ident == "Box" {
                if is_reference {
                    abort!(last_seg.ident.span(), "Savefile does not support reference to Box. This is also generally not very useful, just use a regular reference for arguments.");
                }
                return parse_box_type(
                    version,
                    &path.path,
                    method_name,
                    is_return_value,
                    arg_name,
                    typ,
                    name_generator,
                    extra_definitions,
                    false,
                    is_mut_ref,
                    is_boxed,
                );
            } else if last_seg.ident == "Result" && is_return_value {
                if path.path.segments.len() != 1 {
                    abort!(path.path.segments.span(), "Savefile does not support types named 'Result', unless they are the standard type Result, and it must be specified as 'Result', without any namespace");
                }
                if is_reference {
                    abort!(
                        last_seg.ident.span(),
                        "Savefile does not presently support reference to Result. Consider removing the '&'."
                    );
                }

                match &last_seg.arguments {
                    PathArguments::Parenthesized(_) | PathArguments::None => {
                        abort!(last_seg.arguments.span(), "Savefile does not support types named 'Result', unless they are the standard type Result, and it must be specified as 'Result<A,B>', with two type parameters within angle-brackets. Found not type arguments.");
                    }
                    PathArguments::AngleBracketed(params) => {
                        let argvec: Vec<_> = params.args.iter().collect();
                        if argvec.len() != 2 {
                            abort!(last_seg.arguments.span(), "Savefile does not support types named 'Result', unless they are the standard type Result, and it must be specified as 'Result<A,B>', with two type parameters within angle-brackets. Got {} type arguments", argvec.len());
                        }
                        let mut argtypes = vec![];
                        for arg in argvec {
                            match arg {
                                GenericArgument::Type(argtyp) => {
                                    argtypes.push(Box::new(parse_type(
                                        version,
                                        arg_name,
                                        argtyp,
                                        method_name,
                                        is_return_value,
                                        &mut *name_generator,
                                        extra_definitions,
                                        is_reference,
                                        is_mut_ref,
                                        is_boxed,
                                    )));
                                }
                                GenericArgument::Lifetime(_) => {
                                    abort!(arg.span(), "Savefile does not support lifetime specifications.");
                                }
                                GenericArgument::Const(_) => {
                                    abort!(arg.span(), "Savefile does not support const in this location.");
                                }
                                GenericArgument::Binding(_) => {
                                    abort!(arg.span(), "Savefile does not support the syntax expressed here.");
                                }
                                GenericArgument::Constraint(_) => {
                                    abort!(arg.span(), "Savefile does not support constraints at this position.");
                                }
                            }
                        }
                        let mut i = argtypes.into_iter();
                        let oktype = i.next().unwrap();

                        let errtype = i.next().unwrap();
                        return ArgType::Result(oktype, errtype);
                    }
                }
            } else {
                rawtype = typ;
            }
        }
        Type::ImplTrait(i) => {
            for bound in &i.bounds {
                match bound {
                    TypeParamBound::Trait(t) => {
                        if let Some(last) = t.path.segments.last().map(|x| &x.ident) {
                            if last == "Future" {
                                abort!(
                                    last.span(),
                                    "{}, impl Future is not supported by savefile-abi. You can try using Pin<Box<{}>> instead.",
                                    location, i.bounds.to_token_stream()
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            abort!(
                i.span(),
                "{}, impl trait is not supported by savefile-abi. Try using a box: Box<{}>.",
                location,
                i.bounds.to_token_stream()
            );
        }
        _ => {
            abort!(
                typ.span(),
                "{}, type is unsupported by savefile-abi: {}",
                location,
                typ.to_token_stream()
            );
        }
    }
    if is_mut_ref {
        abort!(
            typ.span(),
            "{}: Mutable references are not supported by savefile-abi (except for trait objects)",
            location,
        );
    }
    ArgType::PlainData(rawtype.clone())
}

pub fn is_well_known<'a, I>(path: I, items: [&str; 3]) -> bool
where
    I: IntoIterator<Item = &'a PathSegment>,
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    for (a, b) in items.iter().rev().zip(path.into_iter().rev()) {
        if b.ident == a {
            continue;
        }
        if b.ident == "core" && *a == "std" {
            continue;
        }
        return false;
    }
    true
}
pub fn safe_is_well_known(path: &Path, items: [&str; 3]) -> bool {
    if !path.leading_colon.is_some() {
        return false;
    }
    if path.segments.len() != 3 {
        return false;
    }

    for (a, b) in items.iter().zip(path.segments.iter()) {
        if b.ident == a {
            continue;
        }
        if b.ident == "core" && *a == "std" {
            continue;
        }
        return false;
    }
    true
}

struct TypeInstruction {
    //callee_trampoline_real_method_invocation_argument1: TokenStream,
    callee_trampoline_temp_variable_declaration1: TokenStream,
    callee_trampoline_variable_deserializer1: TokenStream,
    caller_arg_serializer_temp1: TokenStream,
    caller_arg_serializer1: TokenStream,
    /// The declaration of the function arg at the caller-site, for the caller trampoline.
    /// I.e: 'x: u32'
    //caller_fn_arg1: TokenStream,
    schema: TokenStream,
    arg_type1: TokenStream,

    known_size_align1: Option<(usize, usize)>,
    /// The size and alignment of a pointer to this type
    known_size_align_of_pointer1: Option<(usize, usize)>,
    /// The type that this parameter is primarily deserialized into, on the
    /// deserialized side (i.e, callee for arguments, caller for return value);
    deserialized_type: TokenStream,
}

#[allow(unused)]
fn mutsymbol(ismut: bool) -> TokenStream {
    if ismut {
        quote!(mut)
    } else {
        quote! {}
    }
}

impl ArgType {
    fn get_lifetime(&self) -> Option<Lifetime> {
        match self {
            ArgType::Reference(_, _, lt) => lt.clone(),
            _ => None,
        }
    }
    fn get_instruction(
        &self,
        version: u32,
        arg_index: Option<usize>,
        arg_orig_name: &str,
        arg_name: &TokenStream, //always just 'arg_orig_name'
        nesting_level: u32,
        take_ownership: bool,
        extra_definitions: &mut HashMap<WrapperKey, (ClosureFutureWrapperNames, TokenStream)>,
        prefixed_arg_name: &Ident,
    ) -> TypeInstruction {
        let temp_arg_name = Ident::new(&format!("temp_{}_{}", arg_orig_name, nesting_level), Span::call_site());

        let layout_compatible = if let Some(arg_index) = arg_index {
            quote!(compatibility_mask&(1<<#arg_index) != 0)
        } else {
            quote!(false)
        };
        match self {
            ArgType::Reference(arg_type, is_mut, _lifetime) => {
                //let mutsym = mutsymbol(*is_mut);
                let TypeInstruction {
                    callee_trampoline_temp_variable_declaration1,
                    callee_trampoline_variable_deserializer1,
                    caller_arg_serializer_temp1,
                    caller_arg_serializer1,
                    schema,
                    arg_type1,
                    known_size_align1: _,
                    known_size_align_of_pointer1,
                    deserialized_type,
                } = arg_type.get_instruction(
                    version,
                    arg_index,
                    arg_orig_name,
                    arg_name,
                    nesting_level + 1,
                    false,
                    extra_definitions,
                    prefixed_arg_name,
                );

                let known_size_align1 = match &**arg_type {
                    ArgType::PlainData(plain) => {
                        if compile_time_abi_check_size(plain).is_some() {
                            known_size_align_of_pointer1
                        } else {
                            None
                        }
                    }
                    ArgType::Boxed(inner) => match &**inner {
                        ArgType::Fn(..) | ArgType::Trait(..) | ArgType::Future(..) => Some(MEGA_FAT_POINTER),
                        _ => None,
                    },
                    ArgType::Fn(..) | ArgType::Trait(..) | ArgType::Future(..) => Some(MEGA_FAT_POINTER),
                    ArgType::Str(_) => None,
                    ArgType::Reference(..) => None,
                    ArgType::Slice(_) => None,
                    ArgType::Result(_, _) => None,
                };

                let (mutsymbol, read_raw_ptr) = if *is_mut {
                    (quote!(mut), quote!(read_raw_ptr_mut))
                } else {
                    (quote!(), quote!(read_raw_ptr))
                };

                TypeInstruction {
                    callee_trampoline_temp_variable_declaration1: quote! {
                        #callee_trampoline_temp_variable_declaration1
                        let #mutsymbol #temp_arg_name;
                    },
                    deserialized_type,
                    arg_type1: quote!(&arg_type1),
                    callee_trampoline_variable_deserializer1: quote! {
                        if #layout_compatible {
                            unsafe { &#mutsymbol *(deserializer. #read_raw_ptr ::<#arg_type1>()?) }
                        } else {
                            #temp_arg_name = #callee_trampoline_variable_deserializer1;
                            & #mutsymbol #temp_arg_name
                        }
                    },
                    caller_arg_serializer_temp1,
                    caller_arg_serializer1: quote! {
                        if #layout_compatible {
                            unsafe { serializer.write_raw_ptr(#prefixed_arg_name as *const #arg_type1).expect("Writing argument ref") };
                            Ok(())
                        } else {
                            #caller_arg_serializer1
                        }
                    },
                    schema: quote! {Schema::Reference(std::boxed::Box::new(#schema))},
                    known_size_align1,
                    known_size_align_of_pointer1: None, //Pointer to pointer not even supported
                }
            }
            ArgType::Str(_) => {
                TypeInstruction {
                    //callee_trampoline_real_method_invocation_argument1: quote! {&#prefixed_arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {},
                    deserialized_type: quote! {String},
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            let ptr = deserializer.read_ptr()? as *const u8;
                            let len = deserializer.read_usize()?;
                            std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len)})?
                        }
                    },
                    caller_arg_serializer_temp1: quote!(),
                    caller_arg_serializer1: quote! {
                        {
                            unsafe {
                                serializer.write_ptr(#prefixed_arg_name.as_ptr() as *const ()).expect("Failed while serializing");
                                serializer.write_usize(#prefixed_arg_name.len())
                            }
                        }
                    },
                    //caller_fn_arg1: quote! {#prefixed_arg_name : &str},
                    schema: quote!(Schema::Str),

                    arg_type1: quote!(str),
                    known_size_align1: None,
                    known_size_align_of_pointer1: None,
                }
            }
            ArgType::Slice(arg_type) => {
                let TypeInstruction {
                    callee_trampoline_temp_variable_declaration1,
                    callee_trampoline_variable_deserializer1: _,
                    caller_arg_serializer_temp1,
                    caller_arg_serializer1: _,
                    schema,
                    arg_type1,
                    known_size_align1,
                    known_size_align_of_pointer1: _,
                    deserialized_type: _,
                } = arg_type.get_instruction(
                    version,
                    arg_index,
                    arg_orig_name,
                    arg_name,
                    nesting_level + 1,
                    false,
                    extra_definitions,
                    prefixed_arg_name,
                );

                TypeInstruction {
                    //callee_trampoline_real_method_invocation_argument1: quote! {&#prefixed_arg_name},
                    callee_trampoline_temp_variable_declaration1: quote! {
                        #callee_trampoline_temp_variable_declaration1
                        let #temp_arg_name;
                    },
                    deserialized_type: quote! {Vec<_>},
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            #temp_arg_name = deserialize_slice_as_vec::<_,#arg_type1>(&mut deserializer)?;
                            #temp_arg_name
                        }
                    },
                    caller_arg_serializer_temp1,
                    caller_arg_serializer1: quote! {
                        (#prefixed_arg_name).serialize(&mut serializer)
                    }, //we only support slices containing savefile-serializable stuff, so we don't forward to the item type here

                    schema: quote!( Schema::Slice(std::boxed::Box::new(#schema)) ),
                    arg_type1: quote!( [#arg_type1] ),
                    known_size_align1: if known_size_align1.is_some() {
                        Some(FAT_POINTER)
                    } else {
                        None
                    },
                    known_size_align_of_pointer1: None,
                }
            }
            ArgType::PlainData(arg_type) => TypeInstruction {
                deserialized_type: quote! {#arg_type},
                callee_trampoline_temp_variable_declaration1: quote!(),
                callee_trampoline_variable_deserializer1: quote! {
                    <#arg_type as Deserialize>::deserialize(&mut deserializer)?
                },
                caller_arg_serializer_temp1: quote!(),
                caller_arg_serializer1: quote! {
                    #prefixed_arg_name.serialize(&mut serializer)
                },
                schema: quote!( get_schema::<#arg_type>(version) ),
                known_size_align1: compile_time_abi_check_size(arg_type),
                arg_type1: arg_type.to_token_stream(),
                known_size_align_of_pointer1: None,
            },
            ArgType::Boxed(inner_arg_type) => {
                let TypeInstruction {
                    callee_trampoline_temp_variable_declaration1,
                    callee_trampoline_variable_deserializer1,
                    caller_arg_serializer_temp1,
                    caller_arg_serializer1,
                    schema,
                    arg_type1,
                    known_size_align1: _,
                    known_size_align_of_pointer1: _,
                    deserialized_type,
                } = inner_arg_type.get_instruction(
                    version,
                    arg_index,
                    arg_orig_name,
                    &quote!( #arg_name ),
                    nesting_level + 1,
                    true,
                    extra_definitions,
                    prefixed_arg_name,
                );

                let mut known_size_align1 = None;

                match &**inner_arg_type {
                    ArgType::Fn(_, _, _, _, _, _) => {
                        known_size_align1 = Some((FAT_POINTER_SIZE + POINTER_SIZE, FAT_POINTER_ALIGNMENT));
                    }
                    ArgType::Trait(..) => {
                        known_size_align1 = Some((FAT_POINTER_SIZE + POINTER_SIZE, FAT_POINTER_ALIGNMENT));
                    }
                    _ => {}
                }

                TypeInstruction {
                    //deserialized_type: quote!{Box<AbiConnection<dyn #trait_name>>},
                    deserialized_type: quote! {Box<#deserialized_type>},
                    callee_trampoline_temp_variable_declaration1: quote! {
                        #callee_trampoline_temp_variable_declaration1
                    },
                    callee_trampoline_variable_deserializer1: quote! {
                        std::boxed::Box::new( #callee_trampoline_variable_deserializer1 )
                    },
                    caller_arg_serializer_temp1,
                    caller_arg_serializer1,
                    schema: quote!( Schema::Boxed( std::boxed::Box::new(#schema) ) ),
                    arg_type1: quote!( Box<#arg_type1> ),
                    known_size_align1,
                    known_size_align_of_pointer1: None,
                }
            }
            ArgType::Trait(trait_name, ismut) => {
                let trait_type = trait_name;

                let newsymbol = quote! {new_from_ptr};

                let owning = if take_ownership {
                    quote!(Owning::Owned)
                } else {
                    quote!(Owning::NotOwned)
                };

                TypeInstruction {
                    deserialized_type: quote! { AbiConnection<dyn #trait_type> },
                    callee_trampoline_temp_variable_declaration1: quote! {},
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            unsafe { AbiConnection::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, #owning)? }
                        }
                    },
                    caller_arg_serializer_temp1: quote!(),
                    caller_arg_serializer1: quote! {
                        {
                            PackagedTraitObject::#newsymbol::<dyn #trait_type>( unsafe { std::mem::transmute(#prefixed_arg_name) } ).serialize(&mut serializer)
                        }
                    },
                    schema: quote!( Schema::Trait(#ismut, <dyn #trait_name as AbiExportable>::get_definition(version)) ),
                    arg_type1: quote!( dyn #trait_name ),
                    known_size_align1: Some((FAT_POINTER_SIZE + POINTER_SIZE, FAT_POINTER_ALIGNMENT)),
                    known_size_align_of_pointer1: None,
                }
            }
            ArgType::Fn(fndef, args, ret_type, ismut, sync, send) => {
                let temp_arg_name2 = Ident::new(&format!("temp2_{}", arg_orig_name), Span::call_site());
                let temp_arg_ser_name = Ident::new(&format!("temp_ser_{}", arg_orig_name), Span::call_site());

                let wrapper_names = emit_closure_helpers(
                    version,
                    args,
                    ret_type.clone(),
                    *ismut,
                    extra_definitions,
                    &Ident::new(if *ismut { "FnMut" } else { "Fn" }, Span::call_site()),
                    take_ownership,
                    *sync,
                    *send,
                );

                let temp_trait_name_wrapper = wrapper_names.wrapper_struct_name;
                let temp_trait_type = wrapper_names.trait_name;
                //let temp_trait_name_wrapper = Ident::new(&format!("{}_wrapper", temp_trait_type), Span::call_site());

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
                let owning = if take_ownership {
                    quote!(Owning::Owned)
                } else {
                    quote!(Owning::NotOwned)
                };

                let arg_access = if take_ownership {
                    quote! {
                        #prefixed_arg_name
                    }
                } else {
                    quote! {
                        #prefixed_arg_name as *#mutorconst _
                    }
                };
                let arg_make_ptr = if take_ownership {
                    quote! {
                        std::boxed::Box::into_raw(Box::new(#temp_arg_ser_name))
                    }
                } else {
                    quote! {
                        &#mutsymbol #temp_arg_ser_name as *#mutorconst _
                    }
                };

                TypeInstruction {
                    deserialized_type: quote! {AbiConnection::<dyn #temp_trait_type>},
                    callee_trampoline_temp_variable_declaration1: quote! {
                        let #mutsymbol #temp_arg_name;
                        let #mutsymbol #temp_arg_name2;
                    },
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            #temp_arg_name = unsafe { AbiConnection::<dyn #temp_trait_type>::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, #owning)? };
                            #temp_arg_name2 = move|#(#typedarglist,)*| {#temp_arg_name.docall(#(#arglist,)*)};
                            #temp_arg_name2
                        }
                    },
                    caller_arg_serializer_temp1: quote! {
                        let #mutsymbol #temp_arg_ser_name;
                    },

                    //let #mutsymbol temp : *#mutorconst (dyn #temp_trait_type+'_) = &#mutsymbol #temp_arg_ser_name as *#mutorconst _;
                    caller_arg_serializer1: quote! {
                        {
                            #temp_arg_ser_name = #temp_trait_name_wrapper { func: #arg_access };
                            let #mutsymbol temp : *#mutorconst (dyn #temp_trait_type+'_) = #arg_make_ptr;
                            PackagedTraitObject::#newsymbol::<(dyn #temp_trait_type+'_)>( unsafe { std::mem::transmute(temp)} ).serialize(&mut serializer)
                        }
                    },
                    arg_type1: quote! {dyn #fndef },
                    schema: quote!( Schema::FnClosure(#ismut, <dyn #temp_trait_type as AbiExportable >::get_definition(version)) ),
                    //arg_type1: Default::default(),
                    known_size_align1: Some((FAT_POINTER_SIZE + POINTER_SIZE, FAT_POINTER_ALIGNMENT)),
                    known_size_align_of_pointer1: None,
                }
            }
            ArgType::Future(pin, output, send, sync, unpin) => {
                let wrapper_names =
                    emit_future_helpers(*pin, output.clone(), &mut *extra_definitions, *send, *sync, *unpin);

                let futurer_wrapper_struct_name = wrapper_names.wrapper_struct_name;
                let temp_trait_type = wrapper_names.trait_name;
                let wrap_in_pin = if *pin {
                    quote!(
                        let t = Pin::new(t);
                    )
                } else {
                    quote! {}
                };
                TypeInstruction {
                    deserialized_type: quote! { AbiConnection::<dyn #temp_trait_type> },
                    callee_trampoline_temp_variable_declaration1: quote! {
                        let mut #temp_arg_name;
                    },
                    callee_trampoline_variable_deserializer1: quote! {
                        {
                            let t = Box::new(unsafe {
                                #futurer_wrapper_struct_name {
                                    future: AbiConnection::<dyn #temp_trait_type>::from_raw_packaged(PackagedTraitObject::deserialize(&mut deserializer)?, Owning::Owned)?
                                }
                            });
                            #wrap_in_pin

                            #temp_arg_name = t;

                            #temp_arg_name
                        }
                    },
                    caller_arg_serializer_temp1: quote! {},
                    caller_arg_serializer1: quote! {
                        {
                            let temp : Box<dyn #temp_trait_type + '_> = Box::new(#prefixed_arg_name );
                            let temp = std::boxed::Box::into_raw(temp);
                            PackagedTraitObject::new_from_ptr::<(dyn #temp_trait_type+'_)>( unsafe { std::mem::transmute(temp)} ).serialize(&mut serializer)
                        }
                    },
                    schema: quote!(
                         Schema::Future( <dyn #temp_trait_type as AbiExportable >::get_definition(version), #send,#sync,#unpin)
                    ),
                    arg_type1: quote!(
                        Box<dyn Future<Output=#output> + Unpin>
                    ),
                    known_size_align1: Some((FAT_POINTER_SIZE + POINTER_SIZE, FAT_POINTER_ALIGNMENT)),
                    known_size_align_of_pointer1: None,
                }
            }
            ArgType::Result(ok_type, err_type) => {
                let ok_instruction = ok_type.get_instruction(
                    version,
                    arg_index,
                    "okval",
                    &quote!(okval),
                    nesting_level + 1,
                    true,
                    extra_definitions,
                    &Ident::new("okval", Span::call_site()),
                );
                let err_instruction = err_type.get_instruction(
                    version,
                    arg_index,
                    "errval",
                    &quote!(errval),
                    nesting_level + 1,
                    true,
                    extra_definitions,
                    &Ident::new("errval", Span::call_site()),
                );

                let TypeInstruction {
                    callee_trampoline_variable_deserializer1: ok_callee_trampoline_variable_deserializer1,
                    caller_arg_serializer1: ok_caller_arg_serializer1,
                    deserialized_type: ok_deserialized_type,
                    callee_trampoline_temp_variable_declaration1: ok_callee_trampoline_temp_variable_declaration1,
                    caller_arg_serializer_temp1: ok_caller_arg_serializer_temp1,
                    schema: ok_schema,
                    ..
                } = ok_instruction;

                let TypeInstruction {
                    callee_trampoline_variable_deserializer1: err_callee_trampoline_variable_deserializer1,
                    caller_arg_serializer1: err_caller_arg_serializer1,
                    deserialized_type: err_deserialized_type,
                    callee_trampoline_temp_variable_declaration1: err_callee_trampoline_temp_variable_declaration1,
                    caller_arg_serializer_temp1: err_caller_arg_serializer_temp1,
                    schema: err_schema,
                    ..
                } = err_instruction;

                TypeInstruction {
                    deserialized_type: quote! { Result<#ok_deserialized_type, #err_deserialized_type> },
                    callee_trampoline_temp_variable_declaration1: quote! {
                        #ok_callee_trampoline_temp_variable_declaration1;
                        #err_callee_trampoline_temp_variable_declaration1;
                    },
                    callee_trampoline_variable_deserializer1: quote! {
                            if deserializer.read_bool()? {
                                Ok(#ok_callee_trampoline_variable_deserializer1)
                            } else {
                                Err(#err_callee_trampoline_variable_deserializer1)
                            }
                    },
                    caller_arg_serializer_temp1: quote! {
                        #ok_caller_arg_serializer_temp1;
                        #err_caller_arg_serializer_temp1;
                    },
                    caller_arg_serializer1: quote! {
                        match #prefixed_arg_name {
                            Ok(okval) => {
                                serializer.write_bool(true)?;
                                #ok_caller_arg_serializer1
                            },
                            Err(errval) => {
                                serializer.write_bool(false)?;
                                #err_caller_arg_serializer1
                            }
                        }
                    },
                    schema: quote!(
                        get_result_schema(#ok_schema, #err_schema)
                    ),
                    arg_type1: quote!(
                        Result<#ok_deserialized_type, #err_deserialized_type>
                    ),
                    known_size_align1: None,
                    known_size_align_of_pointer1: None,
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
    receiver_is_pin: bool, //TODO: Way too many bool parameters! (And too many parameters)
    args: Vec<(Ident, &Type)>,
    name_generator: &mut impl FnMut() -> String,
    extra_definitions: &mut HashMap<WrapperKey, (ClosureFutureWrapperNames, TokenStream)>,
    async_trait_detected: bool,
) -> MethodDefinitionComponents {
    let method_name_str = method_name.to_string();

    let mut callee_trampoline_real_method_invocation_arguments: Vec<TokenStream> = vec![];
    let mut callee_trampoline_variable_declaration = vec![];
    let mut callee_trampoline_temp_variable_declaration = vec![];
    let mut callee_trampoline_variable_deserializer = vec![];
    let mut caller_arg_serializers = vec![];
    let mut caller_fn_arg_list = vec![];
    let mut metadata_arguments = vec![];
    let mut caller_arg_serializers_temp = vec![];

    let mut compile_time_known_size = Some(0);
    let mut arg_lifetimes = vec![];
    let mut first_ref_arg = None;
    for (arg_index, (arg_name, typ)) in args.iter().enumerate() {
        let prefixed_arg_name = Ident::new(&format!("arg_{}", arg_name), Span::call_site());
        let argtype = parse_type(
            version,
            &arg_name.to_string(),
            typ,
            &method_name,
            false,
            &mut *name_generator,
            extra_definitions,
            false,
            false,
            false,
        );
        if let ArgType::Reference(..) = &argtype {
            first_ref_arg = Some(typ.span());
        }
        callee_trampoline_variable_declaration.push(quote! {let #prefixed_arg_name;});

        let instruction = argtype.get_instruction(
            version,
            Some(arg_index),
            &arg_name.to_string(),
            &arg_name.to_token_stream(),
            0,
            true,
            extra_definitions,
            &prefixed_arg_name,
        );

        if let Some(lifetime) = argtype.get_lifetime() {
            arg_lifetimes.push(lifetime);
        }

        caller_arg_serializers_temp.push(instruction.caller_arg_serializer_temp1);
        callee_trampoline_real_method_invocation_arguments.push(
            quote! {#prefixed_arg_name}, //instruction.callee_trampoline_real_method_invocation_argument1
        );
        callee_trampoline_temp_variable_declaration.push(instruction.callee_trampoline_temp_variable_declaration1);

        let deserializer_expression = instruction.callee_trampoline_variable_deserializer1;

        callee_trampoline_variable_deserializer.push(quote!( #prefixed_arg_name = #deserializer_expression ; ));
        let arg_serializer = instruction.caller_arg_serializer1;
        caller_arg_serializers.push(quote! {
            #arg_serializer.expect("Failed while serializing");
        });
        caller_fn_arg_list.push(quote!( #prefixed_arg_name: #typ )); //instruction.caller_fn_arg1);
        let schema = instruction.schema;
        //let can_be_sent_as_ref = instruction.can_be_sent_as_ref;
        metadata_arguments.push(quote! {
                        AbiMethodArgument {
                            schema: { let mut context = WithSchemaContext::new(); let context = &mut context; #schema },
                        }
        });
        if let Some(total_size) = &mut compile_time_known_size {
            if let Some((known_size, _known_align)) = instruction.known_size_align1 {
                *total_size += known_size;
            } else {
                compile_time_known_size = None;
            }
        }
    }

    let callee_real_method_invocation_except_args;
    if receiver_is_mut {
        callee_real_method_invocation_except_args = if receiver_is_pin {
            quote! { unsafe { Pin::new_unchecked( &mut *trait_object.as_mut_ptr::<dyn #trait_name>() ) }.#method_name }
        } else {
            quote! { unsafe { &mut *trait_object.as_mut_ptr::<dyn #trait_name>() }.#method_name }
        };
    } else {
        callee_real_method_invocation_except_args =
            quote! { unsafe { &*trait_object.as_const_ptr::<dyn #trait_name>() }.#method_name };
    }
    let async_trait_self_lifetime = async_trait_detected.then(|| quote!( 'life0 ));

    let receiver_type;
    //let receiver_mut_str = receiver_mut.to_string();
    let receiver = if receiver_is_mut {
        if receiver_is_pin {
            receiver_type = quote!(ReceiverType::PinMut);
            quote!(self: Pin<&mut Self>)
        } else {
            receiver_type = quote!(ReceiverType::Mut);
            quote!(& #async_trait_self_lifetime mut self)
        }
    } else {
        receiver_type = quote!(ReceiverType::Shared);
        assert!(!receiver_is_pin);
        quote! {& #async_trait_self_lifetime self}
    };
    let return_value_schema;

    let caller_return_type;
    let ret_deserializer;
    let ret_temp_decl;
    let ret_serialize;

    let result_default;
    let return_ser_temp;
    let ret_type_size;
    if no_return {
        return_value_schema = quote!(get_schema::<()>(0));
        ret_deserializer = quote!(()); //Zero-sized, no deserialize actually needed
        ret_serialize = quote!(());
        caller_return_type = quote!(());
        ret_temp_decl = quote!();
        return_ser_temp = quote!();
        result_default = quote!( MaybeUninit::<Result<#ret_type,SavefileError>>::new(Ok(())) );
        ret_type_size = Some((0, 1));
    //Safe, does not need drop and does not allocate
    } else {
        let parsed_ret_type = parse_type(
            version,
            "___retval",
            &ret_type,
            &method_name,
            true,
            name_generator,
            extra_definitions,
            false,
            false,
            false,
        );
        if let ArgType::Reference(..) = &parsed_ret_type {
            abort!(
                ret_type.span(),
                "Method '{}': savefile-abi does not support methods returning references.",
                method_name
            );
        }
        if let ArgType::Future(..) = &parsed_ret_type {
            if let Some(r) = first_ref_arg {
                abort!(
                    r,
                    "Method '{}': savefile-abi does not support reference arguments to async functions or functions returning futures.",
                    method_name
                );
            }
        }
        if let ArgType::Str(false) = &parsed_ret_type {
            abort!(
                ret_type.span(),
                "Method '{}': savefile-abi does not support methods returning &str. Use \"String\" or \"&'static str\" instead",
                method_name
            );
        }
        let instruction = parsed_ret_type.get_instruction(
            version,
            None,
            "ret",
            &Ident::new("ret", Span::call_site()).to_token_stream(),
            0,
            true,
            extra_definitions,
            &Ident::new("ret", Span::call_site()),
        );
        caller_return_type = instruction.deserialized_type;
        return_value_schema = instruction.schema;
        return_ser_temp = instruction.caller_arg_serializer_temp1;
        ret_deserializer = instruction.callee_trampoline_variable_deserializer1;
        let ret_serializer = instruction.caller_arg_serializer1;
        ret_temp_decl = instruction.callee_trampoline_temp_variable_declaration1;
        ret_type_size = instruction.known_size_align1;
        ret_serialize = quote!( #ret_serializer );

        result_default = quote!( MaybeUninit::<Result<#ret_type,SavefileError>>::uninit() );
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
        arg_buffer = quote! {
            let mut __savefile_internal_datarawdata = [0u8;#compile_time_known_size];
            let mut __savefile_internal_data = Cursor::new(&mut __savefile_internal_datarawdata[..]);
        };
        data_as_ptr = quote!(__savefile_internal_datarawdata[..].as_ptr());
        data_length = quote!( #compile_time_known_size );
    } else {
        arg_buffer = quote!( let mut __savefile_internal_data = FlexBuffer::new(); );
        data_as_ptr = quote!(__savefile_internal_data.as_ptr() as *const u8);
        data_length = quote!(__savefile_internal_data.len());
    }

    let _ = caller_return_type;

    let mut all_lifetimes: Vec<Lifetime> = vec![Lifetime::new("'life0", Span::call_site())];
    for lt in arg_lifetimes.iter().cloned() {
        all_lifetimes.push(lt);
    }
    all_lifetimes.push(Lifetime::new("'async_trait", Span::call_site()));

    let async_trait_lifetime_decls = async_trait_detected.then(|| quote!( < #( #all_lifetimes ),* > ));

    let arg_lifetime_bounds = (async_trait_detected && arg_lifetimes.is_empty() == false).then(|| {
        quote! {
            #( #arg_lifetimes: 'async_trait ),* ,
        }
    });

    let async_trait_where = async_trait_detected.then(|| {
        quote!(
            where
            'life0 : 'async_trait,
            #arg_lifetime_bounds
            Self : 'async_trait
        )
    });

    let caller_method_trampoline = quote! {
        // TODO: Determine if we should use inline here or not? #[inline]
        #[inline]
        fn #method_name #async_trait_lifetime_decls(#receiver, #(#caller_fn_arg_list,)*) #ret_declaration
            #async_trait_where
        {
            let info: &AbiConnectionMethod = &self.template.methods[#method_number as usize];

            let Some(callee_method_number) = info.callee_method_number else {
                panic!("Method '{}' does not exist in implementation.", info.method_name);
            };

            let mut result_buffer = #result_default;
            let compatibility_mask = info.compatibility_mask;

            #arg_buffer

            #(#caller_arg_serializers_temp)*

            let mut serializer = Serializer {
                writer: &mut __savefile_internal_data,
                file_version: self.template.effective_version,
            };
            serializer.write_u32(self.template.effective_version).unwrap();
            #(#caller_arg_serializers)*

            unsafe {

                unsafe extern "C" fn abi_result_receiver<'async_trait>(
                    outcome: *const RawAbiCallResult,
                    result_receiver: *mut (),
                ) {
                    let outcome = unsafe { &*outcome };
                    let result_receiver = unsafe { &mut *(result_receiver as *mut std::mem::MaybeUninit<Result<#ret_type, SavefileError>>) };
                    result_receiver.write(
                        parse_return_value_impl(outcome, |mut deserializer| -> Result<#ret_type, SavefileError> {

                            #ret_temp_decl
                            Ok(#ret_deserializer)
                            //T::deserialize(deserializer)
                        })
                    );
                }

            (self.template.entry)(AbiProtocol::RegularCall {
                trait_object: self.trait_object,
                compatibility_mask,
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
                return_value: { let mut context = WithSchemaContext::new(); let context = &mut context; #return_value_schema},
                receiver: #receiver_type,
                arguments: vec![ #(#metadata_arguments,)* ],
                async_trait_heuristic: #async_trait_detected
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
        let known_size = ret_type_size; //compile_time_abi_check_size(&ret_type);
        if let Some((compile_time_known_size, _align)) = known_size {
            // If we have simple type such as u8, u16 etc, we can sometimes
            // know at compile-time what the size of the args will be.
            // If the rust-compiler offered 'introspection', we could do this
            // for many more types. But we can at least do it for the most simple.

            let compile_time_known_size = compile_time_known_size + 4; //Space for 'version'
            ret_buffer = quote! {
                let mut __savefile_internal_datarawdata = [0u8;#compile_time_known_size];
                let mut __savefile_internal_data = Cursor::new(&mut __savefile_internal_datarawdata[..]);
            };
            data_as_ptr = quote!(__savefile_internal_datarawdata[..].as_ptr());
            data_length = quote!( #compile_time_known_size );
        } else {
            ret_buffer = quote!( let mut __savefile_internal_data = FlexBuffer::new(); );
            data_as_ptr = quote!(__savefile_internal_data.as_ptr() as *const u8);
            data_length = quote!(__savefile_internal_data.len());
        }

        handle_retval = quote! {
            #ret_buffer
            let mut serializer = Serializer {
                writer: &mut __savefile_internal_data,
                file_version: #version,
            };

            #return_ser_temp

            serializer.write_u32(effective_version)?;
            match #ret_serialize
            {
                Ok(()) => {
                    let outcome = RawAbiCallResult::Success {data: #data_as_ptr, len: #data_length};
                    unsafe { __savefile_internal_receiver(&outcome as *const _, abi_result) }
                }
                Err(err) => {
                    let err_str = format!("{:?}", err);
                    let outcome = RawAbiCallResult::AbiError(AbiErrorMsg{error_msg_utf8: err_str.as_ptr(), len: err_str.len()});
                    unsafe { __savefile_internal_receiver(&outcome as *const _, abi_result) }
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
