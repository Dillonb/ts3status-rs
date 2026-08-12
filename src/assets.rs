use rocket::Route;
use rocket::http::Header;
use rocket::response::content::{RawCss, RawJavaScript};

/// Vendored copies of what the page used to load from CDNs, compiled into the
/// binary so the page works without third party requests. Sources:
///
/// - <https://cdn.jsdelivr.net/npm/bootstrap@4.0.0/dist/css/bootstrap.min.css>
/// - <https://cdn.jsdelivr.net/npm/bootstrap@4.0.0/dist/js/bootstrap.min.js>
/// - <https://code.jquery.com/jquery-3.7.1.min.js>
/// - <https://cdn.jsdelivr.net/npm/popper.js@1.12.9/dist/umd/popper.min.js>
/// - <https://cdn.jsdelivr.net/npm/handlebars@4.7.8/dist/handlebars.min.js>
#[derive(Responder)]
struct Asset<T> {
    body: T,
    // Safe to cache forever because every path names its version.
    cache_control: Header<'static>,
}

impl<T> Asset<T> {
    fn new(body: T) -> Self {
        Asset {
            body,
            cache_control: Header::new("Cache-Control", "public, max-age=31536000, immutable"),
        }
    }
}

#[get("/static/bootstrap-4.0.0.min.css")]
fn bootstrap_css() -> Asset<RawCss<&'static str>> {
    Asset::new(RawCss(include_str!("static/bootstrap-4.0.0.min.css")))
}

#[get("/static/bootstrap-4.0.0.min.js")]
fn bootstrap_js() -> Asset<RawJavaScript<&'static str>> {
    Asset::new(RawJavaScript(include_str!("static/bootstrap-4.0.0.min.js")))
}

#[get("/static/jquery-3.7.1.min.js")]
fn jquery_js() -> Asset<RawJavaScript<&'static str>> {
    Asset::new(RawJavaScript(include_str!("static/jquery-3.7.1.min.js")))
}

#[get("/static/popper-1.12.9.min.js")]
fn popper_js() -> Asset<RawJavaScript<&'static str>> {
    Asset::new(RawJavaScript(include_str!("static/popper-1.12.9.min.js")))
}

#[get("/static/handlebars-4.7.8.min.js")]
fn handlebars_js() -> Asset<RawJavaScript<&'static str>> {
    Asset::new(RawJavaScript(include_str!(
        "static/handlebars-4.7.8.min.js"
    )))
}

pub fn routes() -> Vec<Route> {
    routes![
        bootstrap_css,
        bootstrap_js,
        jquery_js,
        popper_js,
        handlebars_js
    ]
}
