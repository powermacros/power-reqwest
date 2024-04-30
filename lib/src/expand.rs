use crate::*;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{spanned::Spanned, Ident, Path};
use syn_prelude::{PathHelpers, ToIdent};

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
            Self::Map(span) => {
                let mut map = syn::Path::from_idents(("serde_json", "Map", *span));
                map.push_ident_arg(1, ("String", *span).to_ident());
                let value = syn::Path::from_idents(("serde_json", "Value", *span));
                map.push_arg(1, value.to_type());
                map.to_type()
            }
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
            ..
        } = self;

        let mut types = if let Some(json) = &request.json {
            json.gen_obj_structs()
        } else if let Some(form) = &request.form {
            form.gen_obj_structs()
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
            let mut field_type = if let Some(typ) = typ.as_type() {
                typ.to_type()
            } else {
                syn::Path::from_ident(("String", name.span())).to_type()
            };
            if optional.is_some() {
                let mut option = syn::Path::from_ident(("Option", field_type.span()));
                option.push_arg(0, field_type);
                field_type = option.to_type()
            }
            let serde = if !name.value().eq(&field_name.to_string()) {
                Some(quote! {#[serde(rename = #name)]})
            } else if let Some(Type::DatetimeString(DateTimeStringType { formatter, .. })) =
                typ.as_type()
            {
                // Some(quote! {#[serde(with = #formatter)]})
                None
            } else {
                None
            };
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
    }
}

impl<T: AsFieldType<A, X>, A: AsFieldAlias, X: AsFieldAssignment> BracedConfig<T, A, X> {
    fn gen_obj_structs(&self) -> Vec<TokenStream> {
        let mut types = self
            .fields
            .iter()
            .filter_map(|f| {
                if let Some(typ) = f.typ.as_type() {
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

impl<A: AsFieldAlias, X: AsFieldAssignment> Type<A, X> {
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

impl<A: AsFieldAlias, X: AsFieldAssignment> ObjectType<A, X> {
    fn gen_obj_structs(&self) -> Vec<TokenStream> {
        let mut types = self
            .fields
            .iter()
            .filter_map(|f| f.typ.gen_obj_structs())
            .flatten()
            .collect::<Vec<_>>();
        types.insert(0, make_object_struct(&self.struct_name, &self.fields));
        types
    }
}
