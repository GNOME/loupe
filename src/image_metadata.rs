use gio::prelude::*;

#[derive(Default)]
pub enum ImageMetadata {
    Exif(exif::Exif),
    #[default]
    None,
}

impl ImageMetadata {
    // TODO: error handling
    pub fn load(file: &gio::File) -> Self {
        let file = std::fs::File::open(file.path().unwrap()).unwrap();
        let mut bufreader = std::io::BufReader::new(&file);
        let exifreader = exif::Reader::new();

        if let Ok(exif) = exifreader.read_from_container(&mut bufreader) {
            Self::Exif(exif)
        } else {
            Self::None
        }
    }

    pub fn orientation(&self) -> Orientation {
        match self {
            Self::Exif(exif) => {
                if let Some(orientation) = exif
                    .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                    .and_then(|x| x.value.get_uint(0))
                {
                    Orientation::from(orientation)
                } else {
                    Orientation::default()
                }
            }
            Self::None => Orientation::default(),
        }
    }
}

// TODO: implement something (that does not fill the complete screen with binary data)
impl std::fmt::Debug for ImageMetadata {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        Ok(())
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Orientation {
    pub rotation: f64,
    pub mirrored: bool,
}

impl From<u32> for Orientation {
    fn from(number: u32) -> Self {
        match number {
            8 => Self {
                rotation: 90.,
                mirrored: false,
            },
            3 => Self {
                rotation: 180.,
                mirrored: false,
            },
            6 => Self {
                rotation: 270.,
                mirrored: false,
            },
            2 => Self {
                rotation: 0.,
                mirrored: true,
            },
            5 => Self {
                rotation: 90.,
                mirrored: true,
            },
            4 => Self {
                rotation: 180.,
                mirrored: true,
            },
            7 => Self {
                rotation: 270.,
                mirrored: true,
            },
            _ => Self::default(),
        }
    }
}
