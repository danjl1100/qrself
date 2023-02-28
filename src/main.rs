//! Binary crate for qrcode generator server, referencing only itself

// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use hyper::{
    http::HeaderValue,
    service::{make_service_fn, service_fn},
    Body, Request, Server,
};
use qrcode::QrCode;
use std::convert::Infallible;

const ENV_BIND_ADDRESS: &str = "BIND_ADDRESS";

#[tokio::main]
async fn main() {
    let bind_address_env = std::env::var(ENV_BIND_ADDRESS).ok();

    // avoid a dependency on `clap`, this is slightly painful but works..?
    let mut args = std::env::args();
    let exec_name = args.next();
    let bind_address_arg = args.next();
    if let Some(extra_arg) = args.next() {
        eprintln!("Unexpected arg {extra_arg:?}");
        return;
    }

    let bind_address_str = bind_address_arg.or(bind_address_env);
    let Some(bind_address_str) = bind_address_str else {
        let exec_name = exec_name.unwrap_or_else(|| "<unknown>".into());
        eprintln!("Expected bind address");
        eprintln!("USAGE: {exec_name} BIND_ADDRESS");
        eprintln!("   OR env-arg BIND_ADDRESS");
        return;
    };
    let bind_address = match bind_address_str.parse() {
        Ok(bind_address) => bind_address,
        Err(error) => {
            println!("ERROR: invalid bind address {bind_address_str:?}: {error}");
            return;
        }
    };

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(qrself_service)) });
    let server = Server::bind(&bind_address).serve(make_svc);
    println!("Serving qrself at {bind_address} ...");

    if let Err(e) = server.await {
        eprintln!("ERROR: server error: {e}");
    }
}

const VALID_HEADERS: &str = "valid static headers";
#[allow(clippy::unused_async)] // `service_fn` requires an async function
async fn qrself_service(req: Request<Body>) -> Result<hyper::Response<Body>, Infallible> {
    let response = if let Some(render_ty) = parse_render_type(&req) {
        let url = rebuild_request_url(&req);
        match QrCode::new(&url) {
            Ok(qr_code) => render::render(&qr_code, render_ty),
            Err(e) => response_builder()
                .body(format!("{{error:{e:?},url:{url:?}}}").into())
                .expect(VALID_HEADERS),
        }
    } else {
        response_builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body("".into())
            .expect(VALID_HEADERS)
    };
    Ok(response)
}
fn truncate_str(s: &str) -> &str {
    const TRUNCATE_LEN: usize = 1024;
    &s[0..(s.len().min(TRUNCATE_LEN))]
}
fn response_builder() -> hyper::http::response::Builder {
    hyper::Response::builder().header("X-Robots-Tag", HeaderValue::from_static("noindex"))
}
fn rebuild_request_url(req: &Request<Body>) -> String {
    let uri = req.uri();
    let host = req
        .headers()
        .get("host")
        .map(|header_value| String::from_utf8_lossy(header_value.as_bytes()))
        .unwrap_or_default();
    format!("http://{host}{uri}")
}

/// Returns the type for rendering to match the request, or `None` for disallowed paths
fn parse_render_type(req: &Request<Body>) -> Option<render::Type> {
    use render::Type;
    const DISALLOW_URIS: &[&str] = &["/favicon.ico", "/robots.txt"];
    let uri_path = req.uri().path();
    let uri_path = truncate_str(uri_path);
    if DISALLOW_URIS.contains(&uri_path) {
        return None;
    }
    let accept_type = parse_accept_type(req);
    let ty = match accept_type {
        // Restrictive
        Some(ty @ (Type::Utf8Text | Type::Image)) => ty,
        // Permissive
        Some(ty @ Type::ImageHtmlEmbed) => parse_extension_type(uri_path).unwrap_or(ty),
        None => parse_extension_type(uri_path).unwrap_or_default(),
    };
    Some(ty)
}
/// Returns the render type for the first recognized Accept header MIME type (if any)
fn parse_accept_type(req: &Request<Body>) -> Option<render::Type> {
    use render::Type;
    const MIME_DELIMITER: char = ',';
    const MIME_HTML: &[&str] = &["text/html"];
    const MIME_IMAGE: &[&str] = &["image/png", "image/*"];
    const MIME_ALL: &[&str] = &["*/*"];
    let accept_header = req
        .headers()
        .get("accept")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let accept_header = truncate_str(accept_header);
    let accept_types = accept_header.split(MIME_DELIMITER).map(str::trim);
    for accept in accept_types {
        if MIME_HTML.contains(&accept) {
            return Some(Type::ImageHtmlEmbed);
        }
        if MIME_IMAGE.contains(&accept) {
            return Some(Type::Image);
        }
        if MIME_ALL.contains(&accept) {
            return Some(Type::Utf8Text);
        }
    }
    None
}
/// Returns the non-text type based on the filename
///
/// NOTE: Excludes text, as usually only console (`accept_type` = text) will format text as monospace
fn parse_extension_type(uri_path: &str) -> Option<render::Type> {
    use render::Type;
    const PATH_DELIMITER: char = '/';
    const EXT_DELIMITER: char = '.';
    const EXT_HTML: &[&str] = &["htm", "html"];
    const EXT_IMAGE: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];
    // Find filename, after past path delimiter (/)
    let file_path = uri_path.split(PATH_DELIMITER).last()?;
    // Find extensionm, after last extension delimiter (.)
    let extension = file_path.split(EXT_DELIMITER).map(str::trim).last()?;
    if EXT_HTML.contains(&extension) {
        Some(Type::ImageHtmlEmbed)
    } else if EXT_IMAGE.contains(&extension) {
        Some(Type::Image)
    } else {
        None
    }
}

mod render {
    use base64::{engine::general_purpose, Engine as _};
    use hyper::{http::HeaderValue, Body};
    use image::ImageBuffer;
    use qrcode::QrCode;
    use std::io::Cursor;

    use crate::{response_builder, VALID_HEADERS};

    static HEADER_CONTENT_TYPE_HTML: HeaderValue =
        HeaderValue::from_static("text/html; charset=UTF-8");

    #[derive(Clone, Copy, Debug, Default)]
    pub enum Type {
        Utf8Text,
        #[default]
        Image,
        ImageHtmlEmbed,
    }
    pub fn render(qr_code: &QrCode, ty: Type) -> hyper::Response<Body> {
        match ty {
            Type::Utf8Text => utf8_text(qr_code),
            Type::Image => image_png(qr_code),
            Type::ImageHtmlEmbed => html_embedded(qr_code),
        }
    }

    fn utf8_text(qr_code: &QrCode) -> hyper::Response<Body> {
        let ascii = qr_code
            .render()
            .module_dimensions(2, 1)
            .light_color(" ")
            .dark_color("\u{2588}")
            .build();
        response_builder()
            .header("Content-Type", HEADER_CONTENT_TYPE_HTML.clone())
            .body(ascii.into())
            .expect(VALID_HEADERS)
    }
    fn html_embedded(qr_code: &QrCode) -> hyper::Response<Body> {
        let image_bytes = qrcode_image_bytes(qr_code, image::ImageOutputFormat::Png);
        let image_bytes_base64 = general_purpose::STANDARD_NO_PAD.encode(image_bytes);
        let body = html_body_b64_image(&image_bytes_base64);
        response_builder()
            .header("Content-Type", HEADER_CONTENT_TYPE_HTML.clone())
            .body(body.into())
            .expect(VALID_HEADERS)
    }
    fn image_png(qr_code: &QrCode) -> hyper::Response<Body> {
        let image_bytes = qrcode_image_bytes(qr_code, image::ImageOutputFormat::Png);
        response_builder()
            .header("Content-Type", HeaderValue::from_static("image/png"))
            .body(image_bytes.into())
            .expect(VALID_HEADERS)
    }
    fn qrcode_image_bytes(qr_code: &QrCode, format: image::ImageOutputFormat) -> Vec<u8> {
        const WHITE: [u8; 1] = [u8::MAX];
        const BLACK: [u8; 1] = [u8::MIN];
        use image_at_qrcode_version::Luma as OldLuma;
        let image = {
            let image = qr_code
                .render()
                // minimal size, enlarge client-side via CSS
                .module_dimensions(1, 1)
                // invert colors
                .dark_color(OldLuma(WHITE))
                .light_color(OldLuma(BLACK))
                .build();
            // abuse "from_raw" to interoperate between different version of `image` crate
            let width = image.width();
            let height = image.height();
            let image: ImageBuffer<image::Luma<u8>, _> =
                ImageBuffer::from_raw(width, height, image)
                    .expect("size correct for lib conversion");
            image
        };
        // serialize image to bytes
        let mut image_bytes_cursor = Cursor::new(Vec::new());
        image
            .write_to(&mut image_bytes_cursor, format)
            .expect("cursor write is infallible");
        image_bytes_cursor.into_inner()
    }

    fn html_body_b64_image(image_bytes_base64: &str) -> String {
        format!(
            "<!DOCTYPE html>
<html>
<head>
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
    <title>Redirecting to target</title>
    <style>
    body {{
        overflow: hidden;
        background-color: black;
    }}
    div {{
        display: flex;
        height: 100vh;
        align-items: center;
        justify-content: center;
    }}
    img {{
        width: 100vmin;
        height: 100vmin;
        image-rendering: pixelated;
    }}
    </style>
</head>
<body>
    <div>
        <img src=\"data:image/png;base64,{image_bytes_base64}\" alt=\"the QR code to scan\" />
    </div>
</body>
</html>"
        )
    }
}
