pub mod url_parser {
    use convert_case::{Case, Casing};
    use nom::{
        branch::alt,
        bytes::complete::{tag, take, take_while1},
        character::complete::{alpha1, alphanumeric1, digit1, one_of},
        combinator::{map, opt},
        error::{context, ErrorKind},
        multi::{count, many0, many1, many_m_n},
        sequence::{preceded, separated_pair, terminated, tuple},
        AsChar, IResult, InputTakeAtPosition,
    };
    use syn::{LitInt, Token};
    use syn_prelude::{ToExpr, ToIdent, ToLitStr, ToSynError};

    use crate::{ApiUriPath, ApiUriQuery, ApiUriSeg, Constant, Expr, Field, Variable};

    pub struct ApiUri<'a> {
        schema: Option<&'a str>,
        auth: Option<(&'a str, Option<&'a str>)>,
        host: Option<String>,
        port: Option<u16>,
        path: Option<UrlPath<'a>>,
        query: Option<UrlQuery<'a>>,
        fragment: Option<&'a str>,
    }

    pub fn parse_uri_and_update_api(api: &mut crate::ApiUri) -> syn::Result<()> {
        let value = api.uri.value();
        let span = api.uri.span();
        let (
            _,
            ApiUri {
                schema,
                auth,
                host,
                port,
                path,
                query,
                fragment,
            },
        ) = uri(&value).map_err(|_| span.to_syn_error("bad url"))?;

        api.schema = schema.map(|schema| (schema, span).to_lit_str());
        api.user = auth.map(|(user, _)| (user, span).to_lit_str());
        api.passwd = auth
            .map(|(_, pswd)| pswd.map(|pswd| (pswd, span).to_lit_str()))
            .flatten();
        api.host = host.map(|host| (host, span).to_lit_str());
        api.port = port.map(|port| LitInt::new(&format!("{port}"), span));
        api.uri_path = path.map(
            |UrlPath {
                 segments,
                 last_slash,
             }| ApiUriPath {
                last_slash,
                segments: segments
                    .into_iter()
                    .map(|seg| match seg {
                        Segment::CodePoints(s) => ApiUriSeg::Static((s, span).to_lit_str()),
                        Segment::Variable(v) => ApiUriSeg::Var((v.name, span).to_ident()),
                    })
                    .collect(),
            },
        );
        api.uri_query = query.map(|UrlQuery { params }| ApiUriQuery {
            fields: params
                .into_iter()
                .map(|Param { name, value }| {
                    let mut default = None;
                    let expr = if let Some(value) = value {
                        Some((
                            Token![=](span),
                            match value {
                                Segment::CodePoints(s) => {
                                    default = Some((s, span).to_lit_str().to_expr());
                                    Expr::Constant(Constant::String((s, span).to_lit_str()))
                                }
                                Segment::Variable(v) => Expr::Variable(Variable {
                                    dollar: span,
                                    name: (v.name, span).to_ident(),
                                    typ: None,
                                }),
                            },
                        ))
                    } else {
                        None
                    };
                    Field {
                        name: (name, span).to_lit_str(),
                        field_name: (name.to_case(Case::Snake), span).to_ident(),
                        optional: None,
                        typ: (),
                        alias: None,
                        expr,
                        default,
                    }
                })
                .collect::<Vec<_>>(),
        });
        api.fragment = fragment.map(|f| (f, span).to_lit_str());
        Ok(())
    }

    pub fn uri(input: &str) -> IResult<&str, ApiUri> {
        let (rest, schema) = opt(alt((
            terminated(tag("https"), tag("://")),
            terminated(tag("http"), tag("://")),
        )))(input)?;

        let mut auth = None;
        let mut host = None;
        let mut port = None;

        let rest = if schema.is_some() {
            let (rest, a) = opt(authority)(rest)?;
            auth = a;
            let (rest, h) = ip_or_host(rest)?;
            host = Some(h);
            let (rest, port_str) = opt(preceded(
                tag(":"),
                context(
                    "port",
                    map(many1(digit1), |s| {
                        s.join("").parse::<u16>().map_err(|_| {
                            nom::Err::Error(nom::error::Error::new(input, ErrorKind::AlphaNumeric))
                        })
                    }),
                ),
            ))(rest)?;

            if let Some(p) = port_str {
                port = Some(p?);
            }
            rest
        } else {
            rest
        };

        let (rest, path) = path(rest)?;
        let (rest, query) = query(rest)?;
        let (rest, fragment) = opt(preceded(tag("#"), code_points))(rest)?;

        Ok((
            rest,
            ApiUri {
                schema,
                auth,
                host,
                port,
                path,
                query,
                fragment,
            },
        ))
    }

    fn authority(input: &str) -> IResult<&str, (&str, Option<&str>)> {
        context(
            "authority",
            terminated(
                separated_pair(alphanumeric1, opt(tag(":")), opt(alphanumeric1)),
                tag("@"),
            ),
        )(input)
    }

    fn host(input: &str) -> IResult<&str, String> {
        context(
            "host",
            alt((
                tuple((many1(terminated(alphanumerichyphen1, tag("."))), alpha1)),
                tuple((many_m_n(1, 1, alphanumerichyphen1), take(0 as usize))),
            )),
        )(input)
        .map(|(next_input, mut res)| {
            if !res.1.is_empty() {
                res.0.push(res.1);
            }
            (next_input, res.0.join("."))
        })
    }

    fn alphanumerichyphen1(input: &str) -> IResult<&str, &str> {
        input.split_at_position1_complete(
            |item| {
                let char_item = item.as_char();
                !(char_item == '-') && !char_item.is_alphanum()
            },
            ErrorKind::AlphaNumeric,
        )
    }

    fn ip_num(input: &str) -> IResult<&str, u8> {
        context("ip number", n_to_m_digits(1, 3))(input).and_then(|(next_input, result)| {
            match result.parse::<u8>() {
                Ok(n) => Ok((next_input, n)),
                Err(_) => Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    ErrorKind::AlphaNumeric,
                ))),
            }
        })
    }

    fn n_to_m_digits<'a>(n: usize, m: usize) -> impl FnMut(&'a str) -> IResult<&str, String> {
        move |input| {
            many_m_n(n, m, one_of("0123456789"))(input)
                .map(|(next_input, result)| (next_input, result.into_iter().collect()))
        }
    }

    fn ip(input: &str) -> IResult<&str, String> {
        context(
            "ip",
            tuple((count(terminated(ip_num, tag(".")), 3), ip_num)),
        )(input)
        .map(|(next_input, res)| {
            let mut result: [u8; 4] = [0, 0, 0, 0];
            res.0
                .into_iter()
                .enumerate()
                .for_each(|(i, v)| result[i] = v);
            result[3] = res.1;
            (next_input, result.map(|n| n.to_string()).join("."))
        })
    }

    fn ip_or_host(input: &str) -> IResult<&str, String> {
        context("ip or host", alt((ip, host)))(input)
    }

    pub struct UrlPath<'a> {
        segments: Vec<Segment<'a>>,
        last_slash: bool,
    }

    fn path(input: &str) -> IResult<&str, Option<UrlPath>> {
        map(
            context(
                "path",
                opt(preceded(
                    tag("/"),
                    tuple((many0(terminated(path_segment, tag("/"))), opt(path_segment))),
                )),
            ),
            |segments| {
                if let Some((segments, last)) = segments {
                    let mut path = UrlPath {
                        segments,
                        last_slash: last.is_none(),
                    };
                    if let Some(last) = last {
                        path.segments.push(last);
                    }
                    Some(path)
                } else {
                    None
                }
            },
        )(input)
    }

    pub enum Segment<'a> {
        CodePoints(&'a str),
        Variable(Var<'a>),
    }

    fn path_segment(input: &str) -> IResult<&str, Segment> {
        alt((
            map(code_points, |v| Segment::CodePoints(v)),
            map(variable, |v| Segment::Variable(v)),
        ))(input)
    }

    fn code_points(input: &str) -> IResult<&str, &str> {
        input.split_at_position1_complete(
            |item| {
                !(item == '-') && !item.is_alphanum() && !(item == '.')
                // ... actual ascii code points and url encoding...: https://infra.spec.whatwg.org/#ascii-code-point
            },
            ErrorKind::AlphaNumeric,
        )
    }

    pub struct Var<'a> {
        name: &'a str,
    }

    fn variable(input: &str) -> IResult<&str, Var> {
        context(
            "variable",
            preceded(
                tag("$"),
                map(
                    take_while1(|item: char| item.is_alphanum() || item == '_'),
                    |name| Var { name },
                ),
            ),
        )(input)
    }

    pub struct UrlQuery<'a> {
        params: Vec<Param<'a>>,
    }

    fn query(input: &str) -> IResult<&str, Option<UrlQuery>> {
        context(
            "query params",
            map(
                opt(preceded(
                    tag("?"),
                    opt(tuple((param, many0(preceded(tag("&"), param))))),
                )),
                |params| {
                    params.flatten().map(|(first, mut params)| {
                        params.insert(0, first);
                        UrlQuery { params }
                    })
                },
            ),
        )(input)
    }

    pub struct Param<'a> {
        name: &'a str,
        value: Option<Segment<'a>>,
    }

    fn param(input: &str) -> IResult<&str, Param> {
        context(
            "query param",
            map(
                preceded(
                    tag("&"),
                    tuple((code_points, opt(preceded(tag("="), path_segment)))),
                ),
                |(name, value)| Param { name, value },
            ),
        )(input)
    }
}

pub mod format_parser {}
