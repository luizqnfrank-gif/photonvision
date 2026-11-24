use crate::camera::BasicSettables;
use crate::frame::{Frame, FrameProvider, FrameStaticProperties};
use crate::opencv_ext::CVMat;
use crate::processes::{VideoMode, VisionSource, VisionSourceSettables};
use image::{DynamicImage, ImageFormat, RgbImage};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use openh264::nal_units;
use photon_common::configuration::{
    CameraCalibrationCoefficients, CameraConfiguration, PVCameraInfo,
};
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Maximum buffered TCP bytes before discarding stale undecoded prefix (8 MiB).
pub const MAX_STREAM_BUFFER_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 400;
pub const DEFAULT_READ_TIMEOUT_MS: u64 = 500;
pub const DEFAULT_RECONNECT_BACKOFF_MS: u64 = 80;

/// Parsed TCP endpoint supporting both `rpicam-vid --listen -o tcp://0.0.0.0:8888` (outbound client)
/// and `tcp://0.0.0.0:8888?listen=true` (inbound listener).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpStreamEndpoint {
    pub raw_uri: String,
    pub host: String,
    pub port: u16,
    pub listen: bool,
}

impl TcpStreamEndpoint {
    pub fn parse(uri: &str) -> Result<Self, String> {
        let trimmed = uri.trim();
        if trimmed.is_empty() {
            return Err("TCP camera URI cannot be empty".to_string());
        }

        let without_scheme = trimmed.strip_prefix("tcp://").unwrap_or(trimmed);
        let (authority, query) = match without_scheme.split_once('?') {
            Some((a, q)) => (a, Some(q)),
            None => (without_scheme, None),
        };
        let authority = authority.trim_end_matches('/');

        let (host_part, port_str) = authority
            .rsplit_once(':')
            .ok_or_else(|| format!("missing ':port' in TCP camera URI '{trimmed}'"))?;

        let host = host_part
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim();
        if host.is_empty() {
            return Err(format!("empty host in TCP camera URI '{trimmed}'"));
        }

        let port: u16 = port_str
            .parse()
            .map_err(|_| format!("invalid port '{port_str}' in TCP camera URI '{trimmed}'"))?;
        if port == 0 {
            return Err("TCP port must be between 1 and 65535".to_string());
        }

        let listen = query
            .map(|q| {
                q.split('&')
                    .any(|kv| kv.eq_ignore_ascii_case("listen=true") || kv == "listen=1")
            })
            .unwrap_or(false);

        Ok(Self {
            raw_uri: trimmed.to_string(),
            host: host.to_string(),
            port,
            listen,
        })
    }

    pub fn socket_addr_string(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn resolve_first_addr(&self) -> Result<SocketAddr, String> {
        let addr_str = self.socket_addr_string();
        addr_str
            .to_socket_addrs()
            .map_err(|e| format!("failed to resolve '{addr_str}': {e}"))?
            .next()
            .ok_or_else(|| format!("no socket addresses resolved for '{addr_str}'"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpStreamCodec {
    Auto,
    H264AnnexB,
    Mjpeg,
}

/// Stateful stream parser and in-process decoder for `rpicam-vid --inline` H.264 Annex-B and MJPEG streams.
pub struct TcpStreamDecoder {
    buffer: Vec<u8>,
    detected_codec: TcpStreamCodec,
    h264_decoder: Option<Decoder>,
    seen_sps: bool,
}

impl Default for TcpStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpStreamDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(256 * 1024),
            detected_codec: TcpStreamCodec::Auto,
            h264_decoder: Decoder::new().ok(),
            seen_sps: false,
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.detected_codec = TcpStreamCodec::Auto;
        self.h264_decoder = Decoder::new().ok();
        self.seen_sps = false;
    }

    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }

    /// Push a chunk of raw TCP stream bytes and return any newly decoded `CVMat` frames.
    pub fn push_bytes(&mut self, chunk: &[u8]) -> Vec<CVMat> {
        if chunk.is_empty() {
            return Vec::new();
        }
        if self.buffer.len() + chunk.len() > MAX_STREAM_BUFFER_BYTES {
            self.buffer.clear();
            self.seen_sps = false;
        }
        self.buffer.extend_from_slice(chunk);

        if self.detected_codec == TcpStreamCodec::Auto {
            self.detected_codec = Self::detect_codec(&self.buffer);
        }

        match self.detected_codec {
            TcpStreamCodec::Mjpeg => self.extract_mjpeg_frames(),
            TcpStreamCodec::H264AnnexB => self.extract_h264_frames(),
            TcpStreamCodec::Auto => Vec::new(),
        }
    }

    fn detect_codec(buf: &[u8]) -> TcpStreamCodec {
        let mjpeg_pos = Self::find_jpeg_soi(buf);
        let h264_pos = Self::find_annex_b_start_code(buf, 0).map(|(idx, _)| idx);

        match (h264_pos, mjpeg_pos) {
            (Some(h), Some(j)) => {
                if h <= j {
                    TcpStreamCodec::H264AnnexB
                } else {
                    TcpStreamCodec::Mjpeg
                }
            }
            (Some(_), None) => TcpStreamCodec::H264AnnexB,
            (None, Some(_)) => TcpStreamCodec::Mjpeg,
            (None, None) => TcpStreamCodec::Auto,
        }
    }

    fn find_jpeg_soi(buf: &[u8]) -> Option<usize> {
        buf.windows(2).position(|w| w == [0xFF, 0xD8])
    }

    fn find_jpeg_eoi(buf: &[u8], start_after: usize) -> Option<usize> {
        if start_after >= buf.len() {
            return None;
        }
        buf[start_after..]
            .windows(2)
            .position(|w| w == [0xFF, 0xD9])
            .map(|rel| start_after + rel)
    }

    fn extract_mjpeg_frames(&mut self) -> Vec<CVMat> {
        let mut out = Vec::new();
        loop {
            let Some(soi) = Self::find_jpeg_soi(&self.buffer) else {
                // Keep at most trailing 1 byte if it's 0xFF
                if self.buffer.last() == Some(&0xFF) {
                    self.buffer.drain(..self.buffer.len() - 1);
                } else {
                    self.buffer.clear();
                }
                break;
            };
            if soi > 0 {
                self.buffer.drain(..soi);
            }
            let Some(eoi) = Self::find_jpeg_eoi(&self.buffer, 2) else {
                break;
            };
            let frame_end = eoi + 2;
            if let Ok(img) =
                image::load_from_memory_with_format(&self.buffer[..frame_end], ImageFormat::Jpeg)
            {
                out.push(CVMat::from_dynamic(img));
            }
            self.buffer.drain(..frame_end);
        }
        out
    }

    /// Finds the next H.264 Annex-B start code (`00 00 00 01` or `00 00 01`) starting at `from`.
    /// Returns `(start_index, start_code_length)`.
    fn find_annex_b_start_code(buf: &[u8], from: usize) -> Option<(usize, usize)> {
        if from + 3 > buf.len() {
            return None;
        }
        let mut i = from;
        while i + 2 < buf.len() {
            if buf[i] == 0 && buf[i + 1] == 0 {
                if buf[i + 2] == 1 {
                    return Some((i, 3));
                } else if i + 3 < buf.len() && buf[i + 2] == 0 && buf[i + 3] == 1 {
                    return Some((i, 4));
                }
            }
            i += 1;
        }
        None
    }

    fn extract_h264_frames(&mut self) -> Vec<CVMat> {
        let mut out = Vec::new();
        if self.h264_decoder.is_none() {
            self.h264_decoder = Decoder::new().ok();
        }

        loop {
            let Some((sc_pos, sc_len)) = Self::find_annex_b_start_code(&self.buffer, 0) else {
                // Retain up to 3 trailing bytes in case a start code spans TCP chunks
                let keep = self.buffer.len().min(3);
                let drain_to = self.buffer.len() - keep;
                if drain_to > 0 {
                    self.buffer.drain(..drain_to);
                }
                break;
            };
            if sc_pos > 0 {
                self.buffer.drain(..sc_pos);
            }
            // Look for the next start code to delimit a complete NAL unit
            let Some((next_sc_pos, _)) = Self::find_annex_b_start_code(&self.buffer, sc_len) else {
                break;
            };

            let nal_header_idx = sc_len;
            let nal_bytes: Vec<u8> = self.buffer.drain(..next_sc_pos).collect();
            if nal_header_idx < nal_bytes.len() {
                let nal_type = nal_bytes[nal_header_idx] & 0x1F;
                if nal_type == 7 {
                    self.seen_sps = true;
                }
                if self.seen_sps {
                    if let Some(frame) = Self::decode_h264_nal(&mut self.h264_decoder, &nal_bytes) {
                        out.push(frame);
                    }
                }
            }
        }

        out
    }

    /// Flush any trailing NAL unit already in the buffer (useful at stream EOF or after complete frame push).
    pub fn flush_remaining_h264(&mut self) -> Vec<CVMat> {
        let mut out = Vec::new();
        if self.detected_codec != TcpStreamCodec::H264AnnexB {
            return out;
        }
        if let Some((sc_pos, sc_len)) = Self::find_annex_b_start_code(&self.buffer, 0) {
            if sc_pos > 0 {
                self.buffer.drain(..sc_pos);
            }
            if sc_len < self.buffer.len() {
                let nal_type = self.buffer[sc_len] & 0x1F;
                if nal_type == 7 {
                    self.seen_sps = true;
                }
                if self.seen_sps {
                    let tail = std::mem::take(&mut self.buffer);
                    if let Some(frame) = Self::decode_h264_nal(&mut self.h264_decoder, &tail) {
                        out.push(frame);
                    }
                }
            }
        }
        out
    }

    fn decode_h264_nal(h264_decoder: &mut Option<Decoder>, annex_b_packet: &[u8]) -> Option<CVMat> {
        let dec = h264_decoder.as_mut()?;
        for packet in nal_units(annex_b_packet) {
            if let Ok(Some(yuv)) = dec.decode(packet) {
                let (w, h) = yuv.dimensions();
                if w > 0 && h > 0 {
                    let mut rgb_bytes = vec![0u8; w * h * 3];
                    yuv.write_rgb8(&mut rgb_bytes);
                    if let Some(rgb_img) = RgbImage::from_raw(w as u32, h as u32, rgb_bytes) {
                        return Some(CVMat::from_dynamic(DynamicImage::ImageRgb8(rgb_img)));
                    }
                }
            }
        }
        None
    }
}

/// Decoupled TCP frame provider for `rpicam-vid --inline --listen -o tcp://0.0.0.0:8888`.
pub struct TcpFrameProvider {
    name: String,
    endpoint: TcpStreamEndpoint,
    fov: f64,
    calibration: Option<CameraCalibrationCoefficients>,
    static_props: Arc<FrameStaticProperties>,
    latest_mat: Arc<RwLock<Option<CVMat>>>,
    connected: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    sequence_id: i64,
    worker_handle: Option<JoinHandle<()>>,
}

impl Drop for TcpFrameProvider {
    fn drop(&mut self) {
        self.stop();
    }
}

impl TcpFrameProvider {
    pub fn new(
        name: impl Into<String>,
        uri: &str,
        width: u32,
        height: u32,
        fov: f64,
        calibration: Option<CameraCalibrationCoefficients>,
    ) -> Self {
        let name_str = name.into();
        let endpoint = TcpStreamEndpoint::parse(uri).unwrap_or(TcpStreamEndpoint {
            raw_uri: uri.to_string(),
            host: "127.0.0.1".to_string(),
            port: 8888,
            listen: false,
        });
        let w = width.max(1);
        let h = height.max(1);
        let static_props = Arc::new(FrameStaticProperties::new(
            w as i32,
            h as i32,
            fov,
            calibration.clone(),
        ));
        let latest_mat = Arc::new(RwLock::new(None));
        let connected = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));

        let worker_handle = Self::spawn_reader_thread(
            endpoint.clone(),
            latest_mat.clone(),
            connected.clone(),
            running.clone(),
        );

        Self {
            name: name_str,
            endpoint,
            fov,
            calibration,
            static_props,
            latest_mat,
            connected,
            running,
            sequence_id: 0,
            worker_handle: Some(worker_handle),
        }
    }

    pub fn endpoint(&self) -> &TcpStreamEndpoint {
        &self.endpoint
    }

    pub fn has_frame(&self) -> bool {
        self.latest_mat.read().map(|g| g.is_some()).unwrap_or(false)
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.connected.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }

    fn spawn_reader_thread(
        endpoint: TcpStreamEndpoint,
        latest_mat: Arc<RwLock<Option<CVMat>>>,
        connected: Arc<AtomicBool>,
        running: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut decoder = TcpStreamDecoder::new();
            let mut read_buf = vec![0u8; 64 * 1024];

            let listener = if endpoint.listen {
                TcpListener::bind(endpoint.socket_addr_string()).ok()
            } else {
                None
            };
            if let Some(l) = &listener {
                let _ = l.set_nonblocking(true);
            }

            while running.load(Ordering::SeqCst) {
                let stream_opt = if let Some(l) = &listener {
                    match l.accept() {
                        Ok((s, _)) => Some(s),
                        Err(_) => {
                            thread::sleep(Duration::from_millis(DEFAULT_RECONNECT_BACKOFF_MS));
                            None
                        }
                    }
                } else if let Ok(addr) = endpoint.resolve_first_addr() {
                    TcpStream::connect_timeout(
                        &addr,
                        Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS),
                    )
                    .ok()
                } else {
                    None
                };

                let Some(mut stream) = stream_opt else {
                    connected.store(false, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(DEFAULT_RECONNECT_BACKOFF_MS));
                    continue;
                };

                let _ = stream.set_nonblocking(false);
                let _ =
                    stream.set_read_timeout(Some(Duration::from_millis(DEFAULT_READ_TIMEOUT_MS)));
                let _ = stream.set_nodelay(true);
                connected.store(true, Ordering::SeqCst);
                decoder.reset();

                while running.load(Ordering::SeqCst) {
                    match stream.read(&mut read_buf) {
                        Ok(0) => {
                            // Clean EOF from remote rpicam-vid server; flush any final NAL unit
                            let flushed = decoder.flush_remaining_h264();
                            if let Some(last_frame) = flushed.into_iter().next_back() {
                                if let Ok(mut guard) = latest_mat.write() {
                                    *guard = Some(last_frame);
                                }
                            }
                            break;
                        }
                        Ok(n) => {
                            let mut frames = decoder.push_bytes(&read_buf[..n]);
                            if frames.is_empty() && n < read_buf.len() {
                                // Check if a single trailing H.264 slice NAL is ready to decode
                                frames = decoder.flush_remaining_h264();
                            }
                            if let Some(last_frame) = frames.into_iter().next_back() {
                                if let Ok(mut guard) = latest_mat.write() {
                                    *guard = Some(last_frame);
                                }
                            }
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            let flushed = decoder.flush_remaining_h264();
                            if let Some(last_frame) = flushed.into_iter().next_back() {
                                if let Ok(mut guard) = latest_mat.write() {
                                    *guard = Some(last_frame);
                                }
                            }
                            continue;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }

                connected.store(false, Ordering::SeqCst);
                if running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(DEFAULT_RECONNECT_BACKOFF_MS));
                }
            }
        })
    }
}

impl FrameProvider for TcpFrameProvider {
    fn get_frame(&mut self) -> Frame {
        self.sequence_id += 1;
        let mat = self
            .latest_mat
            .read()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(|| {
                CVMat::new_rgb(
                    self.static_props.image_width as u32,
                    self.static_props.image_height as u32,
                )
            });

        // Dynamically update FrameStaticProperties if incoming stream resolution differs
        if (mat.width() as i32) != self.static_props.image_width
            || (mat.height() as i32) != self.static_props.image_height
        {
            self.static_props = Arc::new(FrameStaticProperties::new(
                mat.width() as i32,
                mat.height() as i32,
                self.fov,
                self.calibration.clone(),
            ));
        }

        Frame::from_single_mat(self.sequence_id, mat, self.static_props.clone())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

pub type TcpCameraSettables = BasicSettables;

/// Networked TCP camera source (`rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888`).
pub struct TcpCameraSource {
    config: CameraConfiguration,
    settables: TcpCameraSettables,
    frame_provider: TcpFrameProvider,
}

impl TcpCameraSource {
    pub fn new(config: CameraConfiguration) -> Self {
        let (uri, width, height, fps) = match &config.matched_camera_info {
            PVCameraInfo::Tcp {
                uri,
                width,
                height,
                fps,
                ..
            } => (
                uri.clone(),
                (*width).max(1) as u32,
                (*height).max(1) as u32,
                (*fps).max(1),
            ),
            other => (other.unique_path().to_string(), 1280, 720, 30),
        };

        let cal = config
            .calibration_for_resolution(width as i32, height as i32)
            .cloned()
            .or_else(|| config.calibrations.first().cloned());

        let frame_provider = TcpFrameProvider::new(
            config.nickname.clone(),
            &uri,
            width,
            height,
            config.fov,
            cal,
        );
        let mut settables = BasicSettables::new(config.clone());
        let _ = VideoMode {
            width: width as i32,
            height: height as i32,
            fps,
            pixel_format: "H264/MJPEG-TCP".to_string(),
        };
        settables.set_video_mode_index(1); // 1280x720 default mode

        Self {
            config,
            settables,
            frame_provider,
        }
    }

    pub fn tcp_frame_provider(&self) -> &TcpFrameProvider {
        &self.frame_provider
    }
}

impl VisionSource for TcpCameraSource {
    fn frame_provider(&mut self) -> &mut dyn FrameProvider {
        &mut self.frame_provider
    }
    fn settables(&self) -> &dyn VisionSourceSettables {
        &self.settables
    }
    fn settables_mut(&mut self) -> &mut dyn VisionSourceSettables {
        &mut self.settables
    }
    fn camera_configuration(&self) -> &CameraConfiguration {
        &self.config
    }
    fn camera_configuration_mut(&mut self) -> &mut CameraConfiguration {
        &mut self.config
    }
    fn is_vendor_camera(&self) -> bool {
        false
    }
    fn has_leds(&self) -> bool {
        false
    }
}
