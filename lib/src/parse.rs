use syn::{
    parse::{Parse, ParseBuffer, ParseStream},
    spanned::Spanned,
    token::Brace,
    Ident, LitBool, LitFloat, LitInt, LitStr, Token,
};
use syn_prelude::{
    ParseAsIdent, ParseAsLitStr, ToErr, ToSynError, TryParseAsIdent, TryParseOneOfIdents,
    TryParseTokens,
};

use crate::{
    model::{
        Api, ArrayValue, Client, DataField, DataType, ObjectFieldType, ObjectFieldValue,
        ObjectType, ObjectValue, Signing, Value,
    },
    ApiHeader, ApiRequest, Auth, Common,
};

trait TryParseComma {
    fn try_parse_comma(&self) -> syn::Result<()>;
}

impl Parse for Client {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let whole_span = input.span();
        let mut client_name = None;
        let mut config = None;
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
                ClientDecl::Config(decl, brace) => {
                    if config.is_some() {
                        brace
                            .span
                            .close()
                            .to_syn_error("duplicate client config defined previously")
                            .to_err()?;
                    }
                    config = Some(decl);
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
            config,
            common,
            auth,
            signing,
            apis: api_list,
        })
    }
}

enum ClientDecl {
    Name(Ident, Ident),
    Config(Vec<DataField>, Brace),
    Signature(Signing),
    Auth(Auth),
    Api(Api),
    Common(Common),
}

impl Parse for ClientDecl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if let Some(api) = Api::try_parse(input)? {
            Ok(Self::Api(api))
        } else if let Some(ident) = input.try_parse_as_ident("name", true) {
            input.parse::<Token![:]>()?;
            Ok(Self::Name(ident, input.parse()?))
        } else if let Some(_ident) = input.try_parse_as_ident("config", false) {
            input.try_parse_colon();
            let (items, brace) = parse_config_fields(input)?;
            Ok(Self::Config(items, brace))
        } else if let Some(signing) = Signing::try_parse(input)? {
            Ok(Self::Signature(signing))
        } else if let Some(auth) = Auth::try_parse(input)? {
            Ok(Self::Auth(auth))
        } else if let Some(common) = Common::try_parse(input)? {
            Ok(Self::Common(common))
        } else {
            input.span().to_syn_error("unexpect config field").to_err()
        }
    }
}

impl Common {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(_common) = input.try_parse_as_ident("common", false) {
            input.try_parse_colon();
            let inner: ParseBuffer;
            let brace = syn::braced!(inner in input);
            let mut unwrap_response = None;
            while !inner.is_empty() {
                if let Some(_) = inner.try_parse_comma() {
                    continue;
                }

                if let Some(token) = inner.try_parse_as_ident("unwrap_response", false) {
                    inner.parse::<Token![:]>()?;
                    if unwrap_response.is_some() {
                        token.span().to_syn_error("duplicate config").to_err()?;
                    }
                    unwrap_response = Some(inner.parse()?);
                } else {
                    inner
                        .span()
                        .to_syn_error("unexpect field for common config")
                        .to_err()?;
                }
            }
            let span = brace.span.close();

            Ok(Some(Self {
                span,
                unwrap_response,
            }))
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
            let parened: ParseBuffer;
            syn::parenthesized!(parened in input);
            let url: LitStr = parened.parse()?;
            if !parened.is_empty() {
                parened.span().to_syn_error("unexpected content").to_err()?;
            }

            let inner: ParseBuffer;
            let brace = syn::braced!(inner in input);

            let mut name = None;
            let mut response = None;
            let mut header = None;
            let mut request_data = None;

            while !inner.is_empty() {
                if let Some(_) = inner.try_parse_comma() {
                    continue;
                }
                if let Some(_name) = inner.try_parse_as_ident("name", false) {
                    inner.parse::<Token![:]>()?;
                    if name.is_some() {
                        _name.to_syn_error("name has been configured").to_err()?;
                    }
                    name = Some(inner.parse_as_lit_str()?);
                } else if let Some(_) = inner.try_parse_as_ident("request", false) {
                    inner.try_parse_colon();
                    let request: ParseBuffer;
                    syn::braced!(request in inner);
                    while !request.is_empty() {
                        if let Some(_) = request.try_parse_comma() {
                            continue;
                        };
                        if let Some(_config) = request.try_parse_as_ident("header", false) {
                            request.try_parse_colon();
                            if header.is_some() {
                                _config
                                    .span()
                                    .to_syn_error("duplicate header config")
                                    .to_err()?;
                            }
                            let header_buffer: ParseBuffer;
                            syn::braced!(header_buffer in request);
                            let mut list = vec![];
                            while !header_buffer.is_empty() {
                                if let Some(_) = header_buffer.try_parse_comma() {
                                    continue;
                                }
                                list.push(header_buffer.parse()?);
                            }
                            header = Some(list);
                        } else if let Some(_) = request.try_parse_as_ident("data", false) {
                            request.try_parse_colon();
                            let (items, brace) = parse_config_fields(&request)?;
                            if request_data.is_some() {
                                brace
                                    .span
                                    .close()
                                    .to_syn_error("request data has been defined previously")
                                    .to_err()?;
                            }
                            request_data = Some(items);
                        } else {
                            request
                                .span()
                                .to_syn_error("unexpect field for request config")
                                .to_err()?;
                        }
                    }
                } else if let Some(_response) = inner.try_parse_as_ident("response", false) {
                    inner.try_parse_colon();
                    if response.is_some() {
                        _response.span().to_syn_error("duplicated field").to_err()?;
                    }
                    let (items, _) = parse_config_fields(&inner)?;
                    response = Some(items);
                } else {
                    inner
                        .span()
                        .to_syn_error("unexpect contents for api config")
                        .to_err()?;
                }
            }

            Ok(Some(Self {
                name: name.ok_or(brace.span.close().to_syn_error("missing api name"))?,
                response: response.ok_or(
                    brace
                        .span
                        .close()
                        .to_syn_error("missing response type config"),
                )?,
                request: {
                    if request_data.is_some() || header.is_some() {
                        Some(ApiRequest {
                            header: header,
                            data: request_data,
                        })
                    } else {
                        None
                    }
                },
                method,
                url,
            }))
        } else {
            Ok(None)
        }
    }
}

fn parse_config_fields(input: ParseStream) -> syn::Result<(Vec<DataField>, Brace)> {
    let inner: ParseBuffer;
    let brace = syn::braced!(inner in input);
    let mut items: Vec<DataField> = vec![];
    while !inner.is_empty() {
        if let Some(_) = inner.try_parse_comma() {
            continue;
        }
        let item: DataField = inner.parse()?;
        if !items.is_empty() {
            if items.iter().find(|i| i.name.eq(&item.name)).is_some() {
                item.name
                    .span()
                    .to_syn_error("same config key exists")
                    .to_err()?;
            }
        }
        items.push(item);
    }
    Ok((items, brace))
}

impl Parse for ApiHeader {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse_as_lit_str()?;
        input.parse::<Token![=]>()?;
        let value = input.parse()?;
        Ok(Self { name, value })
    }
}

impl Parse for DataField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse_as_ident()?;
        let optional = if let Some(tk) = input.try_parse_question() {
            Some(tk.span())
        } else {
            None
        };
        input.parse::<Token![:]>()?;
        let typ = input.parse()?;
        let default_value = if let Some(_) = input.try_parse_eq() {
            Some(input.parse()?)
        } else {
            None
        };

        Ok(Self {
            name,
            typ,
            value: default_value,
            optional,
        })
    }
}

impl Parse for DataType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut typ = if input.peek(syn::token::Brace) {
            Self::Object(input.parse()?)
        } else {
            let ty = input.parse_as_ident()?;
            match ty.to_string().as_str() {
                "string" | "String" => Self::String(ty.span()),
                "bool" | "boolean" => Self::Bool(ty.span()),
                "int" => Self::Int(ty.span()),
                "uint" => Self::Uint(ty.span()),
                "float" => Self::Float(ty.span()),
                _ => ty.to_syn_error("illegal config value type").to_err()?,
            }
        };
        if input.peek(syn::token::Bracket) {
            let inner: ParseBuffer;
            syn::bracketed!(inner in input);
            if inner.is_empty() {
                typ = Self::List(Box::new(typ));
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

impl Parse for ObjectType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        syn::braced!(inner in input);
        let mut fields: Vec<ObjectFieldType> = vec![];
        while !inner.is_empty() {
            if inner.peek(Token![,]) {
                inner.parse::<Token![,]>()?;
                continue;
            }
            let field: ObjectFieldType = inner.parse()?;
            if !fields.is_empty() {
                if fields.iter().find(|f| f.name.eq(&field.name)).is_some() {
                    field.name.span().to_syn_error("duplicate key").to_err()?;
                }
            }
            fields.push(field);
        }
        Ok(Self { fields })
    }
}

impl Parse for ObjectFieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse_as_ident()?;
        input.parse::<Token![:]>()?;
        let ty: DataType = input.parse()?;
        let default_value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(input.parse::<Value>()?)
        } else {
            None
        };
        Ok(Self {
            name,
            value_type: ty,
            value: default_value,
        })
    }
}

impl Parse for Value {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(if let Some(_) = input.try_parse_dollar() {
            Self::Var(input.parse()?)
        } else if input.peek(LitStr) {
            Self::String(input.parse()?)
        } else if input.peek(LitInt) {
            Self::Int(input.parse()?)
        } else if input.peek(LitFloat) {
            Self::Float(input.parse()?)
        } else if input.peek(LitBool) {
            Self::Bool(input.parse()?)
        } else if input.peek(syn::token::Brace) {
            Self::Object(input.parse()?)
        } else if input.peek(syn::token::Bracket) {
            Self::Array(input.parse()?)
        } else {
            input.span().to_syn_error("missing constant").to_err()?
        })
    }
}

impl Parse for ObjectValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut fields: Vec<ObjectFieldValue> = vec![];
        while !inner.is_empty() {
            if inner.peek(Token![,]) {
                inner.parse::<Token![,]>()?;
                continue;
            }
            let field: ObjectFieldValue = inner.parse()?;
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

impl Parse for ArrayValue {
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

impl Parse for ObjectFieldValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let value = input.parse()?;
        Ok(Self { name, value })
    }
}
