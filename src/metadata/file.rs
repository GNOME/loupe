use crate::deps::*;

pub struct FileInfo {
    pub(super) display_name: glib::GString,
    pub(super) file_size: Option<u64>,
    pub(super) mime_type: Option<glib::GString>,
    pub(super) created: Option<glib::DateTime>,
    pub(super) modified: Option<glib::DateTime>,
}

impl FileInfo {
    pub async fn new(file: &gio::File) -> anyhow::Result<Self> {
        let file_info = crate::util::query_attributes_future(
            file,
            vec![
                gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                gio::FILE_ATTRIBUTE_STANDARD_SIZE,
                gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                gio::FILE_ATTRIBUTE_TIME_CREATED,
                gio::FILE_ATTRIBUTE_TIME_MODIFIED,
            ],
        )
        .await?;

        let size = file_info.size();
        let file_size = if size > 0 { Some(size as u64) } else { None };

        Ok(Self {
            display_name: file_info.display_name(),
            file_size,
            mime_type: file_info
                .content_type()
                .and_then(|x| gio::content_type_get_mime_type(&x)),
            created: file_info.creation_date_time(),
            modified: file_info.modification_date_time(),
        })
    }
}
