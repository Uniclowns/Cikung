//! Original Cikung breeds, selected inside one application.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Breed {
    #[default]
    Orange,
    Tuxedo,
    Calico,
    Siamese,
    Silver,
    White,
}
impl Breed {
    pub const ALL: [Self; 6] = [
        Self::Orange,
        Self::Tuxedo,
        Self::Calico,
        Self::Siamese,
        Self::Silver,
        Self::White,
    ];
    pub fn index(self) -> usize {
        match self {
            Self::Orange => 0,
            Self::Tuxedo => 1,
            Self::Calico => 2,
            Self::Siamese => 3,
            Self::Silver => 4,
            Self::White => 5,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Orange => "Orange tabby",
            Self::Tuxedo => "Tuxedo",
            Self::Calico => "Calico",
            Self::Siamese => "Siamese",
            Self::Silver => "Silver shorthair",
            Self::White => "Snow",
        }
    }
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Orange => include_bytes!("../assets/embed/cikung-face.png"),
            Self::Tuxedo => include_bytes!("../assets/embed/tuxedo.png"),
            Self::Calico => include_bytes!("../assets/embed/calico.png"),
            Self::Siamese => include_bytes!("../assets/embed/siamese.png"),
            Self::Silver => include_bytes!("../assets/embed/silver.png"),
            Self::White => include_bytes!("../assets/embed/white.png"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_breed_assets_decode_with_real_transparency() {
        for breed in Breed::ALL {
            let image = image::load_from_memory(breed.bytes()).unwrap().into_rgba8();
            assert!(
                image.width() >= 256 && image.height() >= 256,
                "{}",
                breed.label()
            );
            assert_eq!(image.get_pixel(0, 0).0[3], 0, "{}", breed.label());
            assert!(image.pixels().any(|p| p.0[3] > 240), "{}", breed.label());
        }
    }
}
