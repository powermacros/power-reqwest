use crate::*;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{spanned::Spanned, Ident, LitInt, Path};
use syn_prelude::{PathHelpers, ToIdent, WithPrefix};

impl ToTokens for Client {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            name,
            options_name,
            apis,
            ..
        } = self;
        let params = self
            .options
            .as_ref()
            .map(|params| params.to_client_params_tokens(options_name));

        let options_arg = options_name.as_ref().map(|typ| quote! (options: #typ));
        let options_field = options_arg.as_ref().map(|arg| quote!(#arg,));
        let options_assign = options_arg.as_ref().map(|_| quote!(options,));

        let api_decls = apis.iter().map(|api| api.to_token_stream(self));

        tokens.append_all(quote! {
            #params

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

impl ClientParams {
    fn to_client_params_tokens(&self, options_name: &Option<Ident>) -> TokenStream {
        let fields = self.fields.iter().map(
            |Field {
                 field_name,
                 optional,
                 typ,
                 ..
             }| {
                let mut typ = typ.to_type();
                if optional.is_some() {
                    let mut option = syn::Path::from_ident(("Option", typ.span()));
                    option.push_arg(0, typ);
                    typ = option.to_type();
                }
                quote! {
                    #field_name: #typ
                }
            },
        );

        let field_default_inits = self.fields.iter().map(
            |Field {
                 field_name,
                 optional,
                 typ,
                 expr,
                 ..
             }| {
                let mut init_value = if let Some((_, Expr::Constant(constant))) = expr {
                    constant.to_token_stream()
                } else {
                    typ.default_expr()
                };
                if optional.is_some() {
                    init_value = quote!(Some(#init_value));
                }
                quote! {#field_name: #init_value}
            },
        );

        let field_methods = self.fields.iter().map(
            |Field {
                 field_name,
                 typ,
                 optional,
                 ..
             }| {
                let set_fn = field_name.with_prefix("set_");
                let arg_typ = typ.to_type();
                let value = if optional.is_some() {
                    quote! {Some(value)}
                } else {
                    quote!(value)
                };
                quote! {
                    pub fn #set_fn(&mut self, value: #arg_typ) -> &mut Self {
                        self.#field_name = #value;
                        self
                    }
                }
            },
        );

        quote! {
            #[derive(Debug, Clone)]
            pub struct #options_name {
                #(#fields),*
            }

            impl Default for #options_name {
                fn default() -> Self {
                    Self{
                        #(#field_default_inits),*
                    }
                }
            }

            impl #options_name {
                #(
                    #field_methods
                )*
            }
        }
    }
}

impl<A: AsFieldAlias, X: TryParse> Type<A, X> {
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
            Self::DatetimeString(d) => {
                let utc = syn::Path::from_idents(("chrono", "Utc", d.span));
                let mut path = syn::Path::from_idents(("chrono", "DateTime", d.span));
                path.push_arg(1, utc.to_type());
                path.to_type()
            }
            Self::JsonText(j) => Path::from_ident(("String", j.span)).to_type(),
            Self::Map(_) => todo!(),
            Self::List(l) => {
                let mut path = Path::from_ident(("Vec", l.bracket.span.close()));
                path.push_arg(0, l.element_type.to_type());
                path.to_type()
            }
        }
    }

    fn default_expr(&self) -> TokenStream {
        match self {
            Self::Constant(constant) => quote!(#constant),
            Self::String(_) => quote!("".to_owned()),
            Self::Bool(_) => quote!("false"),
            Self::Integer(i) => {
                if i.token.eq("uint") {
                    quote!(0u64)
                } else if i.token.eq("int") || i.token.eq("integer") {
                    quote!(0i64)
                } else {
                    LitInt::new(&format!("0{}", i.token.to_string()), i.token.span())
                        .to_token_stream()
                }
            }
            Self::Float(f) => {
                if f.token.eq("f32") {
                    quote!(0f32)
                } else {
                    quote!(0f64)
                }
            }
            Self::Object(_) => todo!(),
            Self::DatetimeString(_) => quote!("".to_owned()),
            Self::JsonText(_) => quote!("".to_owned()),
            Self::Map(_) => todo!(),
            Self::List(_) => quote!(vec![]),
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
            Self::Object(_value) => todo!(),
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
            ..
        } = self;

        let mut types = if let Some(json) = &request.json {
            json.to_types()
        } else if let Some(form) = &request.form {
            form.to_types()
        } else {
            vec![]
        };
        if let Some(queries) = &request.query {
            types.extend(queries.to_types());
        }
        if let Some(headers) = &request.header {
            types.extend(headers.to_types());
        }

        if let Some(response) = response {
            if let Some(json) = &response.json {
                types.extend(json.to_types());
            } else if let Some(form) = &response.form {
                types.extend(form.to_types());
            }
            if let Some(cookies) = &response.cookie {
                types.extend(cookies.to_types());
            }
            if let Some(headers) = &response.header {
                types.extend(headers.to_types());
            }
        }

        quote! {
            #(#types)*

            impl #client_name {
                pub async fn #name() {

                }
            }
        }
    }
}

fn make_object_struct<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment>(
    name: &Ident,
    fields: &Vec<Field<T, A, X>>,
) -> TokenStream {
    let fields_in_struct = fields.iter().map(
        |Field {
             name,
             field_name,
             optional,
             typ,
             ..
         }| {
            let mut typ = if let Some(typ) = typ.as_type() {
                typ.to_type()
            } else {
                syn::Path::from_ident(("String", name.span())).to_type()
            };
            if optional.is_some() {
                let mut option = syn::Path::from_ident(("Option", typ.span()));
                option.push_arg(0, typ);
                typ = option.to_type()
            }
            quote! {
                #field_name: #typ
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

    quote! {
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
    }
}

impl<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment> BracedConfig<T, A, X> {
    fn to_types(&self) -> Vec<TokenStream> {
        let mut types = self
            .fields
            .iter()
            .filter_map(|f| {
                if let Some(Type::Object(obj)) = f.typ.as_type() {
                    Some(obj.to_types())
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

impl<A: AsFieldAlias, X: AsFieldAssignment> ObjectType<A, X> {
    fn to_types(&self) -> Vec<TokenStream> {
        let mut types = self
            .fields
            .iter()
            .filter_map(|f| {
                if let Type::Object(obj) = &f.typ {
                    Some(obj.to_types())
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
