use super::*;

// Keep the published JSON shape while giving binary serializers an enum
// discriminant they can decode without deserialize_any.
#[derive(Serialize, Deserialize)]
#[serde(remote = "EntityKind", tag = "type", rename_all = "snake_case")]
enum HumanKind {
    Image(ImageData),
    Text(TextData),
    Character(CharacterData),
    Video(VideoData),
    Audio(AudioData),
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "EntityKind", rename_all = "snake_case")]
enum BinaryKind {
    Image(ImageData),
    Text(TextData),
    Character(CharacterData),
    Video(VideoData),
    Audio(AudioData),
}

impl Serialize for EntityKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            HumanKind::serialize(self, serializer)
        } else {
            BinaryKind::serialize(self, serializer)
        }
    }
}

impl<'de> Deserialize<'de> for EntityKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            HumanKind::deserialize(deserializer)
        } else {
            BinaryKind::deserialize(deserializer)
        }
    }
}
