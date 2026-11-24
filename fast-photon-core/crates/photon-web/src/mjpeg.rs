use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Response, StatusCode};
use axum::routing::get;
use axum::Router;
use bytes::Bytes;
use futures_util::stream;
use tokio::sync::watch;

pub fn format_mjpeg_frame(jpeg_bytes: &[u8]) -> Bytes {
    let mut buf = Vec::with_capacity(jpeg_bytes.len() + 128);
    let header_str = format!(
        "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
        jpeg_bytes.len()
    );
    buf.extend_from_slice(header_str.as_bytes());
    buf.extend_from_slice(jpeg_bytes);
    buf.extend_from_slice(b"\r\n");
    Bytes::from(buf)
}

async fn mjpeg_stream_handler(State(rx): State<watch::Receiver<Vec<u8>>>) -> Response<Body> {
    let initial = rx.borrow().clone();
    let s = stream::unfold((rx, Some(initial)), |(mut rx, first)| async move {
        if let Some(init_bytes) = first {
            if !init_bytes.is_empty() {
                return Some((
                    Ok::<Bytes, std::convert::Infallible>(format_mjpeg_frame(&init_bytes)),
                    (rx, None),
                ));
            }
        }
        if rx.changed().await.is_ok() {
            let jpeg = rx.borrow_and_update().clone();
            if !jpeg.is_empty() {
                return Some((
                    Ok::<Bytes, std::convert::Infallible>(format_mjpeg_frame(&jpeg)),
                    (rx, None),
                ));
            }
        }
        None
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, private")
        .body(Body::from_stream(s))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Builds an Axum MJPEG router serving `/stream.mjpg` and `/` for a given `MJPGFrameConsumer` receiver.
pub fn build_mjpeg_router(rx: watch::Receiver<Vec<u8>>) -> Router {
    Router::new()
        .route("/", get(mjpeg_stream_handler))
        .route("/stream.mjpg", get(mjpeg_stream_handler))
        .with_state(rx)
}
