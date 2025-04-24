use crate::schemaorg::*;
use serde_with::{serde_as, OneOrMany};
///<https://schema.org/PronounceableText>
#[serde_as]
#[derive(serde::Deserialize, uniffi::Record)]
pub struct PronounceableText {
    #[serde(rename = "@context")]
    pub context: String,
    ///<https://schema.org/phoneticText>
    #[serde(rename = "phoneticText")]
    #[serde_as(as = "OneOrMany<_>")]
    #[serde(default)]
    pub phonetic_text: Vec<String>,
    ///<https://schema.org/inLanguage>
    #[serde(rename = "inLanguage")]
    #[serde_as(as = "OneOrMany<_>")]
    #[serde(default)]
    pub in_language: Vec<PronounceableTextInLanguageFieldEnum>,
    ///<https://schema.org/speechToTextMarkup>
    #[serde(rename = "speechToTextMarkup")]
    #[serde_as(as = "OneOrMany<_>")]
    #[serde(default)]
    pub speech_to_text_markup: Vec<String>,
    ///<https://schema.org/textValue>
    #[serde(rename = "textValue")]
    #[serde_as(as = "OneOrMany<_>")]
    #[serde(default)]
    pub text_value: Vec<String>,
}
