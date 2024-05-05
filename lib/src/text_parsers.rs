pub mod url_parser {
    use std::net::Ipv4Addr;

    use convert_case::{Case, Casing};
    use nom::{
        branch::alt,
        bytes::complete::{tag, take_while, take_while1},
        character::complete::{alpha1, alphanumeric1, digit1, one_of},
        combinator::{map, map_res, opt},
        error::{context, ErrorKind},
        multi::{count, many0, many1, many_m_n},
        sequence::{preceded, separated_pair, terminated, tuple},
        AsChar, IResult, InputTakeAtPosition,
    };
    use proc_macro2::Span;
    use syn::LitInt;
    use syn_prelude::{ToErr, ToExpr, ToIdent, ToLitStr, ToSynError};

    use crate::{
        ApiUriPath, ApiUriQuery, ApiUriSeg, Constant, Expr, Field, FloatType, IntegerType,
        StringType, Type, Variable,
    };

    pub struct ApiUri<'a> {
        schema: Option<&'a str>,
        auth: Option<(&'a str, Option<&'a str>)>,
        host: Option<IpOrHost<'a>>,
        port: Option<PortOrVar<'a>>,
        path: Option<UrlPath<'a>>,
        query: Option<UrlQuery<'a>>,
        fragment: Option<&'a str>,
    }

    pub fn parse_uri_and_update_api(api: &mut crate::ApiUri) -> syn::Result<()> {
        let value = api.uri_format.value();
        let span = api.uri_format.span();
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

        let mut uri_format = schema.map(|s| s.to_owned()).unwrap_or_default();
        api.schema = schema.map(|schema| (schema, span).to_lit_str());
        if schema.is_some() {
            uri_format.push_str("://");
        }
        api.user = auth.map(|(user, _)| (user, span).to_lit_str());
        api.passwd = auth
            .map(|(_, pswd)| pswd.map(|pswd| (pswd, span).to_lit_str()))
            .flatten();
        if let Some(host_ip) = host {
            match host_ip {
                IpOrHost::Ip(ip) => {
                    let ip = Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]);
                    if ip.is_benchmarking()
                        || ip.is_broadcast()
                        || ip.is_documentation()
                        || ip.is_link_local()
                        || ip.is_multicast()
                        || ip.is_reserved()
                        || ip.is_shared()
                    {
                        span.to_syn_error("unsupported ip address").to_err()?;
                    }
                    uri_format.push_str(&ip.to_string());
                }
                IpOrHost::Host(host_segs) => {
                    uri_format.push_str(
                        &host_segs
                            .iter()
                            .map(|seg| match seg {
                                HostSeg::Seg(s) => *s,
                                HostSeg::Var(_) => "{}",
                            })
                            .collect::<Vec<_>>()
                            .join("."),
                    );
                    api.uri_variables = host_segs
                        .iter()
                        .filter_map(|seg| match seg {
                            HostSeg::Seg(_) => None,
                            HostSeg::Var(var) => Some(var.to_variable(span)),
                        })
                        .collect::<Vec<_>>();
                }
            };
        }
        if let Some(port) = port {
            uri_format.push(':');
            match port {
                PortOrVar::Port(port) => {
                    uri_format.push_str(&format!("{port}"));
                    api.port = Some(LitInt::new(&format!("{port}"), span));
                }
                PortOrVar::Var(var) => {
                    uri_format.push_str("{}");
                    api.port_var = Some(var.to_variable(span));
                    api.uri_variables.push(var.to_variable(span));
                }
            }
        }
        api.uri_path = path.map(
            |UrlPath {
                 segments,
                 last_slash,
             }| {
                let segments = segments
                    .into_iter()
                    .map(|seg| {
                        if !uri_format.ends_with("/") {
                            uri_format.push('/');
                        }
                        match seg {
                            Segment::CodePoints(s) => {
                                uri_format.push_str(s);
                                ApiUriSeg::Static((s, span).to_lit_str())
                            }
                            Segment::Variable(v) => {
                                uri_format.push_str("{}");
                                api.uri_variables.push(v.to_variable(span));
                                ApiUriSeg::Var(v.to_variable(span))
                            }
                        }
                    })
                    .collect();
                if last_slash {
                    if !uri_format.ends_with("/") {
                        uri_format.push('/');
                    }
                }

                ApiUriPath {
                    last_slash,
                    segments,
                }
            },
        );
        api.uri_query = query.map(|UrlQuery { params }| ApiUriQuery {
            fields: params
                .into_iter()
                .map(|Param { name, value }| {
                    let mut default = None;
                    let expr = if let Some(value) = value {
                        Some(match value {
                            Segment::CodePoints(s) => {
                                default = Some((s, span).to_lit_str().to_expr());
                                Expr::Constant(Constant::String((s, span).to_lit_str()))
                            }
                            Segment::Variable(v) => Expr::Variable(v.to_variable(span)),
                        })
                    } else {
                        None
                    };
                    Field {
                        name: (name, span).to_lit_str(),
                        field_name: (name.to_case(Case::Snake), span).to_ident(),
                        optional: None,
                        typ: None,
                        alias: None,
                        expr,
                        default,
                    }
                })
                .collect::<Vec<_>>(),
        });
        api.uri_format = (uri_format, span).to_lit_str();
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
            let (rest, p) = opt(preceded(tag(":"), port_or_var))(rest)?;
            port = p;

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

    fn host(input: &str) -> IResult<&str, Vec<HostSeg>> {
        context(
            "host",
            alt((
                map(
                    tuple((
                        many1(terminated(host_seg, tag("."))),
                        alt((
                            map(alpha1, |s| HostSeg::Seg(s)),
                            map(variable, |v| HostSeg::Var(v)),
                        )),
                    )),
                    |(mut segs, seg)| {
                        segs.push(seg);
                        segs
                    },
                ),
                map(host_seg, |seg| vec![seg]),
            )),
        )(input)
    }

    enum HostSeg<'a> {
        Seg(&'a str),
        Var(Var<'a>),
    }

    fn host_seg(input: &str) -> IResult<&str, HostSeg> {
        alt((
            map(alphanumerichyphen1, |s| HostSeg::Seg(s)),
            map(variable, |v| HostSeg::Var(v)),
        ))(input)
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

    fn ipv4(input: &str) -> IResult<&str, [u8; 4]> {
        context(
            "ip",
            map(
                tuple((count(terminated(ip_num, tag(".")), 3), ip_num)),
                |res| {
                    let mut result: [u8; 4] = [0, 0, 0, 0];
                    res.0
                        .into_iter()
                        .enumerate()
                        .for_each(|(i, v)| result[i] = v);
                    result[3] = res.1;
                    result
                },
            ),
        )(input)
    }

    enum IpOrHost<'a> {
        Ip([u8; 4]),
        Host(Vec<HostSeg<'a>>),
    }

    fn ip_or_host(input: &str) -> IResult<&str, IpOrHost> {
        context(
            "ip or host",
            alt((
                map(ipv4, |ip| IpOrHost::Ip(ip)),
                map(host, |host| IpOrHost::Host(host)),
            )),
        )(input)
    }

    enum PortOrVar<'a> {
        Port(u16),
        Var(Var<'a>),
    }

    fn port_or_var(input: &str) -> IResult<&str, PortOrVar> {
        context(
            "port",
            alt((
                map_res(digit1, |s: &str| {
                    s.parse().map(|port| PortOrVar::Port(port))
                }),
                map(variable, |var| PortOrVar::Var(var)),
            )),
        )(input)
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
        typ: Option<&'static str>,
        client_option: bool,
    }

    impl Var<'_> {
        fn to_variable(&self, span: Span) -> Variable {
            Variable {
                dollar: span,
                name: (self.name, span).to_ident(),
                client_option: self.client_option,
                typ: self.typ.map(|typ| match typ {
                    "string" => Type::String(StringType { span }),
                    "bool" => Type::Bool(span),
                    "f32" => Type::Float(FloatType {
                        token: ("f32", span).to_ident(),
                        limits: None,
                    }),
                    "f64" => Type::Float(FloatType {
                        token: ("f64", span).to_ident(),
                        limits: None,
                    }),
                    int @ _ => Type::Integer(IntegerType {
                        token: (int, span).to_ident(),
                        limits: None,
                    }),
                }),
            }
        }
    }

    fn variable(input: &str) -> IResult<&str, Var> {
        context(
            "variable",
            alt((
                variable_with_type,
                preceded(
                    tag("$$"),
                    map(
                        take_while1(|item: char| item.is_alphanum() || item == '_'),
                        |name| Var {
                            name,
                            typ: None,
                            client_option: true,
                        },
                    ),
                ),
                preceded(
                    tag("$"),
                    map(
                        take_while1(|item: char| item.is_alphanum() || item == '_'),
                        |name| Var {
                            name,
                            typ: None,
                            client_option: false,
                        },
                    ),
                ),
            )),
        )(input)
    }

    fn variable_with_type(input: &str) -> IResult<&str, Var> {
        let (rest, _) = tag("$")(input)?;
        let (rest, _) = take_while(|item: char| item.is_whitespace())(rest)?;
        let (rest, _) = tag("{")(rest)?;
        let (rest, _) = take_while(|item: char| item.is_whitespace())(rest)?;
        let (rest, name) = take_while1(|item: char| item.is_alphanum() || item == '_')(rest)?;
        let (rest, _) = take_while(|item: char| item.is_whitespace())(rest)?;
        let (rest, has_type) = opt(tag(":"))(rest)?;
        let (rest, typ) = if has_type.is_some() {
            let (rest, _) = take_while(|item: char| item.is_whitespace())(rest)?;
            let (rest, typ) = alt((
                map(alt((tag("string"), tag("str"), tag("String"))), |_| {
                    "string"
                }),
                map(alt((tag("int"), tag("integer"), tag("i64"))), |_| "i64"),
                map(alt((tag("uint"), tag("u64"))), |_| "u64"),
                map(tag("i8"), |_| "i8"),
                map(tag("u8"), |_| "u8"),
                map(tag("i16"), |_| "i16"),
                map(tag("u16"), |_| "u16"),
                map(tag("i32"), |_| "i32"),
                map(tag("u32"), |_| "u32"),
                map(tag("bool"), |_| "bool"),
                map(alt((tag("float"), tag("f64"))), |_| "f64"),
                map(tag("f32"), |_| "f32"),
            ))(rest)?;
            let (rest, _) = take_while(|item: char| item.is_whitespace())(rest)?;
            (rest, Some(typ))
        } else {
            (rest, None)
        };
        let (rest, _) = tag("}")(rest)?;
        Ok((
            rest,
            Var {
                name,
                typ,
                client_option: false,
            },
        ))
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
