use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Error, Expr, ExprLit, GenericParam, Generics, Lit, LitStr, Meta, MetaNameValue, RangeLimits, Type, WhereClause};
use syn::token::Token;

pub(crate) fn get_extra_where_clauses(
    gen2: &Generics,
    where_clause: Option<&WhereClause>,
    the_trait: TokenStream,
) -> TokenStream {
    let extra_where_separator;
    if let Some(where_clause) = where_clause {
        if where_clause.predicates.trailing_punct() {
            extra_where_separator = quote!();
        } else {
            extra_where_separator = quote!(,);
        }
    } else {
        extra_where_separator = quote!(where);
    }
    let mut where_clauses = vec![];
    for param in gen2.params.iter() {
        if let GenericParam::Type(t) = param {
            let t_name = &t.ident;
            let clause = quote! {#t_name : #the_trait};
            where_clauses.push(clause);
        }
    }
    let extra_where = quote! {
        #extra_where_separator #(#where_clauses),*
    };
    extra_where
}

#[derive(Debug)]
pub(crate) struct VersionRange {
    pub(crate) from: u32,
    pub(crate) to: u32,
    pub(crate) convert_fun: String,
    pub(crate) serialized_type: String,
}

#[derive(Debug)]
pub(crate) struct AttrsResult {
    pub(crate) version_from: u32, //0 means no lower bound
    pub(crate) version_to: u32,   //u32::MAX means no upper bound
    pub(crate) ignore: bool,
    pub(crate) default_fn: Option<TokenStream>,
    pub(crate) default_val: Option<TokenStream>,
    pub(crate) deserialize_types: Vec<VersionRange>,
    pub(crate) introspect_key: bool,
    pub(crate) introspect_ignore: bool,
}

impl AttrsResult {
    pub(crate) fn min_safe_version(&self) -> u32 {
        let mut min_safe_version = 0;
        if self.version_to < std::u32::MAX {
            // A delete
            min_safe_version = min_safe_version.max(self.version_to.saturating_add(1));
        }
        // An addition
        min_safe_version.max(self.version_from)
    }
}
pub(crate) enum RemovedType {
    NotRemoved,
    Removed,
    AbiRemoved,
}
impl RemovedType {
    pub(crate) fn is_removed(&self) -> bool {
        match self {
            RemovedType::NotRemoved => false,
            RemovedType::Removed => true,
            RemovedType::AbiRemoved => true,
        }
    }
}
pub(crate) fn check_is_remove(field_type: &syn::Type) -> RemovedType {
    let mut tokens = TokenStream::new();
    field_type.to_tokens(&mut tokens);
    for tok in tokens.into_iter() {
        if tok.to_string() == "Removed" {
            return RemovedType::Removed;
            //TODO: This is not robust, since it's based on text matching
        }
        if tok.to_string() == "AbiRemoved" {
            return RemovedType::AbiRemoved;
            //TODO: This is not robust, since it's based on text matching
        }
    }
    RemovedType::NotRemoved
}

pub(crate) fn overlap<'a>(b: &'a VersionRange) -> impl Fn(&'a VersionRange) -> bool {
    assert!(b.to >= b.from);
    move |a: &'a VersionRange| {
        assert!(a.to >= a.from);
        let no_overlap = a.to < b.from || a.from > b.to;
        !no_overlap
    }
}

pub(crate) fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .last()
        .expect("Expected at least one segment")
        .ident
        .to_string()
}

pub(crate) fn parse_attr_tag(attrs: &[syn::Attribute]) -> AttrsResult {
    let mut field_from_version = None;
    let mut field_to_version = None;
    let mut default_fn = None;
    let mut default_val = None;
    let mut ignore = false;
    let mut introspect_ignore = false;
    let mut introspect_key = false;
    let mut deser_types = Vec::new();
    for attr in attrs.iter() {
        {
            //Ok(ref meta) => match meta {


                //let x = path_to_string(x);
                if attr.path().is_ident("savefile_ignore") {
                    ignore = true;
                }
                if attr.path().is_ident("savefile_introspect_key") {
                    introspect_key = true;
                }
                if attr.path().is_ident("savefile_introspect_ignore") {
                    introspect_ignore = true;
                }

                if attr.path().is_ident("savefile_default_val") {
                    match &attr.meta {
                        Meta::NameValue(nv) => {
                            let val = &nv.value;
                            match val {
                                Expr::Lit(x) => {
                                    match &x.lit {
                                        Lit::Str(lit) => {
                                            let s = lit.value();
                                            default_val = Some(quote! { str::parse(#s).expect("Expected valid literal string") });
                                        }
                                        _ => {
                                            default_val = Some(quote! { #val });
                                        }
                                    }
                                }
                                _ => {
                                    default_val = Some(quote! { #val });
                                }
                            }
                        }
                        Meta::Path(_) |
                        Meta::List(_) => {
                            abort!(
                                attr.path().span(),
                                "Invalid default value"
                            )
                        }
                    }
                }

                if attr.path().is_ident("savefile_default_fn") {

                    let args: Result<syn::Ident, Error> = attr.parse_args();
                    match &attr.meta {
                        Meta::NameValue(nv) => {
                            match &nv.value {
                                Expr::Path(path) => {
                                    default_fn = Some(path.path.to_token_stream());
                                }
                                Expr::Lit(lit) =>  {
                                    match &lit.lit {
                                        Lit::Str(litstr) => {
                                            default_fn = Some(Ident::new(litstr.value().as_str(), lit.lit.span().clone()).to_token_stream());
                                        }
                                        _=> {
                                            abort!(
                                            attr.path().span(),
                                            "Invalid savefile_default_fn value"
                                        )
                                        }
                                    }
                                }
                                _ => {
                                    abort!(
                                attr.path().span(),
                                "Invalid savefile_default_fn value"
                            )
                                }
                            }
                        }
                        _ => {
                            abort!(
                                attr.path().span(),
                                "Invalid savefile_default_fn value"
                            )
                        }

                    }
                }
                if attr.path().is_ident("savefile_versions_as") {
                    let mut rhs = None;
                    match &attr.meta {
                        Meta::Path(_) |
                        Meta::List(_) => {}
                        Meta::NameValue(nv) => {
                            match &nv.value {
                                Expr::Lit(lit)=> {
                                    match &lit.lit {
                                        Lit::Str(litstr) => {
                                            rhs = Some(litstr.value());
                                        }
                                        _ =>{}
                                    }
                                } _ => {

                                }
                            }
                        }
                    }

                    if rhs.is_none() {
                        abort!(
                            attr.span(),
                            "Invalid savefile_versions_as-attribute value, should be string",
                        )
                    };
                    let litstr2 = rhs.unwrap();

                    let output2: Vec<String> =
                        litstr2.splitn(3, ':').map(|x| x.to_string()).collect();
                    if output2.len() != 3 && output2.len() != 2 {
                        abort!(litstr2.span(), "The #savefile_versions_as tag must contain a version range and a deserialization type, such as : #[savefile_versions_as=0..3:MyStructType]");
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
                        abort!(litstr2.span(), "savefile_versions_as tag must contain a (possibly half-open) range, such as 0..3 or 2.. (fields present in all versions to date should not use the savefile_versions_as-attribute)");
                    }
                    let (a, b) = (output[0].to_string(), output[1].to_string());

                    let from_ver = if a.trim() == "" {
                        0
                    } else if let Ok(a_u32) = a.parse::<u32>() {
                        a_u32
                    } else {
                        abort!(litstr2.span(), "The from version in the version tag must be an integer. Use #[savefile_versions_as=0..3:MyStructType] for example");
                    };

                    let to_ver = if b.trim() == "" {
                        std::u32::MAX
                    } else if let Ok(b_u32) = b.parse::<u32>() {
                        b_u32
                    } else {
                        abort!(litstr2.span(), "The to version in the version tag must be an integer. Use #[savefile_versions_as=0..3:MyStructType] for example");
                    };
                    if to_ver < from_ver {
                        abort!(litstr2.span(), "Version ranges must specify lower number first.");
                    }

                    let item = VersionRange {
                        from: from_ver,
                        to: to_ver,
                        convert_fun: convert_fun.to_string(),
                        serialized_type: version_type.to_string(),
                    };
                    if deser_types.iter().any(overlap(&item)) {
                        abort!(
                                        litstr2.span(),
                                        "#savefile_versions_as attributes may not specify overlapping ranges"
                                    );
                    }
                    deser_types.push(item);
                }

            if attr.path().is_ident("savefile_versions") {

                let mut rhs = None;
                match &attr.meta {
                    Meta::Path(_) |
                    Meta::List(_) => {
                    }
                    Meta::NameValue(nv) => {
                        rhs = Some(&nv.value);
                    }
                }

                if rhs.is_none() {
                    abort!(
                            attr.span(),
                            "Invalid savefile_versions_as-attribute value."
                        )
                };
                let rhs = rhs.unwrap();


                match rhs {
                    Expr::Range(val) => {
                        let mut range_start : Option<u32> = None;
                        let mut range_end : Option<u32> = None;
                        if let Some(start) = &val.start {
                            range_start = Some(parse_integer(&*start));
                        }
                        if let Some(end) = &val.end  {
                            range_end = Some(parse_integer(&*end));
                        }
                        match val.limits {
                            RangeLimits::HalfOpen(_) => {}
                            RangeLimits::Closed(_) => {
                                if let Some(end) = &mut range_end {
                                    *end += 1;
                                }
                            }
                        }
                        field_from_version = range_start;
                        field_to_version = range_end;
                    }
                    Expr::Lit(lit) => {

                        match &lit.lit {
                            Lit::Str(litstr) => {
                                let output: Vec<String> = litstr.value().split("..").map(|x| x.to_string()).collect();
                                if output.len() != 2 {
                                    abort!(litstr.span(), "savefile_versions tag must contain a (possibly half-open) range, such as 0..3 or 2.. (fields present in all versions to date should not use the savefile_versions-attribute)");
                                }
                                let (a, b) = (output[0].to_string(), output[1].to_string());

                                if field_from_version.is_some() || field_to_version.is_some() {
                                    abort!(
                                        litstr.span(),
                                        "There can only be one savefile_versions attribute on each field."
                                    )
                                }
                                if a.trim() == "" {
                                    field_from_version = Some(0);
                                } else if let Ok(a_u32) = a.parse::<u32>() {
                                    field_from_version = Some(a_u32);
                                } else {
                                    abort!(litstr.span(), "The from version in the version tag must be an integer. Use #[savefile_versions=0..3] for example");
                                }

                                if b.trim() == "" {
                                    field_to_version = Some(std::u32::MAX);
                                } else if let Ok(b_u32) = b.parse::<u32>() {
                                    field_to_version = Some(b_u32);
                                } else {
                                    abort!(litstr.span(), "The to version in the version tag must be an integer. Use #[savefile_versions=0..3] for example");
                                }
                                if field_to_version.expect("Expected field_to_version")
                                    < field_from_version.expect("expected field_from_version")
                                {
                                    abort!(
                                        litstr.span(),
                                        "savefile_versions ranges must specify lower number first."
                                    );
                                }
                            }
                            _ => {
                                abort!(lit.lit.span(), "Invalid savefile_versions-attribute value[1]");
                            }
                        }
                    }
                    _ => {
                        abort!(rhs.span(), "Invalid savefile_versions-attribute value [2]")
                    }
                }

            }
/*
            if path == "savefile_versions_as" {
                        match &x.lit {
                            &syn::Lit::Str(ref litstr2) => {

                            }
                            _ => abort!(
                                x.path.span(),
                                "Unexpected datatype for value of attribute savefile_versions_as"
                            ),
                        }
                    }

                    if path == "savefile_versions" {
                        match &x.lit {
                            &syn::Lit::Str(ref litstr) => {

                            }
                            _ => abort!(
                                x.lit.span(),
                                "Unexpected datatype for value of attribute savefile_versions"
                            ),
                        }
                    }
                }*/
            //},
            /*Err(e) => {
                abort!(attr.span(), "Unparsable attribute: {:?} ({})", e, attr.to_token_stream());
            }*/
        }
    }

    let versions_tag_range = VersionRange {
        from: field_from_version.unwrap_or(0),
        to: field_to_version.unwrap_or(std::u32::MAX),
        convert_fun: "dummy".to_string(),
        serialized_type: "dummy".to_string(),
    };
    if deser_types.iter().any(overlap(&versions_tag_range)) {
        abort_call_site!("The version ranges of #version_as attributes may not overlap those of #savefile_versions");
    }
    for dt in deser_types.iter() {
        if dt.to >= field_from_version.unwrap_or(0) {
            abort!(dt.to.span(), "The version ranges of #version_as attributes must be lower than those of the #savefile_versions attribute.");
        }
    }

    AttrsResult {
        version_from: field_from_version.unwrap_or(0),
        version_to: field_to_version.unwrap_or(std::u32::MAX),
        default_fn,
        default_val,
        ignore,
        deserialize_types: deser_types,
        introspect_key,
        introspect_ignore,
    }
}

fn parse_integer(p0: &Expr) -> u32 {
    match p0 {
        Expr::Lit(lit) => {
            match &lit.lit {
                Lit::Int(i) => {
                    match i.base10_parse() {
                        Ok(val) => val,
                        Err(_) => {
                            abort!(
                                p0.span(),
                                "Expected an integer"
                            )
                        }

                    }
                }
                _ =>             abort!(
                p0.span(),
                "Expected an integer"
            )

            }
        }
        _ => {
            abort!(
                p0.span(),
                "Expected an integer"
            )
        }
    }
}

pub(crate) struct FieldInfo<'a> {
    pub(crate) field_span: Span,
    pub(crate) ident: Option<syn::Ident>,
    pub(crate) index: u32,
    pub(crate) ty: &'a syn::Type,
    pub(crate) attrs: &'a Vec<syn::Attribute>,
}
impl<'a> FieldInfo<'a> {
    /// field name for named fields, .1 or .2 for tuple fields.
    pub fn get_accessor(&self) -> TokenStream {
        match &self.ident {
            None => {
                let index = syn::Index::from(self.index as usize);
                index.to_token_stream()
            }
            Some(id) => id.to_token_stream(),
        }
    }
}
pub(crate) fn compile_time_size(typ: &Type) -> Option<(usize /*size*/, usize /*alignment*/)> {
    match typ {
        Type::Path(p) => {
            if let Some(ident) = p.path.get_ident() {
                match ident.to_string().as_str() {
                    "u8" => Some((1, 1)),
                    "i8" => Some((1, 1)),
                    "u16" => Some((2, 2)),
                    "i16" => Some((2, 2)),
                    "u32" => Some((4, 4)),
                    "i32" => Some((4, 4)),
                    "u64" => Some((8, 8)),
                    "i64" => Some((8, 8)),
                    "char" => Some((4, 4)),
                    "bool" => Some((1, 1)),
                    "f32" => Some((4, 4)),
                    "f64" => Some((8, 8)),
                    _ => None,
                }
            } else {
                None
            }
        }
        Type::Tuple(t) => {
            let mut itemsize_align = None;
            let mut result_size = 0;
            if t.elems.iter().next().is_none() {
                //Empty tuple
                return Some((0, 1));
            }
            for item in t.elems.iter() {
                let (cursize, curalign) = compile_time_size(item)?;
                if let Some(itemsize_align) = itemsize_align {
                    if itemsize_align != (cursize, curalign) {
                        // All items not the same size and have same alignment. Otherwise: Might be padding issues.
                        return None; //It could conceivably still be packed, but we're conservative here.
                    }
                } else {
                    itemsize_align = Some((cursize, curalign));
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
                Expr::Lit(l) => match &l.lit {
                    Lit::Int(t) => {
                        let size: usize = t.base10_parse().ok()?;
                        Some((size * itemsize, itemalign))
                    }
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}
pub(crate) fn compile_time_check_reprc(typ: &Type) -> bool {
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
            match &x.len {
                Expr::Lit(l) => {
                    match &l.lit {
                        Lit::Int(l) => {
                            match l.base10_parse::<i64>() {
                                Ok(count) => {
                                    if count == 0 {
                                        return true; //0-length arrays are always reprc
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            compile_time_check_reprc(&x.elem)
        }
        Type::Tuple(t) => {
            let mut size = None;
            for x in &t.elems {
                if !compile_time_check_reprc(x) {
                    return false;
                }
                let xsize = if let Some(s) = compile_time_size(x) {
                    if s == (0, 0) || s == (0, 1) {
                        // Zero sized types can be mixed with other sizes, as long as all
                        // the other sizes are the same size.
                        continue;
                    }
                    s
                } else {
                    return false;
                };
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
        _ => false,
    }
}
