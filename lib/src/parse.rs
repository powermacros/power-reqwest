use convert_case::{Case, Casing};
use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseBuffer, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token::Paren,
    ExprRange, Ident, LitStr, Token,
};
use syn_prelude::{
    ForkWithParsible, ParseAsIdent, ParseAsLitStr, PathHelpers, ToErr, ToExpr, ToIdent,
    ToIdentWithCase, ToSpan, ToSynError, TryParseAsIdent, TryParseOneOfIdents, TryParseTokens,
    WithPrefix, WithSuffix,
};

use crate::model::*;

impl Parse for Client {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let whole_span = input.span();
        let mut client = Self {
            name: Ident::new("_", whole_span),
            options_name: None,
            options: None,
            hooks: None,
            auth: None,
            signing: None,
            apis: vec![],
        };
        while !input.is_empty() {
            if input.try_parse_comma().is_some() || input.try_parse_semi().is_some() {
                continue;
            }

            if let Some(api) = Api::try_parse(input)? {
                client.apis.push(api);
            } else if let Some(_ident) = input.try_parse_as_ident("name", true) {
                input.parse::<Token![:]>()?;
                let name: Ident = input.parse()?;
                if !name.to_string().is_case(Case::UpperCamel) {
                    name.to_syn_error("expect 'UpperCamel' case name")
                        .to_err()?;
                }
                client.name = name;
            } else if let Some(ident) = input.try_parse_one_of_idents(("params", "options")) {
                if let Some(params) = &client.options {
                    (ident.span(), params.token)
                        .to_span()
                        .to_syn_error("duplicated client params(options) config")
                        .to_err()?;
                }
                input.try_parse_colon();
                let params = ClientParams::parse(input, ident.span())?;
                for field in params.fields.iter() {
                    field.requires_to_simple_type()?;
                    field.check_constant_expr_with_type()?;
                }
                client.options = Some(params);
            } else if let Some(signing) = Signing::try_parse(input)? {
                if let Some(prev) = &client.signing {
                    (signing.span, prev.span)
                        .to_span()
                        .to_syn_error("duplicated signature config")
                        .to_err()?;
                }
                input.try_parse_colon();
                client.signing = Some(signing);
            } else if let Some(auth) = Auth::try_parse(input)? {
                if let Some(prev) = &client.auth {
                    (auth.span, prev.span)
                        .to_span()
                        .to_syn_error("duplicated authorization config")
                        .to_err()?;
                }
                input.try_parse_colon();
                client.auth = Some(auth);
            } else if let Some(hooks) = Hooks::try_parse(input)? {
                if let Some(prev) = &client.hooks {
                    (hooks.span, prev.span)
                        .to_span()
                        .to_syn_error("duplicated hooks config")
                        .to_err()?;
                }
                input.try_parse_colon();
                client.hooks = Some(hooks);
            } else {
                input
                    .span()
                    .to_syn_error("unexpect config field")
                    .to_err()?;
            }
        }
        if client.options.is_some() {
            client.options_name = Some(client.name.with_suffix("Options"));
        }
        client.resolve_object_type_names()?;
        Ok(client)
    }
}

impl Client {
    fn resolve_object_type_names(&mut self) -> syn::Result<()> {
        for api in self.apis.iter_mut() {
            let prefix = api.name.to_ident_with_case(Case::UpperCamel);
            if let Some(json) = &mut api.request.json {
                json.resolve_types(prefix.with_suffix("RequestData"))?;
            } else if let Some(form) = &mut api.request.form {
                form.resolve_types(prefix.with_suffix("RequestData"))?;
            };
            if let Some(headers) = &mut api.request.header {
                headers.resolve_types(prefix.with_suffix("RequestHeaders"))?;
            };
            if let Some(query) = &mut api.request.query {
                query.resolve_types(prefix.with_suffix("Query"))?;
            };

            if let Some(response) = &mut api.response {
                if let Some(json) = &mut response.json {
                    json.resolve_types(prefix.with_suffix("ResponseData"))?;
                } else if let Some(form) = &mut response.form {
                    form.resolve_types(prefix.with_suffix("ResponseData"))?;
                }
                if let Some(headers) = &mut response.header {
                    headers.resolve_types(prefix.with_suffix("ResponseHeaders"))?;
                }
                if let Some(cookies) = &mut response.cookie {
                    cookies.resolve_types(prefix.with_suffix("ResponseCookies"))?;
                }
            }
        }
        Ok(())
    }
}

impl Hooks {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(_common) = input.try_parse_as_ident("hooks", false) {
            input.try_parse_colon();
            let inner: ParseBuffer;
            let brace = syn::braced!(inner in input);
            let mut on_submit = None;
            while !inner.is_empty() {
                if let Some(_) = inner.try_parse_comma() {
                    continue;
                }

                if let Some(token) = inner.try_parse_as_ident("on_submit", false) {
                    inner.parse::<Token![:]>()?;
                    if on_submit.is_some() {
                        token.span().to_syn_error("duplicate config").to_err()?;
                    }
                    on_submit = Some(inner.parse()?);
                } else {
                    inner.span().to_syn_error("unsupported hook").to_err()?;
                }
            }
            let span = brace.span.close();

            Ok(Some(Self { span, on_submit }))
        } else {
            Ok(None)
        }
    }
}

impl Signing {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(_ident) = input.try_parse_one_of_idents(("signing", "signature")) {
            input.parse::<Token![:]>()?;
            let inner: ParseBuffer;
            let brace = syn::braced!(inner in input);
            let mut sign = Signing {
                span: brace.span.close(),
                sign_fn: Default::default(),
            };
            while !inner.is_empty() {
                if let Some(_) = inner.try_parse_comma() {
                    continue;
                }

                if let Some(_sign_token) = inner.try_parse_as_ident("sign", false) {
                    if sign.sign_fn.is_some() {
                        _sign_token
                            .span()
                            .to_syn_error("duplicate sign before")
                            .to_err()?;
                    }
                    inner.parse::<Token![:]>()?;
                    sign.sign_fn = Some(inner.parse()?);
                } else {
                    inner
                        .span()
                        .to_syn_error("unxpected signing field")
                        .to_err()?;
                }
            }
            //
            Ok(Some(sign))
        } else {
            Ok(None)
        }
    }
}

impl Auth {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(_ident) = input.try_parse_one_of_idents(("auth", "authorization")) {
            input.parse::<Token![:]>()?;
            let inner: ParseBuffer;
            let brace = syn::braced!(inner in input);

            let mut url = None;

            while !inner.is_empty() {
                if let Some(_) = inner.try_parse_comma() {
                    continue;
                }

                if let Some(_url) = inner.try_parse_as_ident("url", false) {
                    inner.parse::<Token![:]>()?;
                    if url.is_some() {
                        _url.to_syn_error("duplicated url config").to_err()?;
                    }
                    url = Some(inner.parse::<LitStr>()?);
                } else {
                    inner
                        .span()
                        .to_syn_error("unexpected authorization field")
                        .to_err()?;
                }
            }

            let span = brace.span.close();
            Ok(Some(Self {
                span,
                url: url.ok_or(span.to_syn_error("missing authorization url"))?,
            }))
        } else {
            Ok(None)
        }
    }
}

impl Api {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(method) = input.try_parse_one_of_idents(("get", "post", "put", "delete")) {
            let name = input.parse_as_ident()?;
            if !name.to_string().is_case(Case::Snake) {
                name.to_syn_error("method for client expects normal snake-case name")
                    .to_err()?;
            }
            let url_input: ParseBuffer;
            let paren = syn::parenthesized!(url_input in input);
            let uri: LitStr = url_input.parse()?;
            if !url_input.is_empty() {
                url_input
                    .span()
                    .to_syn_error("unexpected content")
                    .to_err()?;
            }

            let request = input.parse()?;
            let response = if input.peek(Token![->]) {
                input.parse::<Token![->]>()?;
                Some(input.parse()?)
            } else {
                None
            };

            Ok(Some(Self {
                method,
                name,
                paren,
                uri: ApiUri {
                    uri,
                    schema: None,
                    user: None,
                    passwd: None,
                    host: None,
                    port: None,
                    uri_path: None,
                    uri_query: None,
                    fragment: None,
                },
                request,
                response,
            }))
        } else {
            Ok(None)
        }
    }
}

impl Parse for ApiRequest {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut request = Self {
            brace,
            header: None,
            query: None,
            form: None,
            json: None,
        };

        while !inner.is_empty() {
            if let Some(_comma) = inner.try_parse_comma() {
                continue;
            }

            if let Some(json) = inner.try_parse_as_ident("json", false) {
                if let Some(prev) = request.json {
                    (json.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated json config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                request.json = Some(RequestJson::parse(input, json.span())?);
            } else if let Some(query) = inner.try_parse_as_ident("query", false) {
                if let Some(prev) = request.query {
                    (query.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated query config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                request.query = Some(RequestQueries::parse(&inner, query.span())?);
            } else if let Some(form) = inner.try_parse_as_ident("form", false) {
                if let Some(prev) = &request.form {
                    (form.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated form config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                request.form = Some(RequestForm::parse(&inner, form.span())?);
            } else if let Some(header) = inner.try_parse_as_ident("header", false) {
                if let Some(prev) = &request.header {
                    (header.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated header config")
                        .to_err()?;
                }
                input.try_parse_colon();
                request.header = Some(RequestHeaders::parse(&inner, header.span())?);
            } else {
                inner
                    .span()
                    .to_syn_error("unexpected config item")
                    .to_err()?;
            }
        }

        Ok(request)
    }
}

impl Parse for ApiResponse {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut response = Self {
            brace,
            header: None,
            cookie: None,
            json: None,
            form: None,
        };

        while !inner.is_empty() {
            if let Some(_comma) = inner.try_parse_comma() {
                continue;
            }

            if let Some(json) = inner.try_parse_as_ident("json", false) {
                if let Some(prev) = &response.json {
                    (json.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated json config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                response.json = Some(ResponseJson::parse(&inner, json.span())?);
            } else if let Some(form) = inner.try_parse_as_ident("form", false) {
                if let Some(prev) = &response.form {
                    (form.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated form config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                response.form = Some(ResponseForm::parse(&inner, form.span())?);
            } else if let Some(cookie) = inner.try_parse_as_ident("cookie", false) {
                if let Some(prev) = &response.cookie {
                    (cookie.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated cookie config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                response.cookie = Some(ResponseCookies::parse(&inner, cookie.span())?);
            } else if let Some(header) = inner.try_parse_as_ident("header", false) {
                if let Some(prev) = &response.header {
                    (header.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated header config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                response.header = Some(ResponseHeaders::parse(&inner, header.span())?);
            } else {
                inner
                    .span()
                    .to_syn_error("unexpected contents in response config")
                    .to_err()?;
            }
        }

        Ok(response)
    }
}

impl<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment> BracedConfig<T, A, X> {
    fn parse(input: ParseStream, token: Span) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut fields: Vec<Field<T, A, X>> = vec![];
        while !inner.is_empty() {
            if let Some(_) = inner.try_parse_comma() {
                continue;
            }
            let field: Field<T, A, X> = inner.parse()?;
            if let Some(prev) = fields.iter().find(|f| f.field_name.eq(&field.field_name)) {
                (field.name.span(), prev.name.span())
                    .to_span()
                    .to_syn_error("duplicated field")
                    .to_err()?;
            }
            fields.push(field);
        }
        Ok(Self {
            token,
            struct_name: ("_", token).to_ident(),
            brace,
            fields,
        })
    }
}

impl<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment> BracedConfig<T, A, X> {
    fn resolve_types(&mut self, name: Ident) -> syn::Result<()> {
        let prefix = name.to_string();
        self.struct_name = name.clone();
        for field in self.fields.iter_mut() {
            field.resolve_field_type(&prefix)?;
        }
        Ok(())
    }
}

impl<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment> Field<T, A, X> {
    fn resolve_field_type(&mut self, prefix: &str) -> syn::Result<()> {
        if let Some(typ) = self.typ.as_type_mut() {
            match typ {
                Type::Object(obj) => {
                    obj.resolve_type_name(&self.field_name, prefix, false)?;
                }
                Type::JsonText(JsonStringType { typ, .. }) => {
                    if let Some(Type::Object(obj)) = typ.as_type_mut() {
                        obj.resolve_type_name(&self.field_name, prefix, false)?;
                    }
                }
                Type::List(ListType { element_type, .. }) => {
                    if let Type::Object(obj) = element_type.as_mut() {
                        obj.resolve_type_name(&self.field_name, prefix, true)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl<A: AsFieldAlias, X: AsFieldAssignment> ObjectType<A, X> {
    fn resolve_type_name(
        &mut self,
        field_name: &Ident,
        prefix: &str,
        is_list_item: bool,
    ) -> syn::Result<()> {
        self.struct_name = field_name
            .to_ident_with_case(Case::UpperCamel)
            .with_prefix(prefix);
        let obj_name = self.struct_name.to_string();
        for child in self.fields.iter_mut() {
            child.resolve_field_type(&obj_name)?;
        }
        Ok(())
    }
}

impl<A: AsFieldAlias, X: AsFieldAssignment> Parse for ObjectType<A, X> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut fields: Vec<ObjectField<A, X>> = vec![];
        while !inner.is_empty() {
            if let Some(_) = inner.try_parse_comma() {
                continue;
            }
            let field: ObjectField<A, X> = inner.parse()?;
            if let Some(prev) = fields.iter().find(|f| f.name.eq(&field.name)) {
                (field.name.span(), prev.name.span())
                    .to_span()
                    .to_syn_error("duplicated field")
                    .to_err()?;
            }
            fields.push(field);
        }
        Ok(Self {
            struct_name: Ident::new("_", brace.span.close()),
            brace,
            fields,
        })
    }
}
impl<A: AsFieldAlias, X: AsFieldAssignment> Type<A, X> {
    fn parse_basic(input: ParseStream) -> syn::Result<Self> {
        Ok(if input.peek(syn::token::Brace) {
            Self::Object(input.parse()?)
        } else if let Some(string) = StringType::try_parse(input)? {
            Self::String(string)
        } else if let Some(integer) = IntegerType::try_parse(input)? {
            Self::Integer(integer)
        } else if let Some(bool) = input.try_parse_as_ident("bool", false) {
            Self::Bool(bool.span())
        } else if let Some(json) = JsonStringType::try_parse(input)? {
            Self::JsonText(json)
        } else if let Some(object) = input.try_parse_as_ident("object", false) {
            Self::Map(object.span())
        } else if let Some(float) = FloatType::try_parse(input)? {
            Self::Float(float)
        } else if let Some(datetime) = DateTimeStringType::try_parse(input)? {
            Self::DatetimeString(datetime)
        } else if let Some(constant) = Constant::try_parse(input)? {
            Self::Constant(constant)
        } else {
            input
                .span()
                .to_syn_error("illegal config value type")
                .to_err()?
        })
    }
}

impl<A, X> ToSpan for Type<A, X> {
    fn to_span(&self) -> Span {
        match self {
            Self::Constant(c) => c.span(),
            Self::String(s) => s.span,
            Self::Bool(s) => *s,
            Self::Integer(i) => i.token.span(),
            Self::Float(f) => f.token.span(),
            Self::Object(o) => o.brace.span.close(),
            Self::DatetimeString(d) => d.span,
            Self::JsonText(j) => j.span,
            Self::Map(s) => *s,
            Self::List(l) => (l.element_type.to_span(), l.bracket.span.close()).to_span(),
        }
    }
}

impl<A: AsFieldAlias, X: AsFieldAssignment> AsFieldType<A, X> for Type<A, X> {
    fn peek(input: ParseStream) -> syn::Result<()> {
        if !input.peek(syn::token::Brace) {
            input.parse::<Token![:]>()?;
        }
        Ok(())
    }

    fn parse_type(input: ParseStream) -> syn::Result<Self> {
        let mut typ = Self::parse_basic(input)?;
        if input.peek(syn::token::Bracket) {
            let inner: ParseBuffer;
            let bracket = syn::bracketed!(inner in input);
            if inner.is_empty() {
                typ = Self::List(ListType {
                    bracket,
                    element_type: Box::new(typ),
                });
            } else {
                inner
                    .span()
                    .to_syn_error("unexpect content for list type")
                    .to_err()?;
            }
        }
        Ok(typ)
    }

    fn as_type(&self) -> Option<&Type<A, X>> {
        Some(self)
    }

    fn as_type_mut(&mut self) -> Option<&mut Type<A, X>> {
        Some(self)
    }
}

impl StringType {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_one_of_idents(("string", "String", "str")) {
            Ok(Some(Self { span: ident.span() }))
        } else {
            Ok(None)
        }
    }
}

impl IntegerType {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(token) = input.try_parse_one_of_idents((
            "uint", "int", "integer", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64",
            "usize", "isize",
        )) {
            Ok(Some(Self {
                token,
                limits: IntLimits::try_parse(input)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn is_u64(&self) -> bool {
        self.token.eq("uint") || self.token.eq("u64")
    }
}

impl IntLimits {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if input.peek(syn::token::Paren) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let limits = inner.parse_terminated(IntLimit::parse, Token![,])?;
            let mut last_max = usize::MAX;
            for limit in limits.iter() {
                match limit {
                    IntLimit::Range(r) => {
                        if let Some(start) = &r.start {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(i),
                                ..
                            }) = start.as_ref()
                            {
                                if last_max != usize::MAX {
                                    if last_max > i.base10_parse()? {
                                        i.span().to_syn_error("range conflict").to_err()?;
                                    }
                                }
                            } else {
                                start.span().to_syn_error("expect integer value").to_err()?;
                            }
                        }
                        if let Some(end) = &r.end {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(i),
                                ..
                            }) = end.as_ref()
                            {
                                last_max = i.base10_parse()?;
                            } else {
                                end.span().to_syn_error("expect integer value").to_err()?;
                            }
                        }
                    }
                    IntLimit::Opt(v) => {
                        let v = v.base10_parse()?;
                        if v > last_max {
                            last_max = v;
                        }
                    }
                }
            }
            Ok(Some(Self { paren, limits }))
        } else {
            Ok(None)
        }
    }
}

impl Parse for IntLimit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let res = input.fork_with_parsible::<ExprRange>();
        Ok(if res.is_err() {
            Self::Opt(input.parse()?)
        } else {
            Self::Range(res?)
        })
    }
}

impl FloatType {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(float) = input.try_parse_one_of_idents(("float", "f64", "f32")) {
            Ok(Some(Self {
                token: float,
                limits: FloatLimits::try_parse(input)?,
            }))
        } else {
            Ok(None)
        }
    }
}

impl FloatLimits {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if input.peek(syn::token::Paren) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let limits = inner.parse_terminated(ExprRange::parse, Token![,])?;
            Ok(Some(Self { paren, limits }))
        } else {
            Ok(None)
        }
    }
}

impl<A: AsFieldAlias, X: AsFieldAssignment> TryParse for JsonStringType<A, X> {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(json) = input.try_parse_as_ident("json", false) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            Ok(Some(Self {
                paren,
                span: json.span(),
                typ: Box::new(Type::parse_type(&inner)?),
            }))
        } else {
            Ok(None)
        }
    }
}

impl DateTimeStringType {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_one_of_idents(("datetime", "date")) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let format = inner.parse::<LitStr>()?;
            Ok(Some(Self {
                paren,
                span: ident.span(),
                format,
            }))
        } else {
            Ok(None)
        }
    }
}

impl<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment> Parse for Field<T, A, X> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse_as_lit_str()?;
        let optional = input.try_parse_question();
        T::peek(input)?;
        let typ = T::parse_type(input)?;
        let alias = A::try_parse(input)?;
        let expr = X::try_parse(input)?;
        let field_name = if let Some(alias) = &alias {
            if let Some(alias) = alias.as_alias() {
                alias.clone()
            } else {
                name.to_ident_with_case(Case::Snake)
            }
        } else {
            name.to_ident_with_case(Case::Snake)
        };
        let mut default = None;
        if let Some(Type::Constant(c)) = typ.as_type() {
            default = Some(c.to_value());
        } else if let Some(x) = &expr {
            if let Some(Expr::Constant(c)) = x.as_assignment() {
                default = Some(c.to_value());
            }
        }
        if let Some(opt) = &optional {
            default = Some(syn::Path::from_ident(("None", opt.span()).to_ident()).to_expr());
        }
        Ok(Self {
            name,
            field_name,
            optional: optional.map(|o| o.span()),
            typ,
            alias,
            expr,
            default,
        })
    }
}

impl<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment> Field<T, A, X> {
    fn requires_to_simple_type(&self) -> syn::Result<()> {
        if let Some(t) = self.typ.as_type() {
            match t {
                Type::Object(_) => t.to_span().to_syn_error("unsupported type").to_err(),
                Type::Map(_) => t.to_span().to_syn_error("unsupported type").to_err(),
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    fn check_constant_expr_with_type(&self) -> syn::Result<()> {
        if let (Some(t), Some(x)) = (self.typ.as_type(), self.expr.as_ref()) {
            if let Some(x) = x.as_assignment() {
                let okay = is_type_and_value_match(t, x);
                if !okay {
                    (t.to_span(), x.to_span())
                        .to_span()
                        .to_syn_error("unmatch type with value")
                        .to_err()?;
                }
            }
        }
        Ok(())
    }
}

fn is_type_and_constant_match<A, X>(t: &Type<A, X>, c: &Constant) -> bool {
    match (t, c) {
        (Type::String(_), Constant::String(_)) => true,
        (Type::Integer(_), Constant::Int(_)) => true,
        (Type::Float(_), Constant::Float(_)) => true,
        (Type::Bool(_), Constant::Bool(_)) => true,
        _ => false,
    }
}

fn is_type_and_value_match<A, X>(t: &Type<A, X>, x: &Expr) -> bool {
    match (t, x) {
        (Type::String(_), Expr::Json(_)) => true,
        (Type::String(_), Expr::Datetime(_)) => true,
        (Type::String(_), Expr::Format(_)) => true,
        (Type::String(_), Expr::Join(_)) => true,
        (Type::Integer(i), Expr::Timestamp(_)) => i.is_u64(),
        (t, Expr::Constant(c)) => is_type_and_constant_match(t, c),
        (t, Expr::Or(OrExpr { default, .. })) => is_type_and_constant_match(t, default),
        _ => false,
    }
}

impl Parse for Expr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let expr = if let Some(dollar) = input.try_parse_dollar() {
            let variable = Variable::continue_to_parse(input, dollar)?;
            if input.peek(Token![||]) {
                Self::Or(OrExpr::parse(input, variable)?)
            } else {
                Self::Variable(variable)
            }
        } else if let Some(string) = JsonStringifyFn::try_parse(input)? {
            Self::Json(string)
        } else if let Some(string) = DatetimeFn::try_parse(input)? {
            Self::Datetime(string)
        } else if let Some(string) = FormatFn::try_parse(input)? {
            Self::Format(string)
        } else if let Some(string) = JoinStringFn::try_parse(input)? {
            Self::Join(string)
        } else if let Some(uint) = UnixTimestampUintFn::try_parse(input)? {
            Self::Timestamp(uint)
        } else {
            Self::Constant(input.parse()?)
        };
        Ok(expr)
    }
}

impl ToSpan for Expr {
    fn to_span(&self) -> Span {
        match self {
            Self::Constant(x) => x.to_span(),
            Self::Variable(x) => x.to_span(),
            Self::Json(x) => x.to_span(),
            Self::Format(x) => x.to_span(),
            Self::Datetime(x) => x.to_span(),
            Self::Timestamp(x) => x.to_span(),
            Self::Join(x) => x.to_span(),
            Self::Or(x) => x.to_span(),
        }
    }
}

impl Parse for Constant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if let Some(constant) = Self::try_parse(input)? {
            Ok(constant)
        } else {
            input.span().to_syn_error("missing constant").to_err()
        }
    }
}

impl ToSpan for Constant {
    fn to_span(&self) -> Span {
        match self {
            Constant::String(c) => c.span(),
            Constant::Bool(c) => c.span(),
            Constant::Int(c) => c.span(),
            Constant::Float(c) => c.span(),
            Constant::Object(c) => c.span,
            Constant::Array(c) => c.span,
        }
    }
}

impl Constant {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        Ok(if input.peek(LitStr) {
            Some(Self::String(input.parse()?))
        } else if input.peek(syn::LitInt) {
            Some(Self::Int(input.parse()?))
        } else if input.peek(syn::LitFloat) {
            Some(Self::Float(input.parse()?))
        } else if input.peek(syn::LitBool) {
            Some(Self::Bool(input.parse()?))
        } else if input.peek(syn::token::Brace) {
            Some(Self::Object(input.parse()?))
        } else if input.peek(syn::token::Bracket) {
            Some(Self::Array(input.parse()?))
        } else {
            None
        })
    }

    pub fn span(&self) -> Span {
        match self {
            Constant::String(s) => s.span(),
            Constant::Bool(b) => b.span(),
            Constant::Int(i) => i.span(),
            Constant::Float(f) => f.span(),
            Constant::Object(o) => o.span(),
            Constant::Array(a) => a.span(),
        }
    }

    pub fn to_value(&self) -> syn::Expr {
        match self {
            Constant::String(c) => syn::Expr::MethodCall(syn::ExprMethodCall {
                attrs: vec![],
                receiver: Box::new(c.to_expr()),
                dot_token: Token![.](c.span()),
                method: ("to_owned", c.span()).to_ident(),
                turbofish: None,
                paren_token: Paren(c.span()),
                args: Punctuated::new(),
            }),
            Constant::Bool(c) => c.to_expr(),
            Constant::Int(c) => c.to_expr(),
            Constant::Float(c) => c.to_expr(),
            Constant::Object(c) => todo!(),
            Constant::Array(c) => c
                .elements
                .iter()
                .map(|c| c.to_value())
                .collect::<Vec<_>>()
                .to_expr(),
        }
    }
}

impl Parse for ObjectConstant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut fields: Vec<ObjectConstantField> = vec![];
        while !inner.is_empty() {
            if inner.peek(Token![,]) {
                inner.parse::<Token![,]>()?;
                continue;
            }
            let field: ObjectConstantField = inner.parse()?;
            if !fields.is_empty() {
                if fields.iter().find(|f| f.name.eq(&field.name)).is_some() {
                    field.name.span().to_syn_error("duplicated key").to_err()?;
                }
            }
            fields.push(field);
        }
        Ok(Self {
            fields,
            span: brace.span.close(),
        })
    }
}

impl ObjectConstant {
    pub fn span(&self) -> Span {
        self.span
    }
}

impl Parse for ConstantArray {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let bracket = syn::bracketed!(inner in input);
        let mut elements = vec![];
        while !inner.is_empty() {
            if inner.peek(Token![,]) {
                inner.parse::<Token![,]>()?;
                continue;
            }
            elements.push(inner.parse()?);
        }
        Ok(Self {
            elements,
            span: bracket.span.close(),
        })
    }
}

impl ConstantArray {
    pub fn span(&self) -> Span {
        self.span
    }
}

impl Parse for ObjectConstantField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let value = input.parse()?;
        Ok(Self { name, value })
    }
}

impl Variable {
    fn continue_to_parse(input: ParseStream, dollar: Token![$]) -> syn::Result<Self> {
        let name = input.parse()?;
        Ok(if input.peek(Token![:]) {
            Type::<(), ()>::peek(input)?;
            Self {
                dollar: dollar.span,
                name,
                typ: Some(Type::parse_type(input)?),
            }
        } else {
            Self {
                dollar: dollar.span,
                name,
                typ: None,
            }
        })
    }
}

impl ToSpan for Variable {
    fn to_span(&self) -> Span {
        if let Some(typ) = &self.typ {
            (self.dollar, typ.to_span()).to_span()
        } else {
            (self.dollar, self.name.span()).to_span()
        }
    }
}

impl Parse for Variable {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let dollar = input.parse()?;
        Self::continue_to_parse(input, dollar)
    }
}

impl OrExpr {
    fn parse(input: ParseStream, variable: Variable) -> syn::Result<Self> {
        let or = input.parse::<Token![||]>()?;
        Ok(Self {
            variable,
            or,
            default: input.parse()?,
        })
    }
}

impl ToSpan for OrExpr {
    fn to_span(&self) -> Span {
        (self.variable.to_span(), self.default.to_span()).to_span()
    }
}

impl FormatFn {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_one_of_idents(("format", "fmt")) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let format_text = inner.parse::<LitStr>()?;
            let args = if let Some(_comma) = inner.try_parse_comma() {
                Some(inner.parse_terminated(Expr::parse, Token![,])?)
            } else {
                None
            };
            Ok(Some(Self {
                fn_token: ident.span(),
                paren,
                format_text,
                args,
            }))
        } else {
            Ok(None)
        }
    }
}
impl ToSpan for FormatFn {
    fn to_span(&self) -> Span {
        (self.fn_token, self.paren.span.close()).to_span()
    }
}

impl JsonStringifyFn {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(json) = input.try_parse_as_ident("json", false) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let variable = inner.parse()?;
            Ok(Some(Self {
                fn_token: json.span(),
                paren,
                variable,
            }))
        } else {
            Ok(None)
        }
    }
}
impl ToSpan for JsonStringifyFn {
    fn to_span(&self) -> Span {
        (self.fn_token, self.paren.span.close()).to_span()
    }
}

impl DatetimeFn {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_one_of_idents(("datetime", "date")) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let variable = Variable::parse(&inner)?;
            inner.parse::<Token![,]>()?;
            let format = inner.parse::<LitStr>()?;
            // FIXME: validate format
            Ok(Some(Self {
                token: ident.span(),
                paren,
                variable,
                format,
            }))
        } else {
            Ok(None)
        }
    }
}
impl ToSpan for DatetimeFn {
    fn to_span(&self) -> Span {
        (self.token, self.paren.span.close()).to_span()
    }
}

impl JoinStringFn {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_one_of_idents(("join_string", "join")) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let variable = Variable::parse(&inner)?;
            inner.parse::<Token![,]>()?;
            let sep = inner.parse::<LitStr>()?;
            Ok(Some(Self {
                token: ident.span(),
                paren,
                variable,
                sep,
            }))
        } else {
            Ok(None)
        }
    }
}
impl ToSpan for JoinStringFn {
    fn to_span(&self) -> Span {
        (self.token, self.paren.span.close()).to_span()
    }
}

impl UnixTimestampUintFn {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_one_of_idents(("timestamp", "unix_timestamp")) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let variable = Variable::parse(&inner)?;
            Ok(Some(Self {
                token: ident.span(),
                paren,
                variable,
            }))
        } else {
            Ok(None)
        }
    }
}

impl ToSpan for UnixTimestampUintFn {
    fn to_span(&self) -> Span {
        (self.token, self.paren.span.close()).to_span()
    }
}
