use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Brace, Bracket, Paren},
    ExprRange, Ident, LitBool, LitFloat, LitInt, LitStr, Token,
};

pub type ClientParams = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;

pub struct Client {
    pub name: Ident,
    pub params: Option<ClientParams>,
    pub hooks: Option<Hooks>,
    pub auth: Option<Auth>,
    pub signing: Option<Signing>,
    pub apis: Vec<Api>,
}

pub struct Hooks {
    pub(crate) span: Span,
    pub on_submit: Option<syn::Path>,
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
    pub name: Ident,
    pub method: Ident,
    pub paren: Paren,
    pub url: LitStr,
    pub request: ApiRequest,
    pub response: Option<ApiResponse>,
}

pub type RequestHeaders = BracedConfig<(), (), (Token![=], Expr)>;
pub type RequestQueries = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;
pub type RequestForm = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;
pub type RequestJson = BracedConfig<Type<(), (Token![=], Expr)>, (), (Token![=], Expr)>;

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

pub struct ApiResponse {
    pub brace: Brace,
    pub header: Option<ResponseHeaders>,
    pub cookie: Option<ResponseCookies>,
    pub json: Option<ResponseJson>,
    pub form: Option<ResponseForm>,
}

pub trait ParseType {
    fn peek(input: ParseStream) -> syn::Result<()>;
    fn parse_type(input: ParseStream) -> syn::Result<Self>
    where
        Self: Sized;
}

pub trait TryParse {
    fn try_parse(input: ParseStream) -> syn::Result<Option<Self>>
    where
        Self: Sized;
}

impl ParseType for () {
    fn peek(_input: ParseStream) -> syn::Result<()> {
        Ok(())
    }
    fn parse_type(_input: ParseStream) -> syn::Result<Self> {
        Ok(())
    }
}

impl TryParse for () {
    fn try_parse(_input: ParseStream) -> syn::Result<Option<Self>> {
        Ok(Some(()))
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

pub struct BracedConfig<T: ParseType, A: TryParse, X: TryParse> {
    pub token: Span,
    pub brace: Brace,
    pub fields: Vec<Field<T, A, X>>,
}

pub enum Type<A: TryParse, X: TryParse> {
    Constant(Constant),
    String(StringType),
    Bool(Span),
    Integer(IntegerType),
    Float(FloatType),
    Object(ObjectType<A, X>),
    Datetime(DateTimeStringType),
    Json(JsonStringType<A, X>),
    Map(Span),
    List(ListType<A, X>),
}

pub struct StringType {
    pub span: Span,
}

pub struct IntegerType {
    // uint, int
    pub token: Ident,
    pub limits: Option<IntLimits>,
}

pub struct IntLimits {
    pub paren: Paren,
    pub limits: Punctuated<IntLimit, Token![,]>,
}

pub enum IntLimit {
    Range(ExprRange),
    Opt(LitInt),
}

pub struct FloatType {
    pub span: Span,
    pub limits: Option<FloatLimits>,
}

pub struct FloatLimits {
    pub paren: Paren,
    pub limits: Punctuated<ExprRange, Token![,]>,
}

pub struct DateTimeStringType {
    pub span: Span,
    pub paren: Paren,
    pub format: LitStr,
}

pub struct JsonStringType<A: TryParse, X: TryParse> {
    pub span: Span,
    pub paren: Paren,
    pub typ: Box<Type<A, X>>,
}

pub struct ListType<A: TryParse, X: TryParse> {
    pub bracket: Bracket,
    pub element_type: Box<Type<A, X>>,
}

pub type ObjectField<A, X> = Field<Type<A, X>, A, X>;

pub struct ObjectType<A: TryParse, X: TryParse> {
    pub brace: Brace,
    pub fields: Vec<ObjectField<A, X>>,
}

pub struct Field<T: ParseType, A: TryParse, X: TryParse> {
    pub name: LitStr,
    pub optional: Option<Token![?]>,
    pub typ: T,
    pub alias: Option<A>,
    pub expr: Option<X>,
}

pub struct ObjectFieldAlias {
    pub right_arrow: Token![->],
    pub map_to: Ident,
}

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

pub struct Variable {
    pub dollar: Span,
    pub name: Ident,
    pub typ: Option<Type<(), ()>>,
}

pub enum Constant {
    String(LitStr),
    Bool(LitBool),
    Int(LitInt),
    Float(LitFloat),
    Object(ObjectConstant),
    Array(ConstantArray),
}

pub struct ObjectConstant {
    pub span: Span,
    pub fields: Vec<ObjectConstantField>,
}

pub struct ObjectConstantField {
    pub name: Ident,
    pub value: Constant,
}

pub struct ConstantArray {
    pub span: Span,
    pub elements: Vec<Constant>,
}

pub struct FormatFn {
    pub fn_token: Span,
    pub paren: Paren,
    pub format_text: LitStr,
    pub args: Option<Punctuated<Expr, Token![,]>>,
}

pub struct JsonStringifyFn {
    pub fn_token: Span,
    pub paren: Paren,
    pub variable: Variable,
}

pub struct DatetimeFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
    pub format: LitStr,
}

pub struct JoinStringFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
    pub sep: LitStr,
}

pub struct UnixTimestampUintFn {
    pub token: Span,
    pub paren: Paren,
    pub variable: Variable,
}

pub struct OrExpr {
    pub variable: Variable,
    pub or: Token![||],
    pub default: Constant,
}
