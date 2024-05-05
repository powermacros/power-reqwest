use crate::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{spanned::Spanned, Ident, Path};
use syn_prelude::{PathHelpers, ToIdent, ToLitStr};

fn make_chrono_datetime_type(span: Span) -> syn::Type {
    let utc = syn::Path::from_idents(("chrono", "Utc", span));
    let mut path = syn::Path::from_idents(("chrono", "DateTime", span));
    path.push_arg(1, utc.to_type());
    path.to_type()
}

fn make_serde_json_map(span: Span) -> syn::Type {
    let mut map = syn::Path::from_idents(("serde_json", "Map", span));
    map.push_ident_arg(1, ("String", span).to_ident());
    let value = syn::Path::from_idents(("serde_json", "Value", span));
    map.push_arg(1, value.to_type());
    map.to_type()
}

impl ToTokens for Client {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name, apis, .. } = self;
        let param_types = self
            .options
            .as_ref()
            .map(|params| params.gen_obj_structs())
            .unwrap_or(vec![]);

        let options_arg = self
            .options
            .as_ref()
            .map(|BracedConfig { struct_name, .. }| quote! (options: #struct_name));
        let options_field = options_arg.as_ref().map(|arg| quote!(#arg,));
        let options_assign = options_arg.as_ref().map(|_| quote!(options,));

        let api_decls = apis.iter().map(|api| api.to_token_stream(self));

        tokens.append_all(quote! {
            #(#param_types)*

            pub struct #name {
                #options_field
                inner: reqwest::Client,
            }

            impl #name {
                pub fn new(#options_arg) -> Self {
                    Self {
                        #options_assign
                        inner: reqwest::Client::new(),
                    }
                }
            }

            #(#api_decls)*
        })
    }
}

impl Type {
    fn to_type(&self) -> syn::Type {
        match self {
            Self::Constant(c) => c.infer_type(),
            Self::String(s) => Path::from_ident(("String", s.span)).to_type(),
            Self::Bool(span) => Path::from_ident(("bool", *span)).to_type(),
            Self::Integer(i) => {
                if i.token.eq("uint") {
                    Path::from_ident(("u64", i.token.span())).to_type()
                } else if i.token.eq("int") || i.token.eq("integer") {
                    Path::from_ident(("i64", i.token.span())).to_type()
                } else {
                    Path::from_ident(&i.token).to_type()
                }
            }
            Self::Float(f) => {
                if f.token.eq("float") {
                    Path::from_ident(("f64", f.token.span())).to_type()
                } else {
                    Path::from_ident(&f.token).to_type()
                }
            }
            Self::Object(o) => syn::Path::from_ident(&o.struct_name).to_type(),
            Self::Datetime(d) => make_chrono_datetime_type(d.span),
            Self::JsonText(j) => Path::from_ident(("String", j.span)).to_type(),
            Self::Map(span) => make_serde_json_map(*span),
            Self::List(l) => {
                let mut path = Path::from_ident(("Vec", l.bracket.span.close()));
                path.push_arg(0, l.element_type.to_type());
                path.to_type()
            }
        }
    }
}

impl Constant {
    fn infer_type(&self) -> syn::Type {
        match self {
            Self::String(value) => Path::from_ident(("String", value.span())).to_type(),
            Self::Bool(value) => Path::from_ident(("bool", value.span())).to_type(),
            Self::Int(value) => Path::from_ident(("u64", value.span())).to_type(),
            Self::Float(value) => Path::from_ident(("f64", value.span())).to_type(),
            Self::Object(_obj) => {
                todo!("not support object constant yet")
            }
            Self::Array(arr) => {
                let el = arr.elements.first().unwrap();
                let mut path = Path::from_ident(("Vec", arr.span));
                path.push_arg(0, el.infer_type());
                path.to_type()
            }
        }
    }
}

impl ToTokens for Constant {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(match self {
            Self::String(s) => quote!(#s.to_owned()),
            Self::Bool(b) => quote!(#b),
            Self::Int(i) => quote!(#i),
            Self::Float(f) => quote!(#f),
            Self::Object(_) => todo!(),
            Self::Array(ConstantArray { elements, .. }) => quote!(#(#elements),*),
        })
    }
}

impl Api {
    fn to_token_stream(&self, client: &Client) -> TokenStream {
        let Client {
            name: client_name, ..
        } = client;
        let Self {
            name,
            request,
            response,
            variables,
            ..
        } = self;

        let mut types = if let Some(data) = &request.data {
            data.data.gen_obj_structs()
        } else {
            vec![]
        };
        if let Some(queries) = &request.query {
            types.extend(queries.gen_obj_structs());
        }
        if let Some(headers) = &request.header {
            types.extend(headers.gen_obj_structs());
        }

        if let Some(response) = response {
            if let Some(json) = &response.json {
                types.extend(json.gen_obj_structs());
            } else if let Some(form) = &response.form {
                types.extend(form.gen_obj_structs());
            }
            if let Some(cookies) = &response.cookie {
                types.extend(cookies.gen_obj_structs());
            }
            if let Some(headers) = &response.header {
                types.extend(headers.gen_obj_structs());
            }
        }

        let args = variables.iter().map(|Variable { name, typ, .. }| {
            if let Some(typ) = &typ {
                let typ = typ.to_type();
                quote!(#name: #typ)
            } else {
                quote!(#name: String)
            }
        });

        quote! {
            #(#types)*

            impl #client_name {
                pub async fn #name(#(#args),*) {

                }
            }
        }
    }
}

fn make_object_struct(name: &Ident, fields: &Vec<Field>) -> TokenStream {
    let fields_in_struct = fields.iter().map(
        |Field {
             name,
             field_name,
             optional,
             typ,
             ..
         }| {
            let mut field_type = if let Some(typ) = typ {
                typ.to_type()
            } else {
                syn::Path::from_ident(("String", name.span())).to_type()
            };
            if optional.is_some() {
                let mut option = syn::Path::from_ident(("Option", field_type.span()));
                option.push_arg(0, field_type);
                field_type = option.to_type()
            }

            let mut serde_options = None;
            if !name.value().eq(&field_name.to_string()) {
                serde_options = Some(vec![quote! {rename = #name}])
            }

            if let Some(Type::Datetime(DateTimeType {
                format: Some(DateTimeFormat { mod_name, .. }),
                ..
            })) = typ
            {
                let formatter = mod_name.to_lit_str();
                if let Some(options) = serde_options.as_mut() {
                    options.push(quote! {with = #formatter})
                } else {
                    serde_options = Some(vec![quote! {with = #formatter}]);
                }
            };
            let serde = serde_options.map(|opts| quote! {#[serde(#(#opts),*)]});

            quote! {
                #serde
                pub #field_name: #field_type
            }
        },
    );

    let field_inits = fields.iter().map(
        |Field {
             field_name,
             default,
             ..
         }| {
            let default = default
                .as_ref()
                .map(|x| x.to_token_stream())
                .unwrap_or(quote!(Default::default()));
            quote! {
                #field_name: #default
            }
        },
    );

    let serde_formatters = fields.iter().filter_map(|Field { typ, .. }| {
        if let Some(Type::Datetime(DateTimeType {
            format: Some(format),
            ..
        })) = typ
        {
            Some(format.gen_serde_formatter())
        } else {
            None
        }
    });

    quote! {
        #[derive(serde::Serialize, serde::Deserialize)]
        pub struct #name {
            #(#fields_in_struct),*
        }
        impl Default for #name {
            fn default() -> Self {
                Self {
                    #(#field_inits),*
                }
            }
        }
        #(#serde_formatters)*
    }
}

impl BracedConfig {
    fn gen_obj_structs(&self) -> Vec<TokenStream> {
        let mut types = self
            .fields
            .iter()
            .filter_map(|f| {
                if let Some(typ) = &f.typ {
                    typ.gen_obj_structs()
                } else {
                    None
                }
            })
            .flatten()
            .collect::<Vec<_>>();
        types.insert(0, make_object_struct(&self.struct_name, &self.fields));

        types
    }
}

impl Type {
    fn gen_obj_structs(&self) -> Option<Vec<TokenStream>> {
        match self {
            Self::Object(obj) => Some(obj.gen_obj_structs()),
            Self::JsonText(JsonStringType { typ, .. }) => {
                if let Type::Object(obj) = typ.as_ref() {
                    Some(obj.gen_obj_structs())
                } else {
                    None
                }
            }
            Self::List(ListType { element_type, .. }) => element_type.gen_obj_structs(),
            _ => None,
        }
    }
}

impl ObjectType {
    fn gen_obj_structs(&self) -> Vec<TokenStream> {
        let mut types = self
            .fields
            .iter()
            .filter_map(|f| f.typ.as_ref().map(|t| t.gen_obj_structs()))
            .flatten()
            .flatten()
            .collect::<Vec<_>>();
        types.insert(0, make_object_struct(&self.struct_name, &self.fields));
        types
    }
}

impl DateTimeFormat {
    fn gen_serde_formatter(&self) -> TokenStream {
        let Self {
            format, mod_name, ..
        } = self;
        quote! {
            mod #mod_name {
                use chrono::{DateTime, Utc, NaiveDateTime};
                use serde::{self, Deserialize, Serializer, Deserializer};


                pub fn serialize<S>(
                    date: &DateTime<Utc>,
                    serializer: S,
                ) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    let s = format!("{}", date.format(#format));
                    serializer.serialize_str(&s)
                }

                pub fn deserialize<'de, D>(
                    deserializer: D,
                ) -> Result<DateTime<Utc>, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    let s = String::deserialize(deserializer)?;
                    let dt = NaiveDateTime::parse_from_str(&s, #format).map_err(serde::de::Error::custom)?;
                    Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                }
            }
        }
    }
}
