pub struct MediaInfo {
    pub file_id: String,
    pub file_unique_id: String,
    pub file_size: Option<i32>,
    pub mime_type: Option<String>,
}

impl MediaInfo {
    pub fn new(file_id: String, file_unique_id: String) -> Self {
        MediaInfo {
            file_id,
            file_unique_id,
            file_size: None,
            mime_type: None,
        }
    }

    pub fn with_size(mut self, size: i32) -> Self {
        self.file_size = Some(size);
        self
    }

    pub fn with_mime_type(mut self, mime_type: String) -> Self {
        self.mime_type = Some(mime_type);
        self
    }
}
