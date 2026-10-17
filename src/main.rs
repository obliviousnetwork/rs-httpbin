use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, MatchedPath, Path, Request},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header::*},
    middleware,
    response::{AppendHeaders, Html, IntoResponse, Redirect, Response, Sse, sse::Event},
    routing::*,
};
use axum_client_ip::InsecureClientIp;
use axum_extra::{
    TypedHeader,
    extract::{CookieJar, Host, Query, cookie},
    headers::{
        Authorization, ContentType, HeaderMapExt, Range, UserAgent,
        authorization::{Basic, Bearer},
    },
    response::ErasedJson,
};
use axum_valid::Garde;
use base64::{Engine, prelude::BASE64_STANDARD};
use garde::Validate;
use mime::{APPLICATION_JSON, IMAGE, TEXT_HTML_UTF_8, TEXT_PLAIN, TEXT_PLAIN_UTF_8, TEXT_XML};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower::ServiceBuilder;
use tower_http::{
    ServiceBuilderExt, compression::CompressionLayer, cors::CorsLayer, request_id::MakeRequestUuid, set_header::SetRequestHeaderLayer,
    trace::TraceLayer,
};
use tracing::debug_span;
use tracing_subscriber::{EnvFilter, fmt::layer, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::data::{Headers, Http, Queries};

mod data;
mod ws;
mod ws_chat;

#[cfg(test)]
mod tests;

fn app() -> Router<()> {
    let mut router = Router::new()
        .route("/", get(index))
        .merge(
            Router::new()
                .route("/delete", delete(anything))
                .route("/get", get(anything))
                .route("/head", head(anything))
                .route("/options", options(anything))
                .route("/patch", patch(anything))
                .route("/post", post(anything))
                .route("/put", put(anything))
                .route("/trace", trace(anything)),
        )
        .merge(
            Router::new()
                .route("/anything", any(anything))
                .route("/anything/{*path}", any(anything)),
        )
        .merge(
            Router::new()
                .route("/absolute-redirect/{n}", any(redirect::absolute_redirect))
                .route("/redirect/{n}", any(redirect::redirect))
                .route("/relative-redirect/{n}", any(redirect::relative_redirect))
                .route("/redirect-to", any(redirect::redirect_to)),
        )
        .merge(
            Router::new()
                .route("/basic-auth/{user}/{passwd}", any(basic_auth::basic_auth))
                .route("/hidden-basic-auth/{user}/{passwd}", any(basic_auth::hidden_basic_auth)),
        )
        .merge(
            Router::new()
                .route("/user-agent", any(user_agent))
                .route("/headers", any(headers))
                .route("/json", get(resp_data::json))
                .route("/xml", get(resp_data::xml))
                .route("/forms/post", any(resp_data::forms_post))
                .route("/html", get(resp_data::html))
                .route("/hostname", get(hostname))
                .route("/uuid", any(uuid))
                .route("/response-headers", any(response_headers))
                .route("/ip", any(ip))
                .route("/bearer", any(bearer)),
        )
        .merge(
            Router::new()
                .route("/image", any(image::image))
                .route("/image/jpeg", any(image::jpeg))
                .route("/image/svg", any(image::svg))
                .route("/image/png", any(image::png))
                .route("/image/webp", any(image::webp))
                .route("/image/avif", any(image::avif))
                .route("/image/jxl", any(image::jxl)),
        )
        //
        .merge(
            Router::new()
                .route("/base64/{value}", any(base_64::base64_decode))
                .route("/base64/encode/{value}", any(base_64::base64_encode))
                .route("/base64/decode/{value}", any(base_64::base64_decode)),
        )
        .route("/sse", any(sse::sse_handler))
        .merge(
            Router::new()
                .route("/cookies", any(cookies::cookies))
                .route("/cookies/set", any(cookies::cookies_set))
                .route("/cookies/delete", any(cookies::cookies_del)),
        )
        .route("/encoding/utf8", any(utf8))
        .route("/robots.txt", any(robots_txt))
        .merge(
            Router::new()
                .route("/links/{total}", any(links::links))
                .route("/links/{total}/{page}", any(links::links)),
        )
        .route("/unstable", get(unstable))
        .route("/status/{codes}", any(status::status))
        .route("/delay/{n}", any(anything).layer(middleware::from_fn(delay_s)))
        .route("/delay/ms/{n}", any(anything).layer(middleware::from_fn(delay_ms)))
        .merge(
            Router::new()
                .route("/cache", any(cache::cache))
                .route("/cache/{n}", any(cache::cache_n))
                .route("/deny", any(deny))
                .route("/bytes/{n}", any(bytes::bytes_n))
                .route("/stream-bytes/{n}", any(stream_bytes::stream_bytes_n))
                .route("/stream/{n}", any(stream::stream_n))
                .route("/drip", any(drip::drip))
                .route("/dump/request", any(dump_request))
                .route("/etag/{etag}", any(etag::etag))
                .route("/range/{n}", any(range::range)),
        )
        .merge(
            Router::new()
                .route("/digest-auth/{qop}/{user}/{passwd}", any(digest_auth::digest_auth_no_algo))
                .route("/digest-auth/{qop}/{user}/{passwd}/{algorithm}", any(digest_auth::digest_auth)),
        )
        .route("/websocket/echo", any(ws::ws_handler))
        .route("/websocket/chat", any(ws_chat::ws_handler))
        .route(
            "/socket-io/chat",
            any(|| async { Html(include_str!("../assets/socketio-chat.html")) }),
        )
        .merge(
            Router::new()
                .route("/openapi.json", get(|| async { include_str!("../openapi.json") }))
                .route("/swagger-ui", get(|| async { Html(swagger_ui::swagger_ui_html("/openapi.json")) })),
        );

    for format in ["gzip", "zstd", "br", "deflate"] {
        router = router.route(
            format!("/{format}").as_str(),
            get(anything).layer(
                ServiceBuilder::default()
                    .layer(SetRequestHeaderLayer::overriding(ACCEPT_ENCODING, HeaderValue::from_static(format)))
                    .layer(CompressionLayer::new()),
            ),
        );
    }

    // router.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api))

    router
}

mod socket_io_chat;
mod swagger_ui;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "DEBUG".into()))
        .with(layer().json())
        .init();

    let port: u16 = std::env::var("PORT").ok().and_then(|port| port.parse().ok()).unwrap_or(3000);
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await.unwrap();
    start_server(listener).await;
}

pub(crate) async fn start_server(listener: tokio::net::TcpListener) {
    let router = app();
    let service = ServiceBuilder::default()
        .compression()
        .set_x_request_id(MakeRequestUuid)
        .propagate_x_request_id()
        .layer(TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
            let request_id = request.headers()["x-request-id"].to_str().ok();
            let matched_path = request.extensions().get::<MatchedPath>().map(MatchedPath::as_str);
            let method = request.method().as_str();
            debug_span!("request_id", method, matched_path, request_id,)
        }))
        .layer(CorsLayer::very_permissive())
        .layer({
            async fn delay(request: Request, next: middleware::Next) -> impl IntoResponse {
                let before = Instant::now();
                let resp = next.run(request).await;
                let after = Instant::now();
                (
                    AppendHeaders([("Server-Timing", format!("total;dur={}", (after - before).as_millis()))]),
                    resp,
                )
            }
            middleware::from_fn(delay)
        })
        .layer(socket_io_chat::socket_io_layer())
        .layer(DefaultBodyLimit::disable());

    let app = router.layer(service);

    eprintln!("Listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn user_agent(user_agent: Option<TypedHeader<UserAgent>>) -> impl IntoResponse {
    ErasedJson::pretty(data::UserAgent {
        user_agent: user_agent.map(|TypedHeader(h)| h.to_string()).unwrap_or_default(),
    })
}

async fn headers(header_map: HeaderMap) -> impl IntoResponse {
    ErasedJson::pretty(get_headers(&header_map))
}

#[derive(Debug, Validate, Deserialize)]
struct UnstableQueryParam {
    #[garde(range(min = 0.0, max = 2.0))]
    pub failure_rate: Option<f32>,
}

async fn unstable(Garde(Query(query)): Garde<Query<UnstableQueryParam>>) -> Response {
    let failure_rate = match query.failure_rate {
        None => 0.5,
        Some(failure_rate @ 0.0..=1.0) => failure_rate,
        _ => {
            return ErasedJson::pretty(data::ErrorDetail::new(
                400,
                "Bad Request",
                format!(
                    "invalid failure rate: {} not in range [0, 1]",
                    query.failure_rate.map_or("None".to_string(), |it| f32::to_string(&it))
                ),
            ))
            .into_response();
        }
    };

    if fastrand::f32() <= failure_rate {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    ().into_response()
}

mod basic_auth {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct BasicAuth {
        pub authorized: bool,
        pub user: String,
    }

    #[derive(Deserialize)]
    pub struct BasicAuthParam {
        user: String,
        passwd: String,
    }

    pub async fn basic_auth(
        Path(BasicAuthParam { user, passwd }): Path<BasicAuthParam>,
        basic_auth: Option<TypedHeader<Authorization<Basic>>>,
    ) -> impl IntoResponse {
        let authorized = match &basic_auth {
            None => false,
            Some(auth) => auth.username() == user && auth.password() == passwd,
        };
        let body = ErasedJson::pretty(BasicAuth {
            authorized,
            user: basic_auth.map(|it| it.username().to_string()).unwrap_or_default(),
        });
        if authorized {
            (StatusCode::OK, body).into_response()
        } else {
            (
                StatusCode::UNAUTHORIZED,
                [(WWW_AUTHENTICATE, HeaderValue::from_static(r#"Basic realm="Fake Realm""#))],
                body,
            )
                .into_response()
        }
    }

    pub async fn hidden_basic_auth(
        Path(BasicAuthParam { user, passwd }): Path<BasicAuthParam>,
        basic_auth: Option<TypedHeader<Authorization<Basic>>>,
    ) -> impl IntoResponse {
        let authorized = match basic_auth {
            None => false,
            Some(auth) => auth.username() == user && auth.password() == passwd,
        };

        if authorized {
            (
                StatusCode::OK,
                ErasedJson::pretty(BasicAuth {
                    authorized,
                    user: if authorized { user } else { Default::default() },
                }),
            )
                .into_response()
        } else {
            (
                StatusCode::NOT_FOUND,
                ErasedJson::pretty(data::ErrorDetail::new(404, "Not Found", "")),
            )
                .into_response()
        }
    }
}

mod status {
    use super::*;

    const STATUS_300_BODY: &[u8] = br#"<!doctype html>
<head>
<title>Multiple Choices</title>
</head>
<body>
<ul>
<li><a href="/image/jpeg">/image/jpeg</a></li>
<li><a href="/image/png">/image/png</a></li>
<li><a href="/image/svg">/image/svg</a></li>
</body>
</html>"#;

    const STATUS_308_BODY: &[u8] = br#"<!doctype html>
<head>
<title>Permanent Redirect</title>
</head>
<body>Permanently redirected to <a href="/image/jpeg">/image/jpeg</a>
</body>
</html>"#;

    const STATUS_406_BODY: &[u8] = br#"{
  "message": "Client did not request a supported media type",
  "accept": [
    "image/webp",
    "image/svg+xml",
    "image/jpeg",
    "image/png",
    "image/"
  ]
}
"#;

    pub async fn status(Path(codes): Path<String>) -> Response {
        // A comma-separated list of codes (optionally with `:weight` suffixes)
        // is treated as a weighted random choice.
        if codes.contains(',') {
            match parse_weighted_choices(&codes) {
                Ok(choices) => do_status(weighted_random_choice(&choices)),
                Err(detail) => bad_request(detail),
            }
        } else {
            match parse_status_code(&codes) {
                Ok(code) => do_status(code),
                Err(detail) => bad_request(detail),
            }
        }
    }

    fn bad_request(detail: String) -> Response {
        (
            StatusCode::BAD_REQUEST,
            ErasedJson::pretty(data::ErrorDetail::new(400, "Bad Request", detail)),
        )
            .into_response()
    }

    fn parse_status_code(input: &str) -> Result<u16, String> {
        let code: u16 = input.parse().map_err(|err| format!("invalid status code: {input:?}: {err}"))?;
        if !(100..=599).contains(&code) {
            return Err(format!("invalid status code: {code} not in range [100, 599]"));
        }
        Ok(code)
    }

    fn parse_weighted_choices(input: &str) -> Result<Vec<(u16, f64)>, String> {
        let mut choices = Vec::new();
        for pair in input.split(',') {
            let (raw_choice, raw_weight) = match pair.split_once(':') {
                Some((choice, weight)) => (choice, Some(weight)),
                None => (pair, None),
            };
            let weight = match raw_weight {
                Some(weight) => weight.parse::<f64>().map_err(|_| format!("invalid weight value: {weight:?}"))?,
                None => 1.0,
            };
            let code = parse_status_code(raw_choice).map_err(|_| format!("invalid choice value: {raw_choice:?}"))?;
            choices.push((code, weight));
        }
        Ok(choices)
    }

    fn weighted_random_choice(choices: &[(u16, f64)]) -> u16 {
        let total_weight: f64 = choices.iter().map(|(_, weight)| weight).sum();
        let fallback = choices.last().map(|(code, _)| *code).unwrap_or(200);
        if total_weight <= 0.0 {
            return fallback;
        }
        let mut remaining = fastrand::f64() * total_weight;
        for (code, weight) in choices {
            remaining -= weight;
            if remaining < 0.0 {
                return *code;
            }
        }
        fallback
    }

    fn do_status(code: u16) -> Response {
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));

        let body: &[u8] = match code {
            300 => {
                headers.insert(LOCATION, HeaderValue::from_static("/image/jpeg"));
                STATUS_300_BODY
            }
            301 | 302 | 303 | 305 | 307 => {
                headers.insert(LOCATION, HeaderValue::from_static("/redirect/1"));
                b""
            }
            308 => {
                headers.insert(LOCATION, HeaderValue::from_static("/image/jpeg"));
                STATUS_308_BODY
            }
            401 => {
                headers.insert(WWW_AUTHENTICATE, HeaderValue::from_static(r#"Basic realm="Fake Realm""#));
                b""
            }
            402 => {
                headers.insert("x-more-info", HeaderValue::from_static("http://vimeo.com/22053820"));
                b"Fuck you, pay me!"
            }
            406 => {
                headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                STATUS_406_BODY
            }
            407 => {
                headers.insert(PROXY_AUTHENTICATE, HeaderValue::from_static(r#"Basic realm="Fake Realm""#));
                b""
            }
            418 => {
                headers.insert("x-more-info", HeaderValue::from_static("http://tools.ietf.org/html/rfc2324"));
                b"I'm a teapot!"
            }
            _ => b"",
        };

        (status, headers, body).into_response()
    }
}

fn get_headers(header_map: &HeaderMap) -> Headers {
    let mut headers = Headers::default();

    for (k, v) in header_map {
        let v = String::from_utf8_lossy(v.as_bytes()).to_string();
        let values = headers.entry(k.as_str().to_string()).or_default();
        values.push(v);
    }

    headers
}

mod cookies {
    use super::*;

    pub async fn cookies(jar: CookieJar) -> impl IntoResponse {
        let m: BTreeMap<_, _> = jar.iter().map(|k| k.name_value()).collect();
        ErasedJson::pretty(m)
    }

    pub async fn cookies_set(Query(query): Query<BTreeMap<String, Vec<String>>>) -> impl IntoResponse {
        let mut jar = CookieJar::new();
        for (k, mut v) in query {
            let v = v.swap_remove(v.len() - 1);
            jar = jar.add(cookie::Cookie::new(k, v));
        }
        (StatusCode::FOUND, (jar, Redirect::to("/cookies"))).into_response()
    }

    pub async fn cookies_del(Query(query): Query<BTreeMap<String, Vec<String>>>) -> impl IntoResponse {
        let mut jar = CookieJar::new();
        for (k, _) in query {
            jar = jar.add(
                cookie::Cookie::build((k, ""))
                    .max_age(std::time::Duration::ZERO.try_into().unwrap_or_default())
                    .expires(Some(std::time::SystemTime::now().into()))
                    .http_only(true)
                    .build(),
            );
        }
        (StatusCode::FOUND, (jar, Redirect::to("/cookies")))
    }
}
async fn anything(
    method: Method,
    uri: Uri,
    Query(query): Query<Vec<(String, Vec<String>)>>,
    header_map: HeaderMap,
    content_type: Option<TypedHeader<ContentType>>,
    InsecureClientIp(origin): InsecureClientIp,
    body: Bytes,
) -> Response {
    let headers = get_headers(&header_map);

    let mut queries = Queries::default();
    for (k, v) in query {
        let vec = queries.entry(k).or_default();
        vec.extend(v);
    }

    let body_string = match std::str::from_utf8(&body) {
        Ok(body) => body.into(),
        Err(_) => BASE64_STANDARD.encode(&body),
    };

    let Http {
        mut form,
        mut files,
        mut json,
        ..
    } = Default::default();
    if let Some(TypedHeader(c)) = content_type {
        let mime: mime::Mime = c.into();
        match (mime.type_(), mime.subtype()) {
            (mime::APPLICATION, mime::JSON) => json = serde_json::from_slice(&body).ok(),
            (mime::APPLICATION, mime::WWW_FORM_URLENCODED) => {
                let f = form_urlencoded::parse(&body);
                for (k, v) in f {
                    form.entry(k.to_string()).or_default().push(v.to_string());
                }
            }
            (mime::MULTIPART, mime::FORM_DATA) => {
                let content_type = headers
                    .headers
                    .get(CONTENT_TYPE.as_str())
                    .and_then(|it| it.first())
                    .map(|s| s.as_str())
                    .unwrap_or_default();
                let boundary = multer::parse_boundary(content_type).ok();
                if let Some(boundary) = boundary {
                    let mut m = multer::Multipart::new(Body::from(body).into_data_stream(), boundary);
                    // println!("{?}", m);

                    while let Some((_idx, field)) = m.next_field_with_idx().await.unwrap_or_default() {
                        // println!("{?}",&field);
                        match (field.file_name(), field.name()) {
                            (None, Some(name)) => form
                                .entry(name.to_string())
                                .or_default()
                                .push(field.text().await.unwrap_or_default()),
                            (Some(_f), Some(name)) => {
                                let vec = files.entry(name.into()).or_default();
                                let field_val = field.bytes().await.unwrap_or_default();
                                let string = match std::str::from_utf8(&field_val) {
                                    Ok(body) => body.into(),
                                    Err(_) => BASE64_STANDARD.encode(&field_val),
                                };
                                vec.push(string);
                            }
                            (_, _) => {}
                        }
                    }
                }
            }
            (_, _) => {}
        }
    }

    ErasedJson::pretty(Http {
        method: method.to_string(),
        uri: uri.to_string(),
        headers,
        origin: origin.into(),
        args: queries,
        data: body_string,
        json,
        form,
        files,
    })
    .into_response()
}

async fn index() -> Html<String> {
    let md = include_str!("../README.md");
    Html(
        markdown::to_html_with_options(md, &markdown::Options::gfm()).unwrap_or_default()
            // language=html
            + indoc::indoc!(r#"
<style>
  @media (prefers-color-scheme: dark) {
    html, img, video, iframe {
      filter: invert(1);
    }
    body {
      background-color: white;
    }
  }
</style>
    "#),
    )
}

#[derive(Deserialize, Serialize, Default)]
struct HostName {
    hostname: String,
}

async fn hostname() -> impl IntoResponse {
    let hostname = whoami::hostname().unwrap_or("<< unknown hostname >>".to_string());

    ErasedJson::pretty(HostName { hostname })
}

async fn utf8() -> impl IntoResponse {
    Html(include_str!("../assets/utf8.html"))
}

async fn delay_ms(Path(delayms): Path<u16>, request: Request, next: middleware::Next) -> impl IntoResponse {
    let before = Instant::now();
    tokio::time::sleep(Duration::from_millis(delayms.min(10000).into())).await;
    let resp = next.run(request).await;
    let after = Instant::now();
    (
        AppendHeaders([("Server-Timing", format!("delay;dur={}", (after - before).as_millis()))]),
        resp,
    )
}

async fn delay_s(Path(delays): Path<u16>, request: Request, next: middleware::Next) -> impl IntoResponse {
    delay_ms(Path(delays * 1000), request, next).await
}

#[derive(Deserialize, Serialize, Default)]
struct UuidResponse {
    uuid: String,
}

async fn uuid() -> impl IntoResponse {
    ErasedJson::pretty(UuidResponse {
        uuid: Uuid::new_v4().to_string(),
    })
}

async fn response_headers(Query(query): Query<BTreeMap<String, Vec<String>>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();

    for (k, v) in query.iter().flat_map(|(k, v)| v.iter().map(move |v| (k, v))) {
        if let (Ok(k), Ok(v)) = (HeaderName::from_str(k), HeaderValue::from_str(v)) {
            headers.append(k, v);
        }
    }

    (headers, ErasedJson::pretty(query))
}

async fn robots_txt() -> impl IntoResponse {
    into_response(TEXT_PLAIN_UTF_8, include_str!("../assets/robots.txt"))
}

#[inline]
fn into_response(content_type: impl AsRef<str>, body: impl IntoResponse) -> Response {
    ([(CONTENT_TYPE, content_type.as_ref())], body).into_response()
}

mod resp_data {
    use super::*;
    pub async fn json() -> impl IntoResponse {
        into_response(APPLICATION_JSON, include_str!("../assets/sample.json"))
    }
    pub async fn xml() -> impl IntoResponse {
        into_response(TEXT_XML, include_str!("../assets/sample.xml"))
    }
    pub async fn html() -> impl IntoResponse {
        into_response(TEXT_HTML_UTF_8, include_str!("../assets/sample.html"))
    }
    pub async fn forms_post() -> impl IntoResponse {
        into_response(TEXT_HTML_UTF_8, include_str!("../assets/forms_post.html"))
    }
}

mod image {
    use super::*;

    pub async fn image(headers: HeaderMap) -> impl IntoResponse {
        let mime = headers.get(ACCEPT).and_then(|v| v.to_str().ok()).map(mime::MimeIter::new);

        if let Some(mime) = mime {
            for m in mime.filter_map(|m| m.ok()).filter(|it| it.type_() == IMAGE) {
                return match m.subtype().as_str() {
                    "jpeg" => jpeg().await,
                    "svg" => svg().await,
                    "png" => png().await,
                    "webp" => webp().await,
                    "avif" => avif().await,
                    "jxl" => jxl().await,
                    _ => continue,
                };
            }
        }

        StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response()
    }

    pub async fn jpeg() -> Response {
        into_response("image/jpeg", include_bytes!("../assets/jpeg.jpeg"))
    }
    pub async fn svg() -> Response {
        into_response("image/svg", include_bytes!("../assets/svg.svg"))
    }
    pub async fn png() -> Response {
        into_response("image/png", include_bytes!("../assets/png.png"))
    }
    pub async fn webp() -> Response {
        into_response("image/webp", include_bytes!("../assets/webp.webp"))
    }
    pub async fn jxl() -> Response {
        into_response("image/jxl", include_bytes!("../assets/jxl.jxl"))
    }
    pub async fn avif() -> Response {
        into_response("image/avif", include_bytes!("../assets/avif.avif"))
    }
}

async fn ip(InsecureClientIp(origin): InsecureClientIp) -> impl IntoResponse {
    ErasedJson::pretty(data::Ip { origin })
}

#[derive(Serialize, Deserialize)]
pub struct BearerAuth {
    pub authorized: bool,
    pub token: String,
}

async fn bearer(header_map: HeaderMap) -> impl IntoResponse {
    let authorization = header_map.typed_get::<Authorization<Bearer>>();

    match authorization {
        None => ErasedJson::pretty(data::ErrorDetail::new(401, "Unauthorized", "")).into_response(),
        Some(token) => ErasedJson::pretty(BearerAuth {
            authorized: true,
            token: token.token().to_string(),
        })
        .into_response(),
    }
}

mod redirect {
    use super::*;

    pub async fn redirect(Path(n): Path<i32>) -> Response {
        match n {
            ..=0 => (StatusCode::BAD_REQUEST, bad_redirect_request()).into_response(),
            1 => (StatusCode::FOUND, Redirect::to("/get")).into_response(),
            2.. => (StatusCode::FOUND, Redirect::to(&format!("/redirect/{}", n - 1))).into_response(),
        }
    }

    fn bad_redirect_request() -> ErasedJson {
        ErasedJson::pretty(data::ErrorDetail::new(400, "Bad Request", "redirect count must be > 0".to_string()))
    }

    pub async fn relative_redirect(Path(n): Path<i32>) -> Response {
        match n {
            ..=0 => (StatusCode::BAD_REQUEST, bad_redirect_request()).into_response(),
            1 => (StatusCode::FOUND, Redirect::to("/get")).into_response(),
            2.. => (StatusCode::FOUND, [(LOCATION, format!("./{}", n - 1))]).into_response(),
        }
    }

    pub async fn absolute_redirect(Path(n): Path<i32>, uri: Uri, Host(host): Host, _req: Request) -> Response {
        match n {
            ..=0 => (StatusCode::BAD_REQUEST, bad_redirect_request()).into_response(),
            1 => (StatusCode::FOUND, Redirect::to("/get")).into_response(),
            2.. => (
                StatusCode::FOUND,
                Redirect::to(&format!(
                    "{}://{host}/absolute-redirect/{}",
                    uri.scheme_str().unwrap_or("http"),
                    n - 1
                )),
            )
                .into_response(),
        }
    }

    #[derive(Debug, Deserialize, Validate)]
    pub struct Params {
        #[garde(length(min = 0))]
        url: String,
        #[garde(range(min = 100, max = 999))]
        status_code: Option<u16>,
    }

    pub async fn redirect_to(Garde(Query(p)): Garde<Query<Params>>) -> Response {
        let Params { url, status_code } = p;
        let status_code = status_code.unwrap_or(302);
        (
            StatusCode::from_u16(status_code)
                .ok()
                .filter(StatusCode::is_redirection)
                .unwrap_or(StatusCode::FOUND),
            Redirect::to(&url),
        )
            .into_response()
    }
}

mod base_64 {
    use super::*;

    pub async fn base64_decode(Path(base64_data): Path<String>) -> impl IntoResponse {
        (
            [(CONTENT_TYPE, TEXT_PLAIN.as_ref())],
            BASE64_STANDARD.decode(base64_data).unwrap_or_else(|e| e.to_string().into_bytes()),
        )
    }

    pub async fn base64_encode(Path(data): Path<String>) -> impl IntoResponse {
        ([(CONTENT_TYPE, TEXT_PLAIN.as_ref())], BASE64_STANDARD.encode(data))
    }
}

mod sse {
    use jiff::SignedDuration;

    use super::*;

    #[derive(Deserialize)]
    pub struct SeeParam {
        pub count: Option<usize>,
        #[serde(default)]
        pub duration: Option<SignedDuration>,
        #[serde(default)]
        pub delay: Option<SignedDuration>,
    }

    pub async fn sse_handler(Query(SeeParam { delay, duration, count }): Query<SeeParam>) -> Response {
        use tokio_stream::StreamExt as _;
        tokio::time::sleep(delay.unwrap_or(SignedDuration::ZERO).unsigned_abs()).await;
        let sec = duration.unwrap_or(SignedDuration::from_secs(1)).unsigned_abs().as_secs_f32();
        let stream = tokio_stream::iter(1..)
            .take(count.unwrap_or(10_usize))
            .throttle(Duration::from_secs_f32(sec))
            .map(|id| {
                let timestamp = jiff::Timestamp::now().as_millisecond();
                #[allow(clippy::useless_conversion)]
                Event::default()
                    .data(serde_json::to_string(&data::SseData { id, timestamp }).unwrap_or_default())
                    .event("ping")
                    .try_into()
            });

        Sse::new(stream).into_response()
    }
}

mod links {
    use super::*;

    #[derive(Debug, Deserialize, Validate)]
    pub struct LinksParam {
        #[garde(range(min = 0, max = 256))]
        pub total: u32,
        #[garde(range(min = 0))]
        pub page: Option<u32>,
    }

    pub async fn links(Garde(Path(p)): Garde<Path<LinksParam>>) -> Response {
        let LinksParam { total, page } = p;

        let Some(cur) = page else {
            return Redirect::to(format!("/links/{total}/0").as_str()).into_response();
        };

        tracing::info!(cur, total);

        let mut env = minijinja::Environment::new();
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);

        let html = env
            .render_str(
                // language=html
                indoc::indoc! {
r#"
        <html>
            <head>
                <title>Links</title>
            </head>
            <body>
            {% for idx in range(total) %}
                {% if idx == cur %}
                    {{idx}}
                {% else %}
                    <a href="/links/{{total}}/{{idx}}">{{idx}}</a>
                {% endif %}
            {% endfor %}
            </body>
        </html>
    "#},
                minijinja::context! {total, cur},
            )
            .unwrap_or_default();
        Html(html).into_response()
    }
}

async fn deny() -> impl IntoResponse {
    into_response(TEXT_PLAIN_UTF_8, "YOU SHOULDN'T BE HERE")
}

mod cache {
    use super::*;

    pub async fn cache(headers: HeaderMap) -> Response {
        if headers.contains_key(IF_MODIFIED_SINCE) || headers.contains_key(IF_NONE_MATCH) {
            StatusCode::NOT_MODIFIED.into_response()
        } else {
            StatusCode::OK.into_response()
        }
    }

    pub async fn cache_n(Path(n): Path<u32>) -> Response {
        ([(CACHE_CONTROL, format!("public, max-age={n}"))], StatusCode::OK).into_response()
    }
}

mod bytes {
    use super::*;

    #[derive(Debug, Deserialize)]
    pub struct BytesQuery {
        seed: Option<u64>,
    }

    pub async fn bytes_n(Path(n): Path<u32>, Query(q): Query<BytesQuery>) -> Response {
        // Cap the response at 10 MiB so a single request cannot generate an unbounded body.
        let n = n.min(10 * 1024 * 1024) as usize;
        let mut buf = vec![0u8; n];
        match q.seed {
            Some(seed) => fastrand::Rng::with_seed(seed).fill(&mut buf),
            None => fastrand::fill(&mut buf),
        }
        ([(CONTENT_TYPE, "application/octet-stream")], buf).into_response()
    }
}

mod stream_bytes {
    use super::*;

    #[derive(Debug, Deserialize)]
    pub struct StreamBytesQuery {
        seed: Option<u64>,
        chunk_size: Option<usize>,
    }

    pub async fn stream_bytes_n(Path(n): Path<u32>, Query(q): Query<StreamBytesQuery>) -> Response {
        let total = n as usize;
        let chunk_size = q.chunk_size.unwrap_or(1024).clamp(1, 10240);
        let mut rng = match q.seed {
            Some(seed) => fastrand::Rng::with_seed(seed),
            None => fastrand::Rng::new(),
        };
        let stream = tokio_stream::iter((0..total).step_by(chunk_size).map(move |start| {
            let end = (start + chunk_size).min(total);
            let mut buf = vec![0u8; end - start];
            rng.fill(&mut buf);
            Ok::<_, std::io::Error>(Bytes::from(buf))
        }));
        ([(CONTENT_TYPE, "application/octet-stream")], Body::from_stream(stream)).into_response()
    }
}

mod stream {
    use super::*;

    pub async fn stream_n(Path(n): Path<u32>) -> Response {
        let n = n.min(100);
        let stream = tokio_stream::iter((0..n).map(move |id| {
            let line = serde_json::to_string(&json!({
                "id": id,
                "url": format!("/stream/{n}"),
                "args": {},
                "headers": {},
            }))
            .unwrap_or_default()
                + "\n";
            Ok::<_, std::io::Error>(Bytes::from(line))
        }));
        ([(CONTENT_TYPE, APPLICATION_JSON.as_ref())], Body::from_stream(stream)).into_response()
    }
}

mod drip {
    use tokio_stream::StreamExt as _;

    use super::*;

    #[derive(Debug, Deserialize)]
    pub struct DripQuery {
        numbytes: Option<usize>,
        duration: Option<u64>,
        delay: Option<u64>,
        code: Option<u16>,
    }

    pub async fn drip(Query(q): Query<DripQuery>) -> Response {
        let numbytes = q.numbytes.unwrap_or(10);
        let duration = q.duration.unwrap_or(2);
        let delay = q.delay.unwrap_or(0);
        let code = StatusCode::from_u16(q.code.unwrap_or(200)).unwrap_or(StatusCode::OK);

        tokio::time::sleep(Duration::from_secs(delay)).await;

        if numbytes == 0 {
            return (code, [(CONTENT_TYPE, "application/octet-stream")], Body::empty()).into_response();
        }
        if duration == 0 {
            return (
                code,
                [(CONTENT_TYPE, "application/octet-stream")],
                Bytes::from(vec![b'*'; numbytes]),
            )
                .into_response();
        }

        let chunk_size = ((numbytes as f64 / duration as f64).ceil() as usize).max(1);
        let num_chunks = numbytes.div_ceil(chunk_size);
        let per_chunk = Duration::from_secs_f64(duration as f64 / num_chunks as f64);

        let stream = tokio_stream::iter((0..numbytes).step_by(chunk_size).map(move |start| {
            let len = chunk_size.min(numbytes - start);
            Ok::<_, std::io::Error>(Bytes::from(vec![b'*'; len]))
        }))
        .throttle(per_chunk);

        (code, [(CONTENT_TYPE, "application/octet-stream")], Body::from_stream(stream)).into_response()
    }
}

async fn dump_request(request: Request) -> Response {
    use std::fmt::Write as _;

    let (parts, body) = request.into_parts();
    let version = match parts.version {
        axum::http::Version::HTTP_09 => "HTTP/0.9",
        axum::http::Version::HTTP_10 => "HTTP/1.0",
        axum::http::Version::HTTP_11 => "HTTP/1.1",
        axum::http::Version::HTTP_2 => "HTTP/2.0",
        axum::http::Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1",
    };

    let mut dump = String::new();
    let _ = writeln!(&mut dump, "{} {} {version}", parts.method, parts.uri);
    for (k, v) in &parts.headers {
        match v.to_str() {
            Ok(v) => {
                let _ = writeln!(&mut dump, "{k}: {v}");
            }
            Err(_) => {
                let _ = writeln!(&mut dump, "{k}: <binary>");
            }
        }
    }
    let _ = writeln!(&mut dump);

    let body = axum::body::to_bytes(body, 10_000_000).await.unwrap_or_default();
    if !body.is_empty() {
        match std::str::from_utf8(&body) {
            Ok(body) => dump.push_str(body),
            Err(_) => dump.push_str("<binary body>\n"),
        }
    }

    ([(CONTENT_TYPE, "text/plain; charset=utf-8")], dump).into_response()
}

mod etag {
    use super::*;

    pub async fn etag(Path(etag): Path<String>, headers: HeaderMap) -> Response {
        if let Some(tag) = headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok())
            && tag == etag
        {
            return StatusCode::NOT_MODIFIED.into_response();
        }

        if let Some(tag) = headers.get(IF_MATCH).and_then(|v| v.to_str().ok())
            && tag != etag
        {
            return StatusCode::PRECONDITION_FAILED.into_response();
        }

        ([(ETAG, etag.clone())], ErasedJson::pretty(json!({ "etag": etag }))).into_response()
    }
}

mod range {
    use std::ops::Bound;

    use tokio_stream::StreamExt as _;

    use super::*;

    #[derive(Debug, Deserialize)]
    pub struct RangeQuery {
        duration: Option<f64>,
        chunk_size: Option<usize>,
    }

    fn body_stream(start: u64, end: u64, chunk_size: usize, duration: f64) -> Body {
        let chunk_size = chunk_size.max(1);
        let chunks = ((end - start + 1) as usize).div_ceil(chunk_size);
        let stream = tokio_stream::iter((start..=end).step_by(chunk_size).map(move |s| {
            let len = (chunk_size as u64).min(end - s + 1) as usize;
            Ok::<_, std::io::Error>(Bytes::from(vec![b'a'; len]))
        }));
        if duration > 0.0 && chunks > 0 {
            Body::from_stream(stream.throttle(Duration::from_secs_f64(duration / chunks as f64)))
        } else {
            Body::from_stream(stream)
        }
    }

    pub async fn range(Path(n): Path<u32>, range: Option<TypedHeader<Range>>, Query(q): Query<RangeQuery>) -> Response {
        let total = n as u64;
        let chunk_size = q.chunk_size.unwrap_or(1024);
        let duration = q.duration.unwrap_or(0.0);

        if total == 0 {
            return (StatusCode::OK, [(CONTENT_TYPE, "application/octet-stream")]).into_response();
        }

        if let Some(TypedHeader(range)) = range {
            if let Some((start_bound, end_bound)) = range.satisfiable_ranges(total).next() {
                let start = match start_bound {
                    Bound::Included(v) => v,
                    Bound::Excluded(v) => v + 1,
                    Bound::Unbounded => 0,
                };
                let end = match end_bound {
                    Bound::Included(v) => v,
                    Bound::Excluded(v) => v - 1,
                    Bound::Unbounded => total - 1,
                };
                let end = end.min(total - 1);
                if start > end {
                    return (StatusCode::RANGE_NOT_SATISFIABLE, [(CONTENT_RANGE, format!("bytes */{total}"))]).into_response();
                }
                return (
                    StatusCode::PARTIAL_CONTENT,
                    [
                        (CONTENT_TYPE, "application/octet-stream".to_string()),
                        (CONTENT_RANGE, format!("bytes {start}-{end}/{total}")),
                    ],
                    body_stream(start, end, chunk_size, duration),
                )
                    .into_response();
            }
            return (StatusCode::RANGE_NOT_SATISFIABLE, [(CONTENT_RANGE, format!("bytes */{total}"))]).into_response();
        }

        (
            StatusCode::OK,
            [(CONTENT_TYPE, "application/octet-stream")],
            body_stream(0, total - 1, chunk_size, duration),
        )
            .into_response()
    }
}

mod digest_auth {
    use super::*;

    const REALM: &str = "rs-httpbin";
    const NONCE: &str = "deadbeef";
    const OPAQUE: &str = "cafebabe";

    pub async fn digest_auth(
        Path((qop, user, passwd, algorithm)): Path<(String, String, String, String)>,
        headers: HeaderMap,
        method: Method,
    ) -> Response {
        handle(qop, user, passwd, algorithm, headers, method)
    }

    pub async fn digest_auth_no_algo(
        Path((qop, user, passwd)): Path<(String, String, String)>,
        headers: HeaderMap,
        method: Method,
    ) -> Response {
        handle(qop, user, passwd, "MD5".to_string(), headers, method)
    }

    #[allow(clippy::too_many_arguments)]
    fn handle(qop: String, user: String, passwd: String, algorithm: String, headers: HeaderMap, method: Method) -> Response {
        let authorized = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|auth| verify(auth, &user, &passwd, &qop, &algorithm, method.as_str()))
            .unwrap_or(false);

        if authorized {
            return (
                StatusCode::OK,
                [(CONTENT_TYPE, "application/json")],
                ErasedJson::pretty(json!({
                    "authenticated": true,
                    "user": user,
                    "qop": qop,
                    "algorithm": algorithm,
                })),
            )
                .into_response();
        }

        let challenge = format!(r#"Digest realm="{REALM}",qop="{qop}",nonce="{NONCE}",opaque="{OPAQUE}",algorithm="{algorithm}""#);
        (
            StatusCode::UNAUTHORIZED,
            [(WWW_AUTHENTICATE, challenge)],
            ErasedJson::pretty(json!({ "authenticated": false, "user": user })),
        )
            .into_response()
    }

    fn hash(algorithm: &str, input: &str) -> Option<String> {
        use sha2::Digest as _;

        match algorithm.to_ascii_uppercase().as_str() {
            "MD5" => Some(format!("{:x}", md5::compute(input))),
            "SHA-256" => Some(format!("{:x}", sha2::Sha256::digest(input.as_bytes()))),
            "SHA-512-256" => Some(format!("{:x}", sha2::Sha512_256::digest(input.as_bytes()))),
            _ => None,
        }
    }

    fn parse_digest(header: &str) -> Option<BTreeMap<String, String>> {
        let header = header.strip_prefix("Digest ")?;
        let mut map = BTreeMap::new();
        for part in header.split(',') {
            let part = part.trim();
            if let Some((k, v)) = part.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
            }
        }
        Some(map)
    }

    fn verify(header: &str, user: &str, passwd: &str, qop: &str, algorithm: &str, method: &str) -> Option<bool> {
        let map = parse_digest(header)?;

        if map.get("username")? != user || map.get("realm")? != REALM || map.get("nonce")? != NONCE {
            return Some(false);
        }

        let uri = map.get("uri")?;
        let response = map.get("response")?;

        let ha1 = hash(algorithm, &format!("{user}:{REALM}:{passwd}"))?;
        let ha2 = hash(algorithm, &format!("{method}:{uri}"))?;

        let expected = match map.get("qop") {
            Some(header_qop) => {
                if !header_qop.eq_ignore_ascii_case(qop) {
                    return Some(false);
                }
                let nc = map.get("nc")?;
                let cnonce = map.get("cnonce")?;
                hash(algorithm, &format!("{ha1}:{NONCE}:{nc}:{cnonce}:{header_qop}:{ha2}"))?
            }
            None => hash(algorithm, &format!("{ha1}:{NONCE}:{ha2}"))?,
        };

        Some(expected == *response)
    }
}
