macro_rules! server_error {
    ($($arg:tt)+) => {
        error!(target: "{server}", $($arg)+)
    }
}

macro_rules! server_warn {
    ($($arg:tt)+) => {
        warn!(target: "{server}", $($arg)+)
    }
}

macro_rules! server_info {
    ($($arg:tt)+) => {
        info!(target: "{server}", $($arg)+)
    }
}

mod auth;
mod error;

pub use auth::{
    AuthenticationStandby, AuthenticationStandbyError, WaitForOsuAuth, WaitForTwitchAuth,
};
pub use error::ServerError;

use eyre::Report;
use handlebars::Handlebars;
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    header::{AUTHORIZATION, CONTENT_TYPE, LOCATION},
    server::Server,
    Body, Client as HyperClient, Request, Response, StatusCode,
};
use hyper_rustls::HttpsConnector;
use prometheus::{Encoder, TextEncoder};
use rosu_v2::Osu;
use routerify::{ext::RequestExt, Middleware, RouteError, Router, RouterService};
use serde_json::json;
use std::{env, fmt, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{fs::File, io::AsyncReadExt, sync::oneshot};

use crate::{
    error::TwitchError,
    twitch::{OAuthToken, TwitchData, TwitchUser},
    util::constants::{GENERAL_ISSUE, TWITCH_OAUTH, TWITCH_USERS_ENDPOINT},
    Context, CONFIG,
};

pub async fn run_server(ctx: Arc<Context>, shutdown_rx: oneshot::Receiver<()>) {
    if cfg!(debug_assertions) {
        info!("Skip server on debug");

        return;
    }

    let ip = CONFIG.get().unwrap().server.internal_ip;
    let port = CONFIG.get().unwrap().server.internal_port;
    let addr = SocketAddr::from((ip, port));
    let router = router(ctx);

    let service = RouterService::new(router).expect("failed to create RouterService");

    let server = Server::bind(&addr)
        .serve(service)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

    info!("Running server...");

    if let Err(why) = server.await {
        error!("{:?}", Report::new(why).wrap_err("server failed"));
    }
}

struct Client(HyperClient<HttpsConnector<HttpConnector<GaiResolver>>, Body>);

struct Context_(Arc<Context>);
struct Handlebars_(Handlebars<'static>);

struct OsuClientId(u64);
struct OsuClientSecret(String);
struct OsuRedirect(String);

struct TwitchClientId(String);
struct TwitchClientSecret(String);
struct TwitchRedirect(String);

fn router(ctx: Arc<Context>) -> Router<Body, ServerError> {
    let connector = HttpsConnector::with_native_roots();
    let client = HyperClient::builder().build(connector);
    let config = CONFIG.get().unwrap();

    let osu_client_id = config.tokens.osu_client_id;
    let osu_client_secret = config.tokens.osu_client_secret.to_owned();

    let twitch_client_id = config.tokens.twitch_client_id.to_owned();
    let twitch_client_secret = config.tokens.twitch_token.to_owned();

    let url = &config.server.external_url;
    let osu_redirect = format!("{}/auth/osu", url);
    let twitch_redirect = format!("{}/auth/twitch", url);

    let mut handlebars = Handlebars::new();
    let env_var = env::var("WEBSITE_PATH").expect("missing env variable `WEBSITE_PATH`");
    let mut path = PathBuf::from(env_var);
    path.push("auth.hbs");

    handlebars
        .register_template_file("auth", path)
        .expect("failed to register auth template to handlebars");

    Router::builder()
        .data(Client(client))
        .data(Context_(ctx))
        .data(Handlebars_(handlebars))
        .data(OsuClientId(osu_client_id))
        .data(OsuClientSecret(osu_client_secret))
        .data(OsuRedirect(osu_redirect))
        .data(TwitchClientId(twitch_client_id))
        .data(TwitchClientSecret(twitch_client_secret))
        .data(TwitchRedirect(twitch_redirect))
        .middleware(Middleware::pre(logger))
        .get("/metrics", metrics_handler)
        .get("/auth/osu", auth_osu_handler)
        .get("/auth/twitch", auth_twitch_handler)
        .get("/auth/auth.css", auth_css_handler)
        .get("/auth/icon.svg", auth_icon_handler)
        .get("/osudirect/:mapset_id", osudirect_handler)
        .any(handle_404)
        .err_handler(error_handler)
        .build()
        .expect("failed to build router")
}

async fn logger(req: Request<Body>) -> Result<Request<Body>, ServerError> {
    server_info!(
        "{} {} {}",
        req.remote_addr(),
        req.method(),
        req.uri().path()
    );

    Ok(req)
}

// Required to pass RouteError to Report
#[derive(Debug)]
struct ErrorWrapper(RouteError);

impl std::error::Error for ErrorWrapper {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl fmt::Display for ErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

async fn error_handler(err: RouteError) -> Response<Body> {
    let report = Report::new(ErrorWrapper(err)).wrap_err("error while handling server request");
    server_error!("{:?}", report);

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(GENERAL_ISSUE))
        .unwrap()
}

type HandlerResult = Result<Response<Body>, ServerError>;

async fn handle_404(_req: Request<Body>) -> HandlerResult {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("404 Not Found"))?;

    Ok(response)
}

async fn metrics_handler(req: Request<Body>) -> HandlerResult {
    let mut buf = Vec::new();
    let encoder = TextEncoder::new();
    let Context_(ctx) = req.data().unwrap();
    let metric_families = ctx.stats.registry.gather();
    encoder.encode(&metric_families, &mut buf).unwrap();

    Ok(Response::new(Body::from(buf)))
}

async fn auth_osu_handler(req: Request<Body>) -> HandlerResult {
    match auth_osu_handler_(&req).await {
        Ok(response) => Ok(response),
        Err(why) => {
            server_warn!("{:?}", Report::new(why).wrap_err("osu! auth failed"));

            let render_data = json!({
                "body_id": "error",
                "error": GENERAL_ISSUE,
            });

            let Handlebars_(handlebars) = req.data().unwrap();
            let page = handlebars.render("auth", &render_data)?;

            let response = Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(page))?;

            Ok(response)
        }
    }
}

async fn auth_osu_handler_(req: &Request<Body>) -> HandlerResult {
    let query = req.uri().query();

    let code = match query.and_then(|q| q.split('&').find(|q| q.starts_with("code="))) {
        Some(query) => &query[5..],
        None => return invalid_auth_query(req),
    };

    let id_opt = query.and_then(|q| {
        q.split('&')
            .find(|q| q.starts_with("state="))
            .map(|q| q[6..].parse())
    });

    let id = match id_opt {
        Some(Ok(state)) => state,
        None | Some(Err(_)) => return invalid_auth_query(req),
    };

    let Context_(ctx) = req.data().unwrap();

    if ctx.auth_standby.is_osu_empty() {
        return unexpected_auth(req);
    }

    let OsuClientId(client_id) = req.data().unwrap();
    let OsuClientSecret(client_secret) = req.data().unwrap();
    let OsuRedirect(redirect) = req.data().unwrap();

    let osu = Osu::builder()
        .client_id(*client_id)
        .client_secret(client_secret)
        .with_authorization(code, redirect)
        .build()
        .await?;

    let user = osu.own_data().await?;

    let render_data = json!({
        "body_id": "success",
        "kind": "osu!",
        "name": user.username,
    });

    let Handlebars_(handlebars) = req.data().unwrap();
    let page = handlebars.render("auth", &render_data)?;

    info!(target: "{server,_Default}", "Successful osu! authorization for `{}`", user.username);

    ctx.auth_standby.process_osu(user, id);

    Ok(Response::new(Body::from(page)))
}

async fn auth_twitch_handler(req: Request<Body>) -> HandlerResult {
    match auth_twitch_handler_(&req).await {
        Ok(response) => Ok(response),
        Err(why) => {
            server_warn!("{:?}", Report::new(why).wrap_err("twitch auth failed"));

            let render_data = json!({
                "body_id": "error",
                "error": GENERAL_ISSUE,
            });

            let Handlebars_(handlebars) = req.data().unwrap();
            let page = handlebars.render("auth", &render_data)?;

            let response = Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(page))?;

            Ok(response)
        }
    }
}

async fn auth_twitch_handler_(req: &Request<Body>) -> HandlerResult {
    let query = req.uri().query();

    let code = match query.and_then(|q| q.split('&').find(|q| q.starts_with("code="))) {
        Some(query) => &query[5..],
        None => return invalid_auth_query(req),
    };

    let id_opt = query.and_then(|q| {
        q.split('&')
            .find(|q| q.starts_with("state="))
            .map(|q| q[6..].parse())
    });

    let id = match id_opt {
        Some(Ok(state)) => state,
        None | Some(Err(_)) => return invalid_auth_query(req),
    };

    let Context_(ctx) = req.data().unwrap();

    if ctx.auth_standby.is_twitch_empty() {
        return unexpected_auth(req);
    }

    let TwitchClientId(client_id) = req.data().unwrap();
    let TwitchClientSecret(client_secret) = req.data().unwrap();
    let TwitchRedirect(redirect) = req.data().unwrap();
    let Client(client) = req.data().unwrap();

    let req_uri = format!(
        "{}?client_id={}&client_secret={}\
        &code={}&grant_type=authorization_code&redirect_uri={}",
        TWITCH_OAUTH, client_id, client_secret, code, redirect
    );

    let token_req = Request::post(req_uri).body(Body::empty())?;

    let response = client
        .request(token_req)
        .await
        .map_err(TwitchError::Hyper)?;

    let bytes = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(TwitchError::Hyper)?;

    let token = serde_json::from_slice::<OAuthToken>(&bytes)
        .map_err(|source| {
            let content = String::from_utf8_lossy(&bytes).into_owned();

            TwitchError::SerdeToken { source, content }
        })
        .map(|token| format!("Bearer {}", token))?;

    let user_req = Request::get(TWITCH_USERS_ENDPOINT)
        .header(AUTHORIZATION, token)
        .header("Client-ID", client_id)
        .body(Body::empty())?;

    let response = client.request(user_req).await.map_err(TwitchError::Hyper)?;

    let bytes = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(TwitchError::Hyper)?;

    let user = serde_json::from_slice::<TwitchData<TwitchUser>>(&bytes)
        .map_err(|source| {
            let content = String::from_utf8_lossy(&bytes).into_owned();

            TwitchError::SerdeUser { source, content }
        })
        .map(|mut data| data.data.pop())?
        .ok_or(TwitchError::NoUser)?;

    let render_data = json!({
        "body_id": "success",
        "kind": "twitch",
        "name": user.display_name,
    });

    let Handlebars_(handlebars) = req.data().unwrap();
    let page = handlebars.render("auth", &render_data)?;

    info!(
        target: "{server,_Default}",
        "Successful twitch authorization for `{}`",
        user.display_name
    );

    ctx.auth_standby.process_twitch(user, id);

    Ok(Response::new(Body::from(page)))
}

async fn auth_css_handler(_: Request<Body>) -> HandlerResult {
    let mut path = PathBuf::from(env::var("WEBSITE_PATH")?);
    path.push("auth.css");
    let mut buf = Vec::with_capacity(1824);
    File::open(path).await?.read_to_end(&mut buf).await?;

    Ok(Response::new(Body::from(buf)))
}

async fn auth_icon_handler(_: Request<Body>) -> HandlerResult {
    let mut path = PathBuf::from(env::var("WEBSITE_PATH")?);
    path.push("icon.svg");
    let mut buf = Vec::with_capacity(11_198);
    File::open(path).await?.read_to_end(&mut buf).await?;

    let response = Response::builder()
        .header(CONTENT_TYPE, "image/svg+xml")
        .body(Body::from(buf))?;

    Ok(response)
}

async fn osudirect_handler(req: Request<Body>) -> HandlerResult {
    let mapset_id: u32 = match req.param("mapset_id").map(|id| id.parse()) {
        Some(Ok(id)) => id,
        Some(Err(_)) | None => {
            let content = "The path following '/osudirect/' must be a numeric mapset id";

            let response = Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(content))?;

            return Ok(response);
        }
    };

    let location = format!("osu://dl/{}", mapset_id);

    let response = Response::builder()
        .status(StatusCode::PERMANENT_REDIRECT)
        .header(LOCATION, location)
        .body(Body::empty())?;

    Ok(response)
}

fn invalid_auth_query(req: &Request<Body>) -> HandlerResult {
    let render_data = json!({
        "body_id": "error",
        "error": "Invalid query",
    });

    let Handlebars_(handlebars) = req.data().unwrap();
    let page = handlebars.render("auth", &render_data)?;

    let response = Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(page))?;

    Ok(response)
}

fn unexpected_auth(req: &Request<Body>) -> HandlerResult {
    let render_data = json!({
        "body_id": "error",
        "error": "Did not expect authentication. Be sure you use the bot command first.",
    });

    let Handlebars_(handlebars) = req.data().unwrap();
    let page = handlebars.render("auth", &render_data)?;

    let response = Response::builder()
        .status(StatusCode::PRECONDITION_FAILED)
        .body(Body::from(page))?;

    Ok(response)
}
