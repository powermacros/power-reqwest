use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Brace, Bracket, Paren},
    ExprRange, Ident, LitBool, LitFloat, LitInt, LitStr, Token,
};

pub type ClientParams = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;

#[derive(Clone)]
pub struct Client {
    pub name: Ident,
    pub options_name: Option<Ident>,
    pub options: Option<ClientParams>,
    pub hooks: Option<Hooks>,
    pub auth: Option<Auth>,
    pub signing: Option<Signing>,
    pub apis: Vec<Api>,
}

#[derive(Clone)]
pub struct Hooks {
    pub(crate) span: Span,
    pub on_submit: Option<syn::Path>,
}

#[derive(Clone)]
pub struct Signing {
    pub(crate) span: Span,
    pub sign_fn: Option<syn::Path>,
}

#[derive(Clone)]
pub struct Auth {
    pub(crate) span: Span,
    pub url: LitStr,
}

#[derive(Clone)]
pub struct Api {
    pub name: Ident,
    pub method: Ident,
    pub uri: ApiUri,
    pub paren: Paren,
    pub request: ApiRequest,
    pub response: Option<ApiResponse>,
}

#[derive(Clone)]
pub struct ApiUri {
    pub uri: LitStr,
    pub schema: Option<LitStr>,
    pub user: Option<LitStr>,
    pub passwd: Option<LitStr>,
    pub host: Option<LitStr>,
    pub port: Option<LitInt>,
    pub uri_path: Option<ApiUriPath>,
    pub uri_query: Option<ApiUriQuery>,
    pub fragment: Option<LitStr>,
}

#[derive(Clone)]
pub struct ApiUriPath {
    pub last_slash: bool,
    pub segments: Vec<ApiUriSeg>,
}

#[derive(Clone)]
pub enum ApiUriSeg {
    Static(LitStr),
    Var(Ident),
}

#[derive(Clone)]
pub struct ApiUriQuery {
    pub fields: Vec<Field<(), (), (Token![=], Expr)>>,
}

pub type RequestHeaders = BracedConfig<(), (), (Token![=], Expr)>;
pub type RequestQueries = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;
pub type RequestForm = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;
pub type RequestJson = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;

#[derive(Clone)]
pub struct ApiRequest {
    pub brace: Brace,
    pub header: Option<RequestHeaders>,
    pub query: Option<RequestQueries>,
    pub form: Option<RequestForm>,
    pub json: Option<RequestJson>,
}

pub type ResponseHeaders = BracedConfig<(), (Token![->], Ident), ()>;
pub type ResponseCookies = BracedConfig<(), (Token![->], Ident), ()>;
pub type ResponseJson = BracedConfig<Type<(Token![->], Ident), ()>, (Token![->], Ident), ()>;
pub type ResponseForm = BracedConfig<Type<(Token![->], Ident), ()>, (Token![->], Ident), ()>;

#[derive(Clone)]
pub struct ApiResponse {
    pub brace: Brace,
    pub header: Option<ResponseHeaders>,
    pub cookie: Option<ResponseCookies>,
    pub json: Option<ResponseJson>,
    pub form: Option<ResponseForm>,
}

pub trait AsFieldType<A, X> {
    fn peek(input: ParseStream) -> syn::Result<()>;
    fn parse_type(input: ParseStream) -> syn::Result<Self>
    where
        Self: Sized;
    fn as_type(&self) -> Option<&Type<A, X>>;
    fn as_type_mut(&mut self) -> Option<&mut Type<A, X>>;
}

pub trait TryParse {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>>
    where
        Self: Sized;
}

pub trait AsFieldAlias: TryParse {
    fn as_alias(&self) -> Option<&Ident>;
}

pub trait AsFieldAssignment: TryParse {
    fn as_assignment(&self) -> Option<&Expr>;
}

impl<A, X> AsFieldType<A, X> for () {
    fn peek(_input: ParseStream) -> syn::Result<()> {
        Ok(())
    }
    fn parse_type(_input: ParseStream) -> syn::Result<Self> {
        Ok(())
    }

    fn as_type(&self) -> Option<&Type<A, X>> {
        None
    }

    fn as_type_mut(&mut self) -> Option<&mut Type<A, X>> {
        None
    }
}

impl TryParse for () {
    fn try_parse(_input: ParseStream) -> syn::Result<Option<Self>> {
        Ok(Some(()))
    }
}

impl AsFieldAlias for () {
    fn as_alias(&self) -> Option<&Ident> {
        None
    }
}

impl AsFieldAssignment for () {
    fn as_assignment(&self) -> Option<&Expr> {
        None
    }
}

impl<T: Parse> TryParse for (Token![->], T) {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>>
    where
        Self: Sized,
    {
        if input.peek(Token![->]) {
            let arrow = input.parse::<Token![->]>()?;
            Ok(Some((arrow, input.parse()?)))
        } else {
            Ok(None)
        }
    }
}

impl AsFieldAlias for (Token![->], Ident) {
    fn as_alias(&self) -> Option<&Ident> {
        Some(&self.1)
    }
}

impl<T: Parse> TryParse for (Token![=], T) {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>>
    where
        Self: Sized,
    {
        if input.peek(Token![=]) {
            let arrow = input.parse::<Token![=]>()?;
            Ok(Some((arrow, input.parse()?)))
        } else {
            Ok(None)
        }
    }
}

impl AsFieldAssignment for (Token![=], Expr) {
    fn as_assignment(&self) -> Option<&Expr> {
        Some(&self.1)
    }
}

#[derive(Clone)]
pub struct BracedConfig<T, A, X> {
    pub token: Span,
    pub struct_name: Ident,
    pub brace: Brace,
    pub fields: Vec<Field<T, A, X>>,
}

#[derive(Clone)]
pub enum Type<A, X> {
    Constant(Constant),
    String(StringType),
    Bool(Span),
    Integer(IntegerType),
    Float(FloatType),
    Object(ObjectType<A, X>),
    DatetimeString(DateTimeStringType),
    JsonText(JsonStringType<A, X>),
    Map(Span),
    List(ListType<A, X>),
}

#[derive(Clone)]
pub struct StringType {
    pub span: Span,
}

#[derive(Clone)]
pub struct IntegerType {
    // uint, int
    pub token: Ident,
    pub limits: Option<IntLimits>,
}

#[derive(Clone)]
pub struct IntLimits {
    pub paren: Paren,
    pub limits: Punctuated<IntLimit, Token![,]>,
}

#[derive(Clone)]
pub enum IntLimit {
    Range(ExprRange),
    Opt(LitInt),
}

#[derive(Clone)]
pub struct FloatType {
    pub token: Ident,
    pub limits: Option<FloatLimits>,
}

#[derive(Clone)]
pub struct FloatLimits {
    pub paren: Paren,
    pub limits: Punctuated<ExprRange, Token![,]>,
}

#[derive(Clone)]
pub struct DateTimeStringType {
    pub span: Span,
    pub paren: Paren,
    pub format: LitStr,
}

#[derive(Clone)]
pub struct JsonStringType<A, X> {
    pub span: Span,
    pub paren: Paren,
    pub typ: Box<Type<A, X>>,
}

#[derive(Clone)]
pub struct ListType<A, X> {
    pub bracket: Bracket,
    pub element_type: Box<Type<A, X>>,
}

pub type ObjectField<A, X> = Field<Type<A, X>, A, X>;

#[derive(Clone)]
pub struct ObjectType<A, X> {
    pub struct_name: Ident,
    pub brace: Brace,
    pub fields: Vec<ObjectField<A, X>>,
}

#[derive(Clone)]
pub struct Field<T, A, X> {
    pub name: LitStr,
    pub field_name: Ident,
    pub optional: Option<Span>,
    pub typ: T,
    pub alias: Option<A>,
    pub expr: Option<X>,
    pub default: Option<syn::Expr>,
}

#[derive(Clone)]
pub struct ObjectFieldAlias {
    pub right_arrow: Token![->],
    pub map_to: Ident,
}

#[derive(Clone)]
pub enum Expr {
    Constant(Constant),
    Variable(Variable),
    Json(JsonStringifyFn),
    Format(FormatFn),
    Datetime(DatetimeFn),
    Timestamp(UnixTimestampUintFn),
    Join(JoinStringFn),
    Or(OrExpr),
}

#[derive(Clone)]
pub struct Variable {
    pub dollar: Span,
    pub name: Ident,
    pub typ: Option<Type<(), ()>>,
}

#[derive(Clone)]
pub enum Constant {
    String(LitStr),
    Bool(LitBool),
    Int(LitInt),
    Float(LitFloat),
    Object(ObjectConstant),
    Array(ConstantArray),
}

#[derive(Clone)]
pub struct ObjectConstant {
    pub span: Span,
    pub fields: Vec<ObjectConstantField>,
}

#[derive(Clone)]
pub struct ObjectConstantField {
    pub name: Ident,
    pub value: Constant,
}

#[derive(Clone)]
pub struct ConstantArray {
    pub span: Span,
    pub elements: Vec<Constant>,
}

#[derive(Clone)]
pub struct FormatFn {
    pub fn_token: Span,
    pub paren: Paren,
    pub format_text: LitStr,
    pub args: Option<Punctuated<Expr, Token![,]>>,
}

#[derive(Clone)]
pub struct JsonStringifyFn {
    pub fn_token: Span,
    pub paren: Paren,
    pub variable: Variable,
}

#[derive(Clone)]
pub struct DatetimeFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
    pub format: LitStr,
}

#[derive(Clone)]
pub struct JoinStringFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
    pub sep: LitStr,
}

#[derive(Clone)]
pub struct UnixTimestampUintFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
}

#[derive(Clone)]
pub struct OrExpr {
    pub variable: Variable,
    pub or: Token![||],
    pub default: Constant,
}
