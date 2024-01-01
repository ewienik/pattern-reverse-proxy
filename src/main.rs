use {
    axum::{
        body::Body,
        extract::State,
        http::{uri::Uri, Request},
        response::{IntoResponse, Response},
        routing::get,
        Router,
    },
    axum_server::tls_rustls::RustlsConfig,
    futures::stream,
    hyper::StatusCode,
    hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor},
    std::convert::Infallible,
    tower::ServiceBuilder,
    tower_http::compression::CompressionLayer,
};

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[tokio::main]
async fn main() {
    tokio::spawn(server());

    let client: Client = hyper_util::client::legacy::Client::builder(TokioExecutor::new())
        .build(HttpConnector::new());

    let app = Router::new()
        .route("/", get(handler))
        .layer(ServiceBuilder::new().layer(CompressionLayer::new()))
        .with_state(client);

    let cert = rcgen::generate_simple_self_signed(vec![
        "pattern.reverse.proxy".to_string(),
        "localhost".to_string(),
    ])
    .unwrap();
    let config = RustlsConfig::from_der(
        vec![cert.serialize_der().unwrap()],
        cert.serialize_private_key_der(),
    )
    .await
    .unwrap();

    tokio::spawn({
        let app = app.clone();
        async move {
            let addr = ([0, 0, 0, 0], 4443).into();
            println!("proxy https listening on {addr}");
            axum_server::bind_rustls(addr, config)
                .serve(app.into_make_service())
                .await
                .unwrap();
        }
    });
    let addr = ([0, 0, 0, 0], 4080).into();
    println!("proxy http listening on {addr}");
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(
    State(client): State<Client>,
    mut req: Request<Body>,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    let uri = format!("http://127.0.0.1:3000{path_query}");

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    Ok(client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}

async fn server() {
    let app = Router::new().route(
        "/",
        get(|| async {
            Body::from_stream(stream::repeat(Ok::<String, Infallible>(
                "Hello, world!\n".to_string(),
            )))
        }),
    );
    let addr = ([127, 0, 0, 1], 3000).into();
    println!("server listening on {addr}");
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
