use std::{fmt::Display, str::FromStr};

use tokenizers::Tokenizer;

#[derive(uniffi::Record)]
struct Token {
    id: u32,
    token: String,
    start: u32,
    end: u32,
}

#[derive(uniffi::Record)]
struct TokenizedBatch {
    token_ids: Vec<Vec<u32>>,
    attention_mask: Vec<Vec<u32>>,
    type_ids: Vec<Vec<u32>>,
}

#[derive(uniffi::Enum, Debug)]
pub enum SpecialTokens {
    Yes,
    No,
}

impl From<SpecialTokens> for bool {
    fn from(value: SpecialTokens) -> Self {
        match value {
            SpecialTokens::Yes => true,
            SpecialTokens::No => false,
        }
    }
}

#[derive(uniffi::Record, Debug)]
pub struct PaddingParams {
    pub strategy: PaddingStrategy,
    pub direction: PaddingDirection,
    pub pad_to_multiple_of: Option<u32>,
    pub pad_id: u32,
    pub pad_type_id: u32,
    pub pad_token: String,
}

impl From<PaddingParams> for tokenizers::PaddingParams {
    fn from(value: PaddingParams) -> Self {
        Self {
            strategy: value.strategy.into(),
            direction: value.direction.into(),
            pad_to_multiple_of: value.pad_to_multiple_of.map(|x| x as usize),
            pad_id: value.pad_id,
            pad_type_id: value.pad_type_id,
            pad_token: value.pad_token,
        }
    }
}

#[derive(uniffi::Enum, Debug)]
pub enum PaddingStrategy {
    BatchLongest,
    Fixed(u32),
}

impl From<PaddingStrategy> for tokenizers::PaddingStrategy {
    fn from(value: PaddingStrategy) -> Self {
        match value {
            PaddingStrategy::BatchLongest => Self::BatchLongest,
            PaddingStrategy::Fixed(x) => Self::Fixed(x as usize),
        }
    }
}

#[derive(uniffi::Enum, Debug)]
pub enum PaddingDirection {
    Left,
    Right,
}

impl From<PaddingDirection> for tokenizers::PaddingDirection {
    fn from(value: PaddingDirection) -> Self {
        match value {
            PaddingDirection::Left => Self::Left,
            PaddingDirection::Right => Self::Right,
        }
    }
}

#[derive(uniffi::Record, Debug)]
pub struct TruncationParams {
    pub direction: TruncationDirection,
    pub max_length: u32,
    pub strategy: TruncationStrategy,
    pub stride: u32,
}

impl From<TruncationParams> for tokenizers::TruncationParams {
    fn from(value: TruncationParams) -> Self {
        Self {
            direction: value.direction.into(),
            max_length: value.max_length as usize,
            strategy: value.strategy.into(),
            stride: value.stride as usize,
        }
    }
}

#[derive(uniffi::Enum, Debug)]
pub enum TruncationDirection {
    Left,
    Right,
}

impl From<TruncationDirection> for tokenizers::TruncationDirection {
    fn from(value: TruncationDirection) -> Self {
        match value {
            TruncationDirection::Left => Self::Left,
            TruncationDirection::Right => Self::Right,
        }
    }
}

#[derive(uniffi::Enum, Debug)]
pub enum TruncationStrategy {
    LongestFirst,
    OnlyFirst,
    OnlySecond,
}

impl From<TruncationStrategy> for tokenizers::TruncationStrategy {
    fn from(value: TruncationStrategy) -> Self {
        match value {
            TruncationStrategy::LongestFirst => Self::LongestFirst,
            TruncationStrategy::OnlyFirst => Self::OnlyFirst,
            TruncationStrategy::OnlySecond => Self::OnlySecond,
        }
    }
}

#[derive(uniffi::Error, Debug)]
pub enum TokenizeError {
    TokenizerCreationFailed,
    InputEncodingFailed,
    InvalidTruncationParams,
}

impl Display for TokenizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizeError::TokenizerCreationFailed => write!(f, "Tokenizer creation failed"),
            TokenizeError::InputEncodingFailed => write!(f, "Input encoding failed"),
            TokenizeError::InvalidTruncationParams => write!(f, "Invalid truncation params"),
        }
    }
}

/// A tokenizer object from a custom dictionary.
#[derive(uniffi::Object)]
struct CustomTokenizerInner {
    tokenizer: Tokenizer,
}

#[uniffi::export]
impl CustomTokenizerInner {
    /// Creates a new custom tokenizer.
    #[uniffi::constructor]
    fn new(
        dictionary: &str,
        padding: Option<PaddingParams>,
        truncation: Option<TruncationParams>,
    ) -> Result<Self, TokenizeError> {
        let mut tokenizer =
            Tokenizer::from_str(dictionary).map_err(|_| TokenizeError::TokenizerCreationFailed)?;

        if let Some(padding) = padding {
            tokenizer.with_padding(Some(padding.into()));
        }
        if let Some(truncation) = truncation {
            tokenizer
                .with_truncation(Some(truncation.into()))
                .map_err(|_| TokenizeError::InvalidTruncationParams)?;
        }

        Ok(Self { tokenizer })
    }
}

#[uniffi::export]
impl CustomTokenizerInner {
    /// Tokenizes an input string and returns a list of tokens.
    fn tokenize(
        &self,
        input: &str,
        special_tokens: SpecialTokens,
    ) -> Result<Vec<Token>, TokenizeError> {
        let encoding = self
            .tokenizer
            .encode(input, special_tokens.into())
            .map_err(|_| TokenizeError::InputEncodingFailed)?;

        let tokens: Vec<Token> = encoding
            .get_tokens()
            .iter()
            .cloned()
            .zip(encoding.get_ids().iter().cloned())
            .zip(encoding.get_offsets().iter().cloned())
            .map(|((token, id), (start, end))| Token {
                id,
                token,
                start: start as u32,
                end: end as u32,
            })
            .collect();

        Ok(tokens)
    }

    /// Tokenizes a list of input strings and returns a list of token IDs.
    fn tokenize_batch(
        &self,
        input: Vec<String>,
        special_tokens: SpecialTokens,
    ) -> Result<TokenizedBatch, TokenizeError> {
        let encodings = self
            .tokenizer
            .encode_batch(input, special_tokens.into())
            .map_err(|_| TokenizeError::InputEncodingFailed)?;

        let token_ids: Vec<_> = encodings.iter().map(|e| e.get_ids().to_vec()).collect();

        let attention_mask: Vec<_> = encodings
            .iter()
            .map(|e| e.get_attention_mask().to_vec())
            .collect();

        let type_ids: Vec<_> = encodings
            .iter()
            .map(|e| e.get_type_ids().to_vec())
            .collect();

        Ok(TokenizedBatch {
            token_ids,
            attention_mask,
            type_ids,
        })
    }

    /// Tokenizes an input string and return a list of token IDs.
    fn get_ids(
        &self,
        input: &str,
        special_tokens: SpecialTokens,
    ) -> Result<Vec<u32>, TokenizeError> {
        Ok(self
            .tokenizer
            .encode(input, special_tokens.into())
            .map_err(|_| TokenizeError::InputEncodingFailed)?
            .get_ids()
            .to_vec())
    }

    /// Tokenizes an input string and returns a list of token strings.
    fn get_tokens(
        &self,
        input: &str,
        special_tokens: SpecialTokens,
    ) -> Result<Vec<String>, TokenizeError> {
        let encoding = self
            .tokenizer
            .encode(input, special_tokens.into())
            .map_err(|_| TokenizeError::InputEncodingFailed)?;
        Ok(encoding.get_tokens().to_vec())
    }

    /// Gets the ID value of a given token.
    fn token_to_id(&self, token: &str) -> Option<u32> {
        self.tokenizer.token_to_id(token)
    }

    /// Gets the string value of a given token ID.
    fn id_to_token(&self, id: u32) -> Option<String> {
        self.tokenizer.id_to_token(id)
    }
}
