use proc_macro2::Span;
use syn::{Ident, LitBool, LitFloat, LitInt, LitStr};

pub struct Client {
    pub name: Ident,
    pub config: Option<Vec<DataField>>,
    pub common: Option<Common>,
    pub auth: Option<Auth>,
    pub signing: Option<Signing>,
    pub apis: Vec<Api>,
}

pub struct Common {
    pub(crate) span: Span,
    pub unwrap_response: Option<syn::Path>,
}

pub struct Signing {
    pub(crate) span: Span,
    pub sign_fn: Option<syn::Path>,
}

pub struct Auth {
    pub(crate) span: Span,
    pub url: LitStr,
}

pub struct Api {
    pub name: LitStr,
    pub method: Ident,
    pub url: LitStr,
    pub request: Option<ApiRequest>,
    pub response: Vec<DataField>,
}

pub struct ApiRequest {
    pub header: Option<Vec<ApiHeader>>,
    pub data: Option<Vec<DataField>>,
}

pub struct ApiHeader {
    pub name: LitStr,
    pub value: Value,
}

pub struct DataField {
    pub name: Ident,
    pub typ: DataType,
    pub optional: Option<Span>,
    pub value: Option<Value>,
}

pub enum DataType {
    String(Span),
    Bool(Span),
    Int(Span),
    Uint(Span),
    Float(Span),
    Object(ObjectType),
    List(Box<DataType>),
}

pub struct ObjectType {
    pub fields: Vec<ObjectFieldType>,
}

pub struct ObjectFieldType {
    pub name: Ident,
    pub value_type: DataType,
    pub value: Option<Value>,
}

pub enum Value {
    Var(Ident),
    String(LitStr),
    Bool(LitBool),
    Int(LitInt),
    Float(LitFloat),
    Object(ObjectValue),
    Array(ArrayValue),
}

pub struct ObjectValue {
    pub span: Span,
    pub fields: Vec<ObjectFieldValue>,
}

pub struct ObjectFieldValue {
    pub name: Ident,
    pub value: Value,
}

pub struct ArrayValue {
    pub(crate) span: Span,
    pub elements: Vec<Value>,
}
