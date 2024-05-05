use std::collections::{HashMap, HashSet};

use proc_macro2::Span;
use syn::{
    parse::ParseStream,
    punctuated::Punctuated,
    token::{Brace, Bracket, Paren},
    ExprRange, Ident, LitBool, LitFloat, LitInt, LitStr, Token,
};

#[derive(Clone, Debug)]
pub struct Client {
    pub name: Ident,
    pub options: Option<BracedConfig>,
    pub option_map: HashMap<Ident, Field>,
    pub hooks: Option<Hooks>,
    pub apis: Vec<Api>,
    pub templates: HashMap<Ident, DataTemplate>,
}

#[derive(Clone, Debug)]
pub struct Hooks {
    pub(crate) span: Span,
    pub on_submit: Option<syn::Path>,
}

#[derive(Clone, Debug)]
pub struct DataTemplates {
    pub templates: Vec<DataTemplate>,
}

#[derive(Clone, Debug)]
pub struct DataTemplate {
    pub(crate) span: Span,
    pub name: Ident,
    pub fields: BracedConfig,
}

#[derive(Clone, Debug)]
pub struct Api {
    pub name: Ident,
    pub method: Ident,
    pub uri: ApiUri,
    pub paren: Paren,
    pub request: ApiRequest,
    pub response: Option<ApiResponse>,
    pub variables: Vec<Variable>,
}

#[derive(Clone, Debug)]
pub struct ApiUri {
    pub uri_format: LitStr,
    pub uri_variables: Vec<Variable>,
    pub schema: Option<LitStr>,
    pub user: Option<LitStr>,
    pub passwd: Option<LitStr>,
    pub host: Option<LitStr>,
    pub port: Option<LitInt>,
    pub port_var: Option<Variable>,
    pub uri_path: Option<ApiUriPath>,
    pub uri_query: Option<ApiUriQuery>,
    pub fragment: Option<LitStr>,
}

#[derive(Clone, Debug)]
pub enum ApiPortOrVar {
    Port(LitInt),
    Var(Variable),
}

#[derive(Clone, Debug)]
pub struct ApiUriPath {
    pub last_slash: bool,
    pub segments: Vec<ApiUriSeg>,
}

#[derive(Clone, Debug)]
pub enum ApiUriSeg {
    Static(LitStr),
    Var(Variable),
}

#[derive(Clone, Debug)]
pub struct ApiUriQuery {
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct ApiRequest {
    pub brace: Brace,
    pub header: Option<BracedConfig>,
    pub header_var: Option<Ident>,
    pub query: Option<BracedConfig>,
    pub query_var: Option<Ident>,
    pub data: Option<ApiRequestData>,
}

#[derive(Clone, Debug)]
pub struct ApiRequestData {
    pub data_type: DataType,
    pub data: BracedConfig,
    pub data_var: Option<Ident>,
}

#[derive(Clone, Debug)]
pub enum DataType {
    Json(Span),
    Form(Span),
    Urlencoded(Span),
}

#[derive(Clone, Debug)]
pub struct ApiResponse {
    pub brace: Brace,
    pub header: Option<BracedConfig>,
    pub cookie: Option<BracedConfig>,
    pub data: Option<ApiResponseData>,
}

#[derive(Clone, Debug)]
pub struct ApiResponseData {
    pub data_type: DataType,
    pub data: BracedConfig,
}

pub trait TryParse {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>>
    where
        Self: Sized;
}

#[derive(Clone, Debug)]
pub struct BracedConfig {
    pub token: Span,
    pub extend: Option<Ident>,
    pub struct_name: Ident,
    pub brace: Brace,
    pub fields: Vec<Field>,
    pub removed_fields: HashSet<LitStr>,
}

#[derive(Clone, Debug)]
pub enum Type {
    Constant(Constant),
    String(StringType),
    Bool(Span),
    Integer(IntegerType),
    Float(FloatType),
    Object(ObjectType),
    Datetime(DateTimeType),
    JsonText(JsonStringType),
    Map(Span),
    List(ListType),
}

impl Type {
    pub fn pure(&self) -> Type {
        match self {
            Type::Constant(c) => Type::Constant(c.clone()),
            Type::String(s) => Type::String(s.clone()),
            Type::Bool(b) => Type::Bool(*b),
            Type::Integer(i) => Type::Integer(i.clone()),
            Type::Float(f) => Type::Float(f.clone()),
            Type::Object(obj) => Type::Object(obj.pure()),
            Type::Datetime(date) => Type::Datetime(date.clone()),
            Type::JsonText(json) => Type::JsonText(json.pure()),
            Type::Map(map) => Type::Map(*map),
            Type::List(list) => Type::List(ListType {
                bracket: list.bracket,
                element_type: Box::new(list.element_type.pure()),
            }),
        }
    }
    pub fn is_string(&self) -> bool {
        match self {
            Type::Constant(Constant::String(_)) => true,
            Type::String(_) => true,
            _ => false,
        }
    }
}

impl PartialEq<Type> for Type {
    fn eq(&self, other: &Type) -> bool {
        match (self, other) {
            (Self::Constant(l0), Type::Constant(r0)) => l0 == r0,
            (Self::String(_), Type::String(_)) => true,
            (Self::Bool(_), Type::Bool(_)) => true,
            (Self::Integer(_), Type::Integer(_)) => true,
            (Self::Float(_), Type::Float(_)) => true,
            (Self::Constant(Constant::String(_)), Type::String(_)) => true,
            (Self::Constant(Constant::Bool(_)), Type::Bool(_)) => true,
            (Self::Constant(Constant::Int(_)), Type::Integer(_)) => true,
            (Self::Constant(Constant::Float(_)), Type::Float(_)) => true,
            (Self::String(_), Type::Constant(Constant::String(_))) => true,
            (Self::Bool(_), Type::Constant(Constant::Bool(_))) => true,
            (Self::Integer(_), Type::Constant(Constant::Int(_))) => true,
            (Self::Float(_), Type::Constant(Constant::Float(_))) => true,
            (Self::Datetime(_), Type::Datetime(_)) => true,
            (Self::Object(l0), Type::Object(r0)) => l0.struct_name.eq(&r0.struct_name),
            (Self::JsonText(l0), Type::JsonText(r0)) => l0.typ.as_ref().eq(r0.typ.as_ref()),
            (Self::Map(_), Type::Map(_)) => true,
            (Self::List(l0), Type::List(r0)) => {
                l0.element_type.as_ref().eq(r0.element_type.as_ref())
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StringType {
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IntegerType {
    // uint, int
    pub token: Ident,
    pub limits: Option<IntLimits>,
}

#[derive(Clone, Debug)]
pub struct IntLimits {
    pub paren: Paren,
    pub limits: Punctuated<IntLimit, Token![,]>,
}

#[derive(Clone, Debug)]
pub enum IntLimit {
    Range(ExprRange),
    Opt(LitInt),
}

#[derive(Clone, Debug)]
pub struct FloatType {
    pub token: Ident,
    pub limits: Option<FloatLimits>,
}

#[derive(Clone, Debug)]
pub struct FloatLimits {
    pub paren: Paren,
    pub limits: Punctuated<ExprRange, Token![,]>,
}

#[derive(Clone, Debug)]
pub struct DateTimeType {
    pub span: Span,
    pub format: Option<DateTimeFormat>,
}

#[derive(Clone, Debug)]
pub struct DateTimeFormat {
    pub paren: Paren,
    pub format: LitStr,
    pub mod_name: Ident,
}

#[derive(Clone, Debug)]
pub struct JsonStringType {
    pub span: Span,
    pub paren: Paren,
    pub typ: Box<Type>,
}

impl JsonStringType {
    pub fn pure(&self) -> JsonStringType {
        JsonStringType {
            span: self.span,
            paren: self.paren,
            typ: Box::new(self.typ.pure()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ListType {
    pub bracket: Bracket,
    pub element_type: Box<Type>,
}

#[derive(Clone, Debug)]
pub struct ObjectType {
    pub struct_name: Ident,
    pub brace: Brace,
    pub fields: Vec<Field>,
}

impl ObjectType {
    pub fn pure(&self) -> ObjectType {
        ObjectType {
            struct_name: self.struct_name.clone(),
            brace: self.brace,
            fields: self
                .fields
                .iter()
                .map(
                    |Field {
                         name,
                         field_name,
                         optional,
                         typ,
                         default,
                         ..
                     }| Field {
                        name: name.clone(),
                        field_name: field_name.clone(),
                        optional: optional.clone(),
                        typ: typ.as_ref().map(|typ| typ.pure()),
                        alias: None,
                        expr: None,
                        default: default.clone(),
                    },
                )
                .collect(),
        }
    }
}

impl PartialEq<ObjectType> for ObjectType {
    fn eq(&self, other: &ObjectType) -> bool {
        self.struct_name.eq(&other.struct_name)
    }
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: LitStr,
    pub field_name: Ident,
    pub optional: Option<Span>,
    pub typ: Option<Type>,
    pub alias: Option<Ident>,
    pub expr: Option<Expr>,
    pub default: Option<syn::Expr>,
}

#[derive(Clone, Debug)]
pub struct ObjectFieldAlias {
    pub right_arrow: Token![->],
    pub map_to: Ident,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Constant(Constant),
    Variable(Variable),
    Json(JsonStringifyFn),
    Format(FormatFn),
    Datetime(DatetimeFn),
    Timestamp(UnixTimestampUintFn),
    Join(JoinStringFn),
    Or(OrExpr),
    Default(Span),
}

#[derive(Clone, Debug)]
pub struct Variable {
    pub dollar: Span,
    pub name: Ident,
    pub typ: Option<Type>,
    pub client_option: bool,
}

#[derive(Clone, Debug)]
pub enum Constant {
    String(LitStr),
    Bool(LitBool),
    Int(LitInt),
    Float(LitFloat),
    Object(ObjectConstant),
    Array(ConstantArray),
}

impl PartialEq for Constant {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(_), Self::String(_)) => true,
            (Self::Bool(_), Self::Bool(_)) => true,
            (Self::Int(_), Self::Int(_)) => true,
            (Self::Float(_), Self::Float(_)) => true,
            (Self::Object(l0), Self::Object(r0)) => l0 == r0,
            (Self::Array(l0), Self::Array(r0)) => l0 == r0,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ObjectConstant {
    pub span: Span,
    pub fields: Vec<ObjectConstantField>,
}

impl PartialEq for ObjectConstant {
    fn eq(&self, other: &Self) -> bool {
        if self.fields.len() != other.fields.len() {
            return false;
        }
        let fields1 = self
            .fields
            .iter()
            .map(|f| (&f.name, &f.value))
            .collect::<HashMap<_, _>>();
        for field2 in other.fields.iter() {
            if let Some(value1) = fields1.get(&field2.name) {
                if field2.value.ne(value1) {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Clone, Debug)]
pub struct ObjectConstantField {
    pub name: Ident,
    pub value: Constant,
}

#[derive(Clone, Debug)]
pub struct ConstantArray {
    pub span: Span,
    pub elements: Vec<Constant>,
}

impl PartialEq for ConstantArray {
    fn eq(&self, other: &Self) -> bool {
        if self.elements.len() != other.elements.len() {
            return false;
        }
        if self.elements.len() == 0 {
            return true;
        }
        let el1 = self.elements.first().unwrap();
        let el2 = other.elements.first().unwrap();
        el1.eq(el2)
    }
}

#[derive(Clone, Debug)]
pub struct FormatFn {
    pub fn_token: Span,
    pub paren: Paren,
    pub format_text: LitStr,
    pub args: Option<Punctuated<Expr, Token![,]>>,
}

#[derive(Clone, Debug)]
pub struct JsonStringifyFn {
    pub fn_token: Span,
    pub paren: Paren,
    pub variable: Variable,
}

#[derive(Clone, Debug)]
pub struct DatetimeFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
    pub format: LitStr,
}

#[derive(Clone, Debug)]
pub struct JoinStringFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
    pub sep: LitStr,
}

#[derive(Clone, Debug)]
pub struct UnixTimestampUintFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
}

#[derive(Clone, Debug)]
pub struct OrExpr {
    pub variable: Variable,
    pub or: Token![||],
    pub default: Constant,
}
