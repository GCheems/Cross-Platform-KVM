use crate::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Clipboard content types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipboardContent {
    Text(String),
    Image(Vec<u8>), // PNG format
    Empty,
}

impl ClipboardContent {
    /// Calculate the size of the content in bytes
    pub fn size_bytes(&self) -> usize {
        match self {
            ClipboardContent::Text(text) => text.len(),
            ClipboardContent::Image(data) => data.len(),
            ClipboardContent::Empty => 0,
        }
    }

    /// Calculate the size of the content in megabytes
    pub fn size_mb(&self) -> f64 {
        self.size_bytes() as f64 / (1024.0 * 1024.0)
    }

    /// Calculate SHA256 hash of the content
    pub fn hash(&self) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        match self {
            ClipboardContent::Text(text) => hasher.update(text.as_bytes()),
            ClipboardContent::Image(data) => hasher.update(data),
            ClipboardContent::Empty => hasher.update(b"empty"),
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Clipboard change event
#[derive(Debug, Clone)]
pub struct ClipboardEvent {
    pub content: ClipboardContent,
    pub timestamp: u64,
}

/// Trait for clipboard service
/// Monitors and synchronizes clipboard content between devices
#[async_trait::async_trait]
pub trait ClipboardService: Send + Sync {
    /// Get the current clipboard content
    async fn get_content(&self) -> Result<ClipboardContent>;

    /// Set the clipboard content
    async fn set_content(&self, content: ClipboardContent) -> Result<()>;

    /// Subscribe to clipboard change events
    fn subscribe(&self) -> Receiver<ClipboardEvent>;

    /// Start monitoring clipboard changes
    async fn start_monitoring(&self) -> Result<()>;

    /// Stop monitoring clipboard changes
    async fn stop_monitoring(&self) -> Result<()>;
}

/// Implementation of clipboard service
pub struct ClipboardServiceImpl {
    clipboard: Arc<Mutex<arboard::Clipboard>>,
    last_hash: Arc<Mutex<Option<String>>>,
    subscribers: Arc<Mutex<Vec<Sender<ClipboardEvent>>>>,
    monitoring: Arc<Mutex<bool>>,
    size_limit_mb: u64,
    /// Cache of recently synced content to avoid redundant transfers
    sync_cache: Arc<Mutex<std::collections::HashMap<String, ClipboardContent>>>,
}

impl ClipboardServiceImpl {
    /// Create a new clipboard service
    pub fn new(size_limit_mb: u64) -> Result<Self> {
        let clipboard = arboard::Clipboard::new()
            .map_err(|e| crate::KvmError::Clipboard(format!("Failed to initialize clipboard: {}", e)))?;
        
        Ok(Self {
            clipboard: Arc::new(Mutex::new(clipboard)),
            last_hash: Arc::new(Mutex::new(None)),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            monitoring: Arc::new(Mutex::new(false)),
            size_limit_mb,
            sync_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }

    /// Check if content has already been synced (incremental sync optimization)
    pub async fn is_already_synced(&self, content: &ClipboardContent) -> bool {
        let hash = content.hash();
        let cache = self.sync_cache.lock().await;
        cache.contains_key(&hash)
    }

    /// Mark content as synced
    pub async fn mark_as_synced(&self, content: ClipboardContent) {
        let hash = content.hash();
        let mut cache = self.sync_cache.lock().await;
        
        // Keep cache size limited (max 100 entries)
        if cache.len() >= 100 {
            // Remove oldest entry (simple FIFO)
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        
        cache.insert(hash, content);
    }

    /// Clear the sync cache
    pub async fn clear_sync_cache(&self) {
        let mut cache = self.sync_cache.lock().await;
        cache.clear();
    }

    /// Check if content exceeds size limit and needs confirmation
    pub fn needs_confirmation(&self, content: &ClipboardContent) -> bool {
        content.size_mb() > self.size_limit_mb as f64
    }

    /// Poll clipboard for changes
    async fn poll_clipboard(&self) -> Result<Option<ClipboardContent>> {
        let mut clipboard = self.clipboard.lock().await;
        
        // Try to get text first
        if let Ok(text) = clipboard.get_text() {
            let content = ClipboardContent::Text(text);
            let hash = content.hash();
            
            let mut last_hash = self.last_hash.lock().await;
            if last_hash.as_ref() != Some(&hash) {
                *last_hash = Some(hash);
                return Ok(Some(content));
            }
        }
        
        // Try to get image
        if let Ok(image_data) = clipboard.get_image() {
            // Convert to PNG format
            let png_data = Self::convert_to_png(&image_data)?;
            let content = ClipboardContent::Image(png_data);
            let hash = content.hash();
            
            let mut last_hash = self.last_hash.lock().await;
            if last_hash.as_ref() != Some(&hash) {
                *last_hash = Some(hash);
                return Ok(Some(content));
            }
        }
        
        Ok(None)
    }

    /// Convert image data to PNG format
    fn convert_to_png(image_data: &arboard::ImageData) -> Result<Vec<u8>> {
        use image::{ImageBuffer, RgbaImage};
        
        let img: RgbaImage = ImageBuffer::from_raw(
            image_data.width as u32,
            image_data.height as u32,
            image_data.bytes.to_vec(),
        ).ok_or_else(|| crate::KvmError::Clipboard("Failed to create image buffer".to_string()))?;
        
        let mut png_data = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_data), image::ImageOutputFormat::Png)
            .map_err(|e| crate::KvmError::Clipboard(format!("Failed to encode PNG: {}", e)))?;
        
        Ok(png_data)
    }

    /// Convert PNG data to ImageData
    fn png_to_image_data(png_data: &[u8]) -> Result<arboard::ImageData<'static>> {
        use image::io::Reader as ImageReader;
        
        let img = ImageReader::new(std::io::Cursor::new(png_data))
            .with_guessed_format()
            .map_err(|e| crate::KvmError::Clipboard(format!("Failed to read image: {}", e)))?
            .decode()
            .map_err(|e| crate::KvmError::Clipboard(format!("Failed to decode image: {}", e)))?;
        
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        
        Ok(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: rgba.into_raw().into(),
        })
    }

    /// Notify all subscribers of a clipboard change
    async fn notify_subscribers(&self, content: ClipboardContent) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let event = ClipboardEvent { content, timestamp };
        
        let mut subscribers = self.subscribers.lock().await;
        subscribers.retain(|sender| {
            sender.try_send(event.clone()).is_ok()
        });
    }
}

#[async_trait::async_trait]
impl ClipboardService for ClipboardServiceImpl {
    async fn get_content(&self) -> Result<ClipboardContent> {
        let mut clipboard = self.clipboard.lock().await;
        
        // Try to get text first
        if let Ok(text) = clipboard.get_text() {
            return Ok(ClipboardContent::Text(text));
        }
        
        // Try to get image
        if let Ok(image_data) = clipboard.get_image() {
            let png_data = Self::convert_to_png(&image_data)?;
            return Ok(ClipboardContent::Image(png_data));
        }
        
        Ok(ClipboardContent::Empty)
    }

    async fn set_content(&self, content: ClipboardContent) -> Result<()> {
        let mut clipboard = self.clipboard.lock().await;
        
        // Calculate hash before moving content
        let hash = content.hash();
        
        match content {
            ClipboardContent::Text(text) => {
                clipboard.set_text(text)
                    .map_err(|e| crate::KvmError::Clipboard(format!("Failed to set text: {}", e)))?;
            }
            ClipboardContent::Image(png_data) => {
                let image_data = Self::png_to_image_data(&png_data)?;
                clipboard.set_image(image_data)
                    .map_err(|e| crate::KvmError::Clipboard(format!("Failed to set image: {}", e)))?;
            }
            ClipboardContent::Empty => {
                clipboard.clear()
                    .map_err(|e| crate::KvmError::Clipboard(format!("Failed to clear clipboard: {}", e)))?;
            }
        }
        
        // Update last hash to avoid triggering our own change
        let mut last_hash = self.last_hash.lock().await;
        *last_hash = Some(hash);
        
        Ok(())
    }

    fn subscribe(&self) -> Receiver<ClipboardEvent> {
        let (tx, rx) = channel(100);
        let subscribers = self.subscribers.clone();
        tokio::spawn(async move {
            let mut subs = subscribers.lock().await;
            subs.push(tx);
        });
        rx
    }

    async fn start_monitoring(&self) -> Result<()> {
        let mut monitoring = self.monitoring.lock().await;
        if *monitoring {
            return Ok(());
        }
        *monitoring = true;
        drop(monitoring);

        let clipboard_service = self.clipboard.clone();
        let last_hash = self.last_hash.clone();
        let subscribers = self.subscribers.clone();
        let monitoring_flag = self.monitoring.clone();
        let size_limit_mb = self.size_limit_mb;
        let sync_cache = self.sync_cache.clone();

        tokio::spawn(async move {
            let service = ClipboardServiceImpl {
                clipboard: clipboard_service,
                last_hash,
                subscribers,
                monitoring: monitoring_flag.clone(),
                size_limit_mb,
                sync_cache,
            };

            loop {
                // Check if we should stop monitoring
                {
                    let monitoring = monitoring_flag.lock().await;
                    if !*monitoring {
                        break;
                    }
                }

                // Poll clipboard every 100ms
                if let Ok(Some(content)) = service.poll_clipboard().await {
                    service.notify_subscribers(content).await;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });

        Ok(())
    }

    async fn stop_monitoring(&self) -> Result<()> {
        let mut monitoring = self.monitoring.lock().await;
        *monitoring = false;
        Ok(())
    }
}
