use std::collections::{HashMap, HashSet};

use convert_case::{Case, Casing};
use proc_macro2::Span;
use syn::{
    parse::{discouraged::Speculative, Parse, ParseBuffer, ParseStream},
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

use crate::{model::*, url_parser::parse_uri_and_update_api};

impl Parse for Client {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let whole_span = input.span();
        let mut client = Self {
            name: Ident::new("_", whole_span),
            options: None,
            hooks: None,
            apis: vec![],
            templates: HashMap::new(),
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
                let params = BracedConfig::parse(input, ident.span(), true, false, true)?;
                for field in params.fields.iter() {
                    field.requires_to_simple_type()?;
                    field.check_constant_expr_with_type()?;
                }
                client.options = Some(params);
            } else if let Some(templates) = DataTemplates::try_parse(input)? {
                for template in templates.templates.into_iter() {
                    if let Some(prev) = client.templates.get(&template.name) {
                        (template.span, prev.span)
                            .to_span()
                            .to_syn_error("duplicated object template")
                            .to_err()?;
                    } else {
                        client.templates.insert(template.name.clone(), template);
                    }
                }
            } else if let Some(template) = DataTemplate::try_parse(input)? {
                if let Some(prev) = client.templates.get(&template.name) {
                    (template.span, prev.span)
                        .to_span()
                        .to_syn_error("duplicated object template")
                        .to_err()?;
                } else {
                    client.templates.insert(template.name.clone(), template);
                }
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

        if let Some(options) = client.options.as_mut() {
            options.struct_name = client.name.with_suffix("Options");
        }

        for api in client.apis.iter_mut() {
            api.extend_templates(&client.templates)?;
        }

        client.resolve_object_type_names()?;

        let option_map = if let Some(options) = &client.options {
            options
                .fields
                .iter()
                .map(|f| (&f.field_name, f.typ.as_ref()))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };

        for (name, template) in client.templates.iter() {
            let mut extends = vec![];
            if template.check_recycle_ref(&client.templates, &mut extends)? {
                extends.insert(0, name);
                extends
                    .into_iter()
                    .map(|x| x.span())
                    .collect::<Vec<_>>()
                    .to_span()
                    .to_syn_error("cannot extend template back to self")
                    .to_err()?;
            }
        }

        for api in client.apis.iter_mut() {
            api.collect_and_check_vars(&option_map)?;
        }

        Ok(client)
    }
}

impl Client {
    fn resolve_object_type_names(&mut self) -> syn::Result<()> {
        for api in self.apis.iter_mut() {
            let prefix = api.name.to_ident_with_case(Case::UpperCamel);
            if let Some(data) = &mut api.request.data {
                data.data.resolve_types(prefix.with_suffix("RequestData"))?;
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

impl DataTemplates {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_as_ident("templates", false) {
            input.parse::<Token![:]>()?;
            let inner: ParseBuffer;
            syn::braced!(inner in input);
            let mut templates = DataTemplates { templates: vec![] };
            while !inner.is_empty() {
                if let Some(_) = inner.try_parse_comma() {
                    continue;
                }
                templates
                    .templates
                    .push(DataTemplate::parse(&inner, ident.span())?);
            }
            Ok(Some(templates))
        } else {
            Ok(None)
        }
    }
}

impl DataTemplate {
    fn parse(input: ParseStream, token_span: Span) -> syn::Result<Self> {
        let name = input.parse::<Ident>()?;
        let extend = if let Some(_colon) = input.try_parse_colon() {
            if input.peek(Ident) {
                Some(input.parse()?)
            } else {
                None
            }
        } else {
            None
        };
        let template = BracedConfig::parse(input, name.span(), true, true, true)?;
        Ok(Self {
            span: token_span,
            name,
            extend,
            fields: template,
        })
    }

    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(template) = input.try_parse_as_ident("template", false) {
            Ok(Some(Self::parse(input, template.span())?))
        } else {
            Ok(None)
        }
    }

    fn check_recycle_ref<'a>(
        &'a self,
        templates: &'a HashMap<Ident, Self>,
        path: &mut Vec<&'a Ident>,
    ) -> syn::Result<bool> {
        if path.contains(&&self.name) {
            return Ok(true);
        }
        if let Some(extend) = &self.extend {
            if let Some(next) = templates.get(extend) {
                path.push(extend);
                next.check_recycle_ref(templates, path)
            } else {
                extend.to_syn_error("no such template").to_err()
            }
        } else {
            Ok(false)
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
            let uri: ApiUri = url_input.parse()?;

            let request = ApiRequest::parse(input)?;
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
                uri,
                request,
                response,
                variables: vec![],
            }))
        } else {
            Ok(None)
        }
    }

    fn extend_templates(&mut self, templates: &HashMap<Ident, DataTemplate>) -> syn::Result<()> {
        if let Some(data) = &mut self.request.data {
            if let Some(extend) = &data.extend {
                data.data.extend_templates(extend, templates)?;
            }
        }
        Ok(())
    }

    fn collect_and_check_vars(
        &mut self,
        options: &HashMap<&Ident, Option<&Type>>,
    ) -> syn::Result<()> {
        self.uri.collect_vars(&mut self.variables)?;
        self.request.collect_vars(&mut self.variables)?;

        for var in self.variables.iter_mut() {
            if var.client_option {
                if let Some(opt_type) = options.get(&var.name) {
                    if let Some(typ) = &var.typ {
                        if let Some(opt_type) = opt_type {
                            if opt_type.ne(&typ) {
                                (typ.to_span(), opt_type.to_span())
                                    .to_span()
                                    .to_syn_error("unmatched type with client option")
                                    .to_err()?;
                            }
                        }
                    } else {
                        var.typ = opt_type.map(|t| t.pure());
                    }
                } else {
                    var.name.to_syn_error("no such option").to_err()?;
                }
            }
        }
        Ok(())
    }
}

impl Parse for ApiUri {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let uri: LitStr = input.parse()?;
        let mut x = ApiUri {
            uri_format: uri,
            uri_variables: vec![],
            schema: None,
            user: None,
            passwd: None,
            host: None,
            port: None,
            port_var: None,
            uri_path: None,
            uri_query: None,
            fragment: None,
        };
        parse_uri_and_update_api(&mut x)?;
        Ok(x)
    }
}

impl ApiUri {
    fn collect_vars(&self, variables: &mut Vec<Variable>) -> syn::Result<()> {
        for var in self.uri_variables.iter() {
            variables.collect(var, None)?;
        }
        if let Some(var) = &self.port_var {
            variables.collect(
                var,
                Some(&Type::Integer(IntegerType {
                    token: ("u16", var.name.span()).to_ident(),
                    limits: None,
                })),
            )?;
        }
        if let Some(path) = &self.uri_path {
            for seg in path.segments.iter() {
                if let ApiUriSeg::Var(var) = seg {
                    variables.collect(var, None)?;
                }
            }
        }
        if let Some(query) = &self.uri_query {
            for field in query.fields.iter() {
                if let Some(Expr::Variable(var)) = &field.expr {
                    variables.collect(var, None)?;
                }
            }
        }
        Ok(())
    }
}

impl ApiRequest {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut request = Self {
            brace,
            header: None,
            query: None,
            data: None,
            header_var: None,
            query_var: None,
        };

        while !inner.is_empty() {
            if let Some(_comma) = inner.try_parse_comma() {
                continue;
            }

            if let Some(data) = ApiRequestData::try_parse(&inner)? {
                if let Some(prev) = &request.data {
                    (data.data.token, prev.data.token)
                        .to_span()
                        .to_syn_error("duplicated json config")
                        .to_err()?;
                }
                request.data = Some(data);
            } else if let Some(query) = inner.try_parse_as_ident("query", false) {
                if let Some(prev) = request.query {
                    (query.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated query config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                request.query = Some(BracedConfig::parse(
                    &inner,
                    query.span(),
                    true,
                    false,
                    true,
                )?);
                request.query_var = Self::parse_var_part(&inner)?;
            } else if let Some(header) = inner.try_parse_as_ident("header", false) {
                if let Some(prev) = &request.header {
                    (header.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated header config")
                        .to_err()?;
                }
                input.try_parse_colon();
                request.header = Some(BracedConfig::parse(
                    &inner,
                    header.span(),
                    false,
                    false,
                    true,
                )?);
                request.header_var = Self::parse_var_part(&inner)?;
            } else {
                inner
                    .span()
                    .to_syn_error("unexpected config item")
                    .to_err()?;
            }
        }

        Ok(request)
    }
    fn parse_var_part(input: ParseStream) -> syn::Result<Option<Ident>> {
        Ok(if let Some(_) = input.try_parse_eq() {
            input.parse::<Token![$]>()?;
            Some(input.parse()?)
        } else {
            None
        })
    }

    fn collect_vars(&self, vars: &mut Vec<Variable>) -> syn::Result<()> {
        if let Some(header) = &self.header {
            header.collect_vars(vars, &self.header_var)?;
        }
        if let Some(query) = &self.query {
            query.collect_vars(vars, &self.query_var)?;
        }
        if let Some(data) = &self.data {
            data.data.collect_vars(vars, &data.data_var)?;
        }
        Ok(())
    }
}

impl ApiRequestData {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) =
            input.try_parse_one_of_idents(("json", "form", "urlencoded", "urlencode", "urlenc"))
        {
            let extend = if let Some(_colon) = input.try_parse_colon() {
                if input.peek(Ident) {
                    Some(input.parse()?)
                } else {
                    None
                }
            } else {
                None
            };
            let data = BracedConfig::parse(input, ident.span(), true, true, true)?;
            let data_var = ApiRequest::parse_var_part(input)?;
            Ok(Some(Self {
                extend,
                data_type: match ident.to_string().as_str() {
                    "json" => RequstDataType::Json(ident.span()),
                    "form" => RequstDataType::Form(ident.span()),
                    "urlencoded" | "urlencode" | "urlenc" => {
                        RequstDataType::Urlencoded(ident.span())
                    }
                    _ => {
                        unreachable!()
                    }
                },
                data,
                data_var,
            }))
        } else {
            Ok(None)
        }
    }
}

trait VariableCollector {
    fn collect(&mut self, var: &Variable, suggested_type: Option<&Type>) -> syn::Result<()>;
}

impl VariableCollector for Vec<Variable> {
    fn collect(&mut self, var: &Variable, suggested_type: Option<&Type>) -> syn::Result<()> {
        if var.client_option {
            return Ok(());
        }
        if let Some(old) = self.iter().find(|old| old.name.eq(&var.name)) {
            if let Some(old_type) = &old.typ {
                if let Some(typ) = suggested_type {
                    // compare type
                    if typ.ne(old_type) {
                        (typ.to_span(), old_type.to_span())
                            .to_span()
                            .to_syn_error("unmatched variable type")
                            .to_err()?;
                    }
                } else if !old_type.is_string() {
                    // check old type is string
                    old_type
                        .to_span()
                        .to_syn_error("expect string type for the variable")
                        .to_err()?;
                }
            } else if let Some(typ) = suggested_type {
                // check new type whether is string
                if !typ.is_string() {
                    (typ.to_span(), old.name.span())
                        .to_span()
                        .to_syn_error("expect string type (unmatched with previous variable type)")
                        .to_err()?;
                }
            }
        }
        let mut var = var.clone();
        if var.typ.is_none() {
            var.typ = suggested_type.map(|t| t.pure())
        }
        self.push(var);
        Ok(())
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
                response.json = Some(BracedConfig::parse(&inner, json.span(), true, true, false)?);
            } else if let Some(form) = inner.try_parse_as_ident("form", false) {
                if let Some(prev) = &response.form {
                    (form.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated form config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                response.form = Some(BracedConfig::parse(&inner, form.span(), true, true, false)?);
            } else if let Some(cookie) = inner.try_parse_as_ident("cookie", false) {
                if let Some(prev) = &response.cookie {
                    (cookie.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated cookie config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                response.cookie = Some(BracedConfig::parse(
                    &inner,
                    cookie.span(),
                    false,
                    true,
                    false,
                )?);
            } else if let Some(header) = inner.try_parse_as_ident("header", false) {
                if let Some(prev) = &response.header {
                    (header.span(), prev.token)
                        .to_span()
                        .to_syn_error("duplicated header config")
                        .to_err()?;
                }
                inner.try_parse_colon();
                response.header = Some(BracedConfig::parse(
                    &inner,
                    header.span(),
                    false,
                    true,
                    false,
                )?);
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

impl BracedConfig {
    fn parse(
        input: ParseStream,
        token: Span,
        parse_type: bool,
        parse_alias: bool,
        parse_assignment: bool,
    ) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut fields: Vec<Field> = vec![];
        let mut removed_fields = HashSet::new();
        while !inner.is_empty() {
            if let Some(_) = inner.try_parse_comma() {
                continue;
            }
            let mut removal = false;
            if inner.peek(Token![-]) {
                inner.parse::<Token![-]>()?;
                removal = true;
            }
            if removal {
                let fork = inner.fork();
                let result = Field::parse(&fork, parse_type, parse_alias, parse_assignment);
                if result.is_err() {
                    removed_fields.insert(inner.parse_as_lit_str()?);
                } else {
                    let field: Field = result?;
                    if let Some(prev) = fields.iter().find(|f| f.field_name.eq(&field.field_name)) {
                        (field.name.span(), prev.name.span())
                            .to_span()
                            .to_syn_error("duplicated field")
                            .to_err()?;
                    }
                    removed_fields.insert(field.name);
                    inner.advance_to(&fork);
                }
            } else {
                let field: Field = Field::parse(&inner, parse_type, parse_alias, parse_assignment)?;
                if let Some(prev) = fields.iter().find(|f| f.field_name.eq(&field.field_name)) {
                    (field.name.span(), prev.name.span())
                        .to_span()
                        .to_syn_error("duplicated field")
                        .to_err()?;
                }
                fields.push(field);
            }
        }
        Ok(Self {
            token,
            struct_name: ("_", token).to_ident(),
            brace,
            fields,
            removed_fields,
        })
    }

    fn resolve_types(&mut self, name: Ident) -> syn::Result<()> {
        let prefix = name.to_string();
        self.struct_name = name.clone();
        for field in self.fields.iter_mut() {
            field.resolve_field_type(&prefix)?;
        }
        Ok(())
    }

    fn extend_templates(
        &mut self,
        extend: &Ident,
        templates: &HashMap<Ident, DataTemplate>,
    ) -> syn::Result<()> {
        if let Some(template) = templates.get(extend) {
            for field in template.fields.fields.iter() {
                if self.removed_fields.contains(&field.name) {
                    continue;
                }
                if self
                    .fields
                    .iter()
                    .find(|f| f.name.eq(&field.name))
                    .is_some()
                {
                    continue;
                }
                self.fields.push(field.clone());
            }
            if let Some(extend) = &template.extend {
                self.extend_templates(extend, templates)?;
            }
        }

        Ok(())
    }

    fn collect_vars<C: VariableCollector>(
        &self,
        vars: &mut C,
        outer_varname: &Option<Ident>,
    ) -> syn::Result<()> {
        let mut spans = vec![];
        for f in self.fields.iter() {
            if let Some(x) = &f.expr {
                x.collect_vars(vars, f.typ.as_ref())?;
            } else {
                if let Some(typ) = &f.typ {
                    match typ {
                        Type::Constant(_) => {}
                        Type::Object(obj) => {
                            if !obj.get_unassigned_fields(&mut spans) {
                                spans.push(f.name.span());
                            }
                        }
                        _ => {
                            spans.push(f.name.span());
                        }
                    }
                }
            }
            if let Some(Type::Object(obj)) = f.typ.as_ref() {
                obj.collect_vars(vars)?;
            }
        }
        if !spans.is_empty() {
            if outer_varname.is_none() {
                spans
                    .to_span()
                    .to_syn_error("missing variable to init fields")
                    .to_err()?;
            }
        }
        Ok(())
    }
}

impl Field {
    fn resolve_field_type(&mut self, prefix: &str) -> syn::Result<()> {
        if let Some(typ) = self.typ.as_mut() {
            match typ {
                Type::Object(obj) => {
                    obj.resolve_type_name(&self.field_name, prefix, false)?;
                }
                Type::JsonText(JsonStringType { typ, .. }) => {
                    if let Type::Object(obj) = typ.as_mut() {
                        obj.resolve_type_name(&self.field_name, prefix, false)?;
                    }
                }
                Type::List(ListType { element_type, .. }) => {
                    if let Type::Object(obj) = element_type.as_mut() {
                        obj.resolve_type_name(&self.field_name, prefix, true)?;
                    }
                }
                Type::Datetime(DateTimeType { format, .. }) => {
                    if let Some(format) = format {
                        format.mod_name = self
                            .field_name
                            .to_ident_with_case(Case::Snake)
                            .with_prefix("_")
                            .with_prefix(prefix.to_case(Case::Snake))
                            .with_suffix("_formatter");
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl ObjectType {
    fn resolve_type_name(
        &mut self,
        field_name: &Ident,
        prefix: &str,
        _is_list_item: bool,
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

    fn collect_vars<C: VariableCollector>(&self, vars: &mut C) -> syn::Result<()> {
        for f in self.fields.iter() {
            if let Some(Expr::Variable(var)) = &f.expr {
                vars.collect(var, f.typ.as_ref())?;
            }
            if let Some(Type::Object(obj)) = f.typ.as_ref() {
                obj.collect_vars(vars)?;
            }
        }
        Ok(())
    }

    fn get_unassigned_fields(&self, spans: &mut Vec<Span>) -> bool {
        let mut has_unsignned = false;
        for field in &self.fields {
            if field.expr.is_none() {
                if let Some(typ) = field.typ.as_ref() {
                    match typ {
                        Type::Constant(_) => {}
                        Type::Object(obj) => {
                            obj.get_unassigned_fields(spans);
                        }
                        _ => {
                            has_unsignned = true;
                            spans.push(field.name.span());
                        }
                    }
                }
            }
        }
        has_unsignned
    }
}

impl Parse for ObjectType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ParseBuffer;
        let brace = syn::braced!(inner in input);
        let mut fields: Vec<Field> = vec![];
        while !inner.is_empty() {
            if let Some(_) = inner.try_parse_comma() {
                continue;
            }
            let field = Field::parse(&inner, true, true, false)?;
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
impl Type {
    fn peek(input: ParseStream) -> syn::Result<()> {
        if !input.peek(syn::token::Brace) {
            input.parse::<Token![:]>()?;
        }
        Ok(())
    }

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
        } else if let Some(datetime) = DateTimeType::try_parse(input)? {
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

    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut typ = Type::parse_basic(input)?;
        if input.peek(syn::token::Bracket) {
            let inner: ParseBuffer;
            let bracket = syn::bracketed!(inner in input);
            if inner.is_empty() {
                typ = Type::List(ListType {
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

impl ToSpan for Type {
    fn to_span(&self) -> Span {
        match self {
            Self::Constant(c) => c.span(),
            Self::String(s) => s.span,
            Self::Bool(s) => *s,
            Self::Integer(i) => i.token.span(),
            Self::Float(f) => f.token.span(),
            Self::Object(o) => o.brace.span.close(),
            Self::Datetime(d) => d.span,
            Self::JsonText(j) => j.span,
            Self::Map(s) => *s,
            Self::List(l) => (l.element_type.to_span(), l.bracket.span.close()).to_span(),
        }
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

impl TryParse for JsonStringType {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(json) = input.try_parse_one_of_idents(("json", "json_string")) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            Ok(Some(Self {
                paren,
                span: json.span(),
                typ: Box::new(Type::parse(&inner)?),
            }))
        } else {
            Ok(None)
        }
    }
}

impl DateTimeFormat {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if input.peek(syn::token::Paren) {
            let inner: ParseBuffer;
            let paren = syn::parenthesized!(inner in input);
            let format = inner.parse::<LitStr>()?;
            Ok(Some(Self {
                paren,
                mod_name: ("_", format.span()).to_ident(),
                format,
            }))
        } else {
            Ok(None)
        }
    }
}

impl DateTimeType {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>> {
        if let Some(ident) = input.try_parse_one_of_idents(("datetime", "date")) {
            Ok(Some(Self {
                span: ident.span(),
                format: DateTimeFormat::try_parse(input)?,
            }))
        } else {
            Ok(None)
        }
    }
}

impl Field {
    fn parse(
        input: ParseStream,
        parse_type: bool,
        parse_alias: bool,
        parse_assignment: bool,
    ) -> syn::Result<Self> {
        let name = input.parse_as_lit_str()?;
        let optional = input.try_parse_question();

        let typ = if parse_type {
            Type::peek(input)?;
            Some(Type::parse(input)?)
        } else {
            None
        };
        let alias = if parse_alias {
            if input.peek(Token![->]) {
                input.parse::<Token![->]>()?;
                Some(input.parse::<Ident>()?)
            } else {
                None
            }
        } else {
            None
        };
        let expr = if parse_assignment {
            if let Some(_eq) = input.try_parse_eq() {
                Some(Expr::parse(input)?)
            } else {
                None
            }
        } else {
            None
        };
        let mut field_name = if let Some(alias) = &alias {
            if alias.is_keyword() {
                alias
                    .to_syn_error("alias name is reserved for rust language")
                    .to_err()?;
            }
            alias.clone()
        } else {
            name.to_ident_with_case(Case::Snake)
        };

        match field_name.to_string().as_str() {
            "type" => {
                field_name = ("typ", field_name.span()).to_ident();
            }
            x @ _ => {
                if is_keyword(x) {
                    field_name =
                        (format!("{}_", field_name.to_string()), field_name.span()).to_ident();
                }
            }
        }

        let mut default = None;
        if let Some(Type::Constant(c)) = typ.as_ref() {
            default = Some(c.to_value());
        } else if let Some(x) = &expr {
            if let Expr::Constant(c) = x {
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

impl Field {
    fn requires_to_simple_type(&self) -> syn::Result<()> {
        if let Some(t) = self.typ.as_ref() {
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
        if let (Some(t), Some(x)) = (self.typ.as_ref(), self.expr.as_ref()) {
            let okay = is_type_and_value_match(t, x);
            if !okay {
                (t.to_span(), x.to_span())
                    .to_span()
                    .to_syn_error("unmatch type with value")
                    .to_err()?;
            }
        }
        Ok(())
    }
}

fn is_type_and_constant_match(t: &Type, c: &Constant) -> bool {
    match (t, c) {
        (Type::String(_), Constant::String(_)) => true,
        (Type::Integer(_), Constant::Int(_)) => true,
        (Type::Float(_), Constant::Float(_)) => true,
        (Type::Bool(_), Constant::Bool(_)) => true,
        _ => false,
    }
}

fn is_type_and_value_match(t: &Type, x: &Expr) -> bool {
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
        } else if let Some(ident) = input.try_parse_as_ident("default", false) {
            let _paren: ParseBuffer;
            let p = syn::parenthesized!(_paren in input);
            Self::Default((ident.span(), p.span.close()).to_span())
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
            Expr::Default(span) => *span,
        }
    }
}

impl Expr {
    fn collect_vars<C: VariableCollector>(
        &self,
        vars: &mut C,
        suggested_type: Option<&Type>,
    ) -> syn::Result<()> {
        match self {
            Expr::Variable(var) => {
                vars.collect(var, suggested_type)?;
            }
            Expr::Datetime(call) => {
                vars.collect(
                    &call.variable,
                    Some(&Type::Datetime(DateTimeType {
                        span: call.variable.name.span(),
                        format: None,
                    })),
                )?;
            }
            Expr::Json(call) => {
                vars.collect(&call.variable, Some(&Type::Map(call.variable.name.span())))?;
            }
            Expr::Timestamp(call) => {
                vars.collect(
                    &call.variable,
                    Some(&Type::Datetime(DateTimeType {
                        span: call.variable.name.span(),
                        format: None,
                    })),
                )?;
            }
            Expr::Format(call) => {
                if let Some(args) = &call.args {
                    for arg in args {
                        arg.collect_vars::<C>(
                            vars,
                            Some(&Type::String(StringType {
                                span: arg.to_span(),
                            })),
                        )?;
                    }
                }
            }
            Expr::Join(call) => {
                vars.collect(
                    &call.variable,
                    Some(&Type::String(StringType {
                        span: call.to_span(),
                    })),
                )?;
            }
            Expr::Or(or) => vars.collect(&or.variable, suggested_type)?,
            _ => {}
        }
        Ok(())
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
            Constant::Object(c) => syn::Expr::Call(syn::ExprCall {
                attrs: vec![],
                func: Box::new(syn::Path::from_idents(("Default", "default", c.span)).to_expr()),
                paren_token: Paren(c.span),
                args: Punctuated::new(),
            }),
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
        let client_option = input.try_parse_dollar();
        let dollar = client_option
            .as_ref()
            .map(|d| (dollar.span(), d.span()).to_span())
            .unwrap_or(dollar.span());
        let client_option = client_option.is_some();
        let name = input.parse()?;
        Ok(if input.peek(Token![:]) {
            Type::peek(input)?;
            Self {
                dollar,
                name,
                typ: Some(Type::parse(input)?),
                client_option,
            }
        } else {
            Self {
                dollar,
                name,
                typ: None,
                client_option,
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

pub trait IsKeyword {
    fn is_keyword(&self) -> bool;
}

impl IsKeyword for Ident {
    fn is_keyword(&self) -> bool {
        is_keyword(&self.to_string())
    }
}

impl IsKeyword for LitStr {
    fn is_keyword(&self) -> bool {
        is_keyword(&self.value())
    }
}

fn is_keyword(ident: &str) -> bool {
    match ident {
        "type" | "abstract" | "as" | "async" | "auto" | "await" | "become" | "box" | "break"
        | "const" | "continue" | "crate" | "default" | "do" | "dyn" | "else" | "enum"
        | "extern" | "final" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "macro"
        | "match" | "mod" | "move" | "mut" | "override" | "priv" | "pub" | "ref" | "return"
        | "static" | "struct" | "super" | "trait" | "try" | "typeof" | "union" | "unsafe"
        | "unsized" | "use" | "virtual" | "where" | "while" | "yield" => true,
        _ => false,
    }
}
