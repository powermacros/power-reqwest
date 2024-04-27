use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseBuffer, ParseStream},
    ExprRange, Ident, LitStr, Token,
};
use syn_prelude::{
    ForkWithParsible, ParseAsIdent, ParseAsLitStr, ToErr, ToSpan, ToSynError, TryParseAsIdent,
    TryParseOneOfIdents, TryParseTokens,
};

use crate::model::*;

impl Parse for Client {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let whole_span = input.span();
        let mut client_name = None;
        let mut params = None;
        let mut signing = None;
        let mut auth = None;
        let mut common = None;
        let mut api_list = vec![];
        while !input.is_empty() {
            if input.try_parse_comma().is_some() || input.try_parse_semi().is_some() {
                continue;
            }

            let decl: ClientDecl = input.parse()?;
            match decl {
                ClientDecl::Name(_token, name) => {
                    if client_name.is_some() {
                        name.to_syn_error("duplicate client name defined previously")
                            .to_err()?;
                    }
                    client_name = Some(name);
                }
                ClientDecl::Params(decl) => {
                    if params.is_some() {
                        decl.token
                            .to_syn_error("duplicate client config defined previously")
                            .to_err()?;
                    }
                    params = Some(decl);
                }
                ClientDecl::Signature(decl) => {
                    if signing.is_some() {
                        decl.span
                            .to_syn_error("duplicate signature/signing config defined previously")
                            .to_err()?;
                    }
                    signing = Some(decl);
                }
                ClientDecl::Auth(decl) => {
                    if auth.is_some() {
                        decl.span
                            .to_syn_error("duplicate authorization config defined previously")
                            .to_err()?;
                    }
                    auth = Some(decl);
                }
                ClientDecl::Api(decl) => {
                    api_list.push(decl);
                }
                ClientDecl::Common(decl) => {
                    if common.is_some() {
                        decl.span
                            .to_syn_error("duplicate common config defined previously")
                            .to_err()?;
                    }
                    common = Some(decl);
                }
            }
        }
        Ok(Self {
            name: client_name.ok_or(whole_span.to_syn_error("missing client name!"))?,
            params,
            hooks: common,
            auth,
            signing,
            apis: api_list,
        })
    }
}

enum ClientDecl {
    Name(Ident, Ident),
    Params(ClientParams),
    Signature(Signing),
    Auth(Auth),
    Api(Api),
    Common(Hooks),
}

impl Parse for ClientDecl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if let Some(api) = Api::try_parse(input)? {
            Ok(Self::Api(api))
        } else if let Some(ident) = input.try_parse_as_ident("name", true) {
            input.parse::<Token![:]>()?;
            Ok(Self::Name(ident, input.parse()?))
        } else if let Some(ident) = input.try_parse_as_ident("params", false) {
            input.try_parse_colon();
            Ok(Self::Params(ClientParams::parse(input, ident.span())?))
        } else if let Some(signing) = Signing::try_parse(input)? {
            Ok(Self::Signature(signing))
        } else if let Some(auth) = Auth::try_parse(input)? {
            Ok(Self::Auth(auth))
        } else if let Some(common) = Hooks::try_parse(input)? {
            Ok(Self::Common(common))
        } else {
            input.span().to_syn_error("unexpect config field").to_err()
        }
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
            let url_input: ParseBuffer;
            let paren = syn::parenthesized!(url_input in input);
            let url: LitStr = url_input.parse()?;
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
                url,
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

impl<T: ParseType, A: TryParse, X: TryParse> BracedConfig<T, A, X> {
    fn parse(input: ParseStream, token: Span) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut fields: Vec<Field<T, A, X>> = vec![];
        while !inner.is_empty() {
            if let Some(_) = inner.try_parse_comma() {
                continue;
            }
            let field: Field<T, A, X> = inner.parse()?;
            if let Some(prev) = fields.iter().find(|f| f.name.eq(&field.name)) {
                (field.name.span(), prev.name.span())
                    .to_span()
                    .to_syn_error("duplicated field")
                    .to_err()?;
            }
            fields.push(field);
        }
        Ok(Self {
            token,
            brace,
            fields,
        })
    }
}

impl<A: TryParse, X: TryParse> Parse for ObjectType<A, X> {
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
        Ok(Self { brace, fields })
    }
}
impl<A: TryParse, X: TryParse> Type<A, X> {
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
            Self::Json(json)
        } else if let Some(object) = input.try_parse_as_ident("object", false) {
            Self::Map(object.span())
        } else if let Some(float) = FloatType::try_parse(input)? {
            Self::Float(float)
        } else if let Some(datetime) = DateTimeStringType::try_parse(input)? {
            Self::Datetime(datetime)
        } else if let Some(constant) = Constant::try_parse(input)? {
            Self::Constant(constant)
        } else {
            input
                .span()
                .to_syn_error("illegal config value type")
                .to_err()?
        })
    }

    fn span(&self) -> Span {
        match self {
            Self::Constant(c) => c.span(),
            Self::String(s) => s.span,
            Self::Bool(s) => *s,
            Self::Integer(i) => i.token.span(),
            Self::Float(f) => f.span,
            Self::Object(o) => o.brace.span.close(),
            Self::Datetime(d) => d.span,
            Self::Json(j) => j.span,
            Self::Map(s) => *s,
            Self::List(l) => (l.element_type.span(), l.bracket.span.close()).to_span(),
        }
    }
}

impl<A: TryParse, X: TryParse> ParseType for Type<A, X> {
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
        if let Some(token) = input.try_parse_one_of_idents(("uint", "int", "integer")) {
            Ok(Some(Self {
                token,
                limits: IntLimits::try_parse(input)?,
            }))
        } else {
            Ok(None)
        }
    }
}

impl IntLimits {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if input.peek(syn::token::Paren) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let limits = inner.parse_terminated(IntLimit::parse, Token![,])?;
            // FIXME: validate limits
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
        if let Some(float) = input.try_parse_as_ident("float", false) {
            Ok(Some(Self {
                span: float.span(),
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

impl<A: TryParse, X: TryParse> TryParse for JsonStringType<A, X> {
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

impl<T: ParseType, A: TryParse, X: TryParse> Parse for Field<T, A, X> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse_as_lit_str()?;
        let optional = input.try_parse_question();
        T::peek(input)?;
        let typ = T::parse_type(input)?;
        Ok(Self {
            name,
            optional,
            typ,
            alias: A::try_parse(input)?,
            expr: X::try_parse(input)?,
        })
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

impl Parse for Constant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if let Some(constant) = Self::try_parse(input)? {
            Ok(constant)
        } else {
            input.span().to_syn_error("missing constant").to_err()
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

    fn span(&self) -> Span {
        match self {
            Constant::String(s) => s.span(),
            Constant::Bool(b) => b.span(),
            Constant::Int(i) => i.span(),
            Constant::Float(f) => f.span(),
            Constant::Object(o) => o.span(),
            Constant::Array(a) => a.span(),
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
    fn span(&self) -> Span {
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
    fn span(&self) -> Span {
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

impl UnixTimestampUintFn {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_as_ident("timestamp", false) {
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
