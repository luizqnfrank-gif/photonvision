use crate::opencv_ext::CVMat;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::watch;

/// Saves requested single-frame snapshots to disk (`org.photonvision.vision.frame.consumer.FileSaveFrameConsumer`).
#[derive(Clone)]
pub struct FileSaveFrameConsumer {
    camera_nickname: String,
    stream_type: String,
    save_dir: PathBuf,
    save_requested: Arc<AtomicBool>,
    saved_count: Arc<AtomicUsize>,
}

impl FileSaveFrameConsumer {
    pub fn new(
        camera_nickname: impl Into<String>,
        stream_type: impl Into<String>,
        save_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            camera_nickname: camera_nickname.into(),
            stream_type: stream_type.into(),
            save_dir: save_dir.into(),
            save_requested: Arc::new(AtomicBool::new(false)),
            saved_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn save_one_frame(&self) {
        self.save_requested.store(true, Ordering::SeqCst);
    }

    pub fn saved_count(&self) -> usize {
        self.saved_count.load(Ordering::SeqCst)
    }

    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    pub fn accept(&self, image: &CVMat) -> Option<PathBuf> {
        if self.save_requested.swap(false, Ordering::SeqCst) && !image.is_empty() {
            let idx = self.saved_count.fetch_add(1, Ordering::SeqCst) + 1;
            let file_name = format!("{}_{}_{}.png", self.camera_nickname, self.stream_type, idx);
            let out_path = self.save_dir.join(file_name);
            if image.save_to_path(&out_path).is_ok() {
                return Some(out_path);
            }
        }
        None
    }
}

/// Publishes JPEG-encoded frames to a lock-free `watch` channel for MJPEG streaming (`MJPGFrameConsumer`).
#[derive(Clone)]
pub struct MJPGFrameConsumer {
    name: String,
    port: u16,
    tx: Arc<watch::Sender<Vec<u8>>>,
    rx: watch::Receiver<Vec<u8>>,
}

impl MJPGFrameConsumer {
    pub fn new(name: impl Into<String>, port: u16) -> Self {
        let (tx, rx) = watch::channel(Vec::new());
        Self {
            name: name.into(),
            port,
            tx: Arc::new(tx),
            rx,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn subscribe(&self) -> watch::Receiver<Vec<u8>> {
        self.rx.clone()
    }

    pub fn accept(&self, image: &CVMat) {
        if !image.is_empty() {
            let jpeg = image.encode_jpeg();
            let _ = self.tx.send(jpeg);
        }
    }
}
