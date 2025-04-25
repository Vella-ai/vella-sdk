use std::fmt::Display;

use crate::schemaorg::{FlightReservation, Organization};
use lol_html::{element, HtmlRewriter, Settings};
use mail_parser::{Addr, HeaderName, MessageParser, MimeHeaders};
use rayon::prelude::*;
use regex::Regex;
use scraper::{Html, Selector};

#[derive(Debug, uniffi::Error)]
pub enum ParserError {
    Base64DecodeFailed,
    NonUtfInput,
    EmptyInput,
    EmailParseFailed,
    NoFromHeader,
    NoToHeader,
}

impl Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::Base64DecodeFailed => write!(f, "Failed to decode base64 input"),
            ParserError::NonUtfInput => write!(f, "Decoded data is not a UTF8 string"),
            ParserError::EmptyInput => write!(f, "Input string is empty"),
            ParserError::EmailParseFailed => write!(f, "Failed to parse email"),
            ParserError::NoFromHeader => write!(f, "Email doesn't have a from header"),
            ParserError::NoToHeader => write!(f, "Email doesn't have a to header"),
        }
    }
}

type Return<T> = Result<T, ParserError>;

fn url_base64_decode(s: &str) -> Return<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    if s.is_empty() {
        return Err(ParserError::EmptyInput);
    }

    let normalized = s.replace('-', "+").replace('_', "/");
    match STANDARD.decode(&normalized) {
        Ok(bytes) => match std::str::from_utf8(&bytes) {
            Ok(decoded) => Ok(decoded.to_string()),
            Err(_) => Err(ParserError::NonUtfInput),
        },
        Err(_) => Err(ParserError::Base64DecodeFailed),
    }
}

#[derive(uniffi::Record)]
struct Email {
    from: EmailAddressWithText,
    from_addresses: Vec<EmailAddress>,

    to: EmailAddressWithText,
    to_addresses: Vec<EmailAddress>,

    cc_addresses: Vec<EmailAddress>,
    bcc_addresses: Vec<EmailAddress>,

    subject: Option<String>,

    /// Unix epoch in seconds
    date: Option<i64>,
    content_id: Option<String>,
    message_id: Option<String>,
    thread_name: Option<String>,
    mime_version: Option<String>,

    headers: Vec<Header>,

    text_bodies: Vec<EmailText>,
    html_bodies: Vec<EmailText>,

    markups: Vec<String>,
    organizations: Vec<Organization>,
    flight_reservations: Vec<FlightReservation>,
}

#[derive(uniffi::Record)]
struct EmailText {
    text: String,
    visible: Option<String>,
}

#[derive(uniffi::Record)]
struct EmailAddress {
    name: Option<String>,
    address: String,
}

#[derive(uniffi::Record)]
struct EmailAddressWithText {
    name: Option<String>,
    text: String,
    address: String,
}

#[derive(uniffi::Record)]
struct Header {
    name: String,
    value: String,
}

impl From<(&str, &str)> for Header {
    fn from((name, value): (&str, &str)) -> Self {
        Self {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }
}

fn parse_addr(value: &Addr<'_>) -> Option<EmailAddress> {
    Some(EmailAddress {
        name: value.name().map(ToOwned::to_owned),
        address: value.address().map(ToOwned::to_owned)?,
    })
}

fn parse_addrs(addrs: &[Addr<'_>]) -> Vec<EmailAddress> {
    addrs
        .par_iter()
        .filter_map(parse_addr)
        .collect::<Vec<EmailAddress>>()
}

fn parse_text(body: String) -> EmailText {
    let escaped = html_escape::decode_html_entities(&body);
    EmailText {
        visible: parse_visible_text(&escaped),
        text: escaped.into_owned(),
    }
}

#[uniffi::export]
fn parse_visible_text(body: &str) -> Option<String> {
    let reply_sep_re =
        Regex::new(r"On\s\w{3},\s(?:\d{1,2}|\w{3})\s(?:\d{1,2}|\w{3}),?\s\d{4}\sat\s\d{1,2}:\d{2}")
            .expect("expression is valid");

    if !reply_sep_re.is_match(body) {
        return None;
    }

    let mut parts = reply_sep_re.splitn(body, 2);
    parts.next().map(str::trim).map(ToOwned::to_owned)
}

fn parse_html(body: String) -> EmailText {
    EmailText {
        visible: parse_visible_html(&body),
        text: body,
    }
}

#[uniffi::export]
fn parse_visible_html(body: &str) -> Option<String> {
    if !body.contains("gmail_quote_container") {
        return None;
    }

    let mut output = Vec::new();

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!(".gmail_quote_container", |el| {
                el.remove();
                Ok(())
            })],
            ..Settings::new()
        },
        |c: &[u8]| output.extend_from_slice(c),
    );

    if rewriter.write(body.as_bytes()).is_err() {
        return None;
    }

    String::from_utf8(output).ok()
}

#[uniffi::export]
fn parse_email(raw: String) -> Return<Email> {
    let raw = url_base64_decode(&raw)?;
    let parser = MessageParser::default();
    let message = parser.parse(&raw).ok_or(ParserError::EmailParseFailed)?;

    let from_header = message
        .header_raw(HeaderName::From)
        .ok_or(ParserError::NoFromHeader)?
        .to_owned();

    let from_addresses: Vec<EmailAddress> = message
        .from()
        .and_then(|addr| addr.as_list())
        .map(parse_addrs)
        .ok_or(ParserError::NoFromHeader)?;

    let from = from_addresses.first().ok_or(ParserError::NoFromHeader)?;
    let from = EmailAddressWithText {
        name: from.name.to_owned(),
        text: from_header,
        address: from.address.to_owned(),
    };

    let to_header = message
        .header_raw(HeaderName::To)
        .ok_or(ParserError::NoToHeader)?
        .to_owned();

    let to_addresses: Vec<EmailAddress> = message
        .to()
        .and_then(|addr| addr.as_list())
        .map(parse_addrs)
        .ok_or(ParserError::NoToHeader)?;

    let to = to_addresses.first().ok_or(ParserError::NoToHeader)?;
    let to = EmailAddressWithText {
        name: to.name.to_owned(),
        text: to_header,
        address: to.address.to_owned(),
    };

    let cc_addresses: Vec<EmailAddress> = message
        .cc()
        .and_then(|addr| addr.as_list())
        .map(parse_addrs)
        .unwrap_or_default();

    let bcc_addresses: Vec<EmailAddress> = message
        .bcc()
        .and_then(|addr| addr.as_list())
        .map(parse_addrs)
        .unwrap_or_default();

    let subject = message.subject().map(ToOwned::to_owned);

    let date = message.date().map(|d| d.to_timestamp());

    let text_bodies: Vec<EmailText> = message
        .text_bodies()
        .par_bridge()
        .map(|x| x.to_string())
        .map(parse_text)
        .collect();
    let html_bodies: Vec<EmailText> = message
        .html_bodies()
        .par_bridge()
        .map(|x| x.to_string())
        .map(parse_html)
        .collect();

    let markups: Vec<String> = message
        .html_bodies()
        .par_bridge()
        .map(|x| x.to_string())
        .flat_map(|x| parse_json_lds(&x))
        .collect();

    let organizations: Vec<Organization> = markups
        .par_iter()
        .filter_map(|x| serde_json::from_str(x).ok())
        .collect();

    let flight_reservations: Vec<FlightReservation> = markups
        .par_iter()
        .filter_map(|x| serde_json::from_str(x).ok())
        .collect();

    let content_id = message.content_id().map(ToOwned::to_owned);
    let message_id = message.message_id().map(ToOwned::to_owned);
    let thread_name = message.thread_name().map(ToOwned::to_owned);

    let mime_version = message.mime_version().as_text().map(ToOwned::to_owned);

    let headers: Vec<Header> = message.headers_raw().map(Into::into).collect();

    Ok(Email {
        from,
        from_addresses,
        to,
        to_addresses,
        cc_addresses,
        bcc_addresses,
        subject,
        date,
        content_id,
        message_id,
        thread_name,
        mime_version,
        headers,
        text_bodies,
        html_bodies,
        markups,
        organizations,
        flight_reservations,
    })
}

fn parse_json_lds(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(r#"script[type="application/ld+json"]"#)
        .expect("failed to create json ld scripts selector");

    let mut results = Vec::new();

    for el in document.select(&selector) {
        if let Some(raw) = el.text().next() {
            let trimmed = raw.trim();

            match serde_json::from_str::<serde_json::Value>(trimmed) {
                Ok(serde_json::Value::Array(arr)) => {
                    for val in arr {
                        if let Ok(s) = serde_json::to_string(&val) {
                            results.push(s);
                        }
                    }
                }
                Ok(val) => {
                    if let Ok(s) = serde_json::to_string(&val) {
                        results.push(s);
                    }
                }
                Err(_) => {
                    // Ignore and return nothing for this script
                }
            }
        }
    }

    results
}

#[derive(serde::Deserialize)]
#[allow(non_snake_case)]
struct GmailMessageIn {
    id: String,
    threadId: String,
    labelIds: Vec<String>,
    snippet: String,
    sizeEstimate: u32,
    raw: String,
    historyId: String,
    internalDate: String,
}

#[derive(uniffi::Record)]
#[allow(non_snake_case)]
struct GmailMessage {
    id: String,
    threadId: String,
    labelIds: Vec<String>,
    snippet: EmailText,
    sizeEstimate: u32,
    data: Email,
    historyId: String,
    internalDate: String,
}

#[derive(uniffi::Record, serde::Deserialize)]
struct GmailError {
    code: u32,
    message: String,
    status: String,
    errors: Vec<GmailErrorItem>,
}

#[derive(uniffi::Record, serde::Deserialize)]
struct GmailErrorItem {
    message: String,
    domain: String,
    reason: String,
}

#[derive(uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
enum BatchResponse {
    Success(GmailMessage),
    Error(GmailError),
}

#[derive(uniffi::Record)]
struct BatchSection {
    batch_name: String,
    response: BatchResponse,
}

fn parse_gmail(
    GmailMessageIn {
        id,
        threadId,
        labelIds,
        snippet,
        sizeEstimate,
        raw,
        historyId,
        internalDate,
    }: GmailMessageIn,
) -> Option<GmailMessage> {
    Some(GmailMessage {
        id,
        threadId,
        labelIds,
        snippet: parse_text(snippet),
        sizeEstimate,
        historyId,
        internalDate,
        data: parse_email(raw).ok()?,
    })
}

#[uniffi::export]
fn parse_batch_response(body: String) -> Vec<BatchSection> {
    body.split("--")
        .par_bridge()
        .filter_map(|section| -> Option<BatchSection> {
            let batch_name = section.lines().next()?.trim().to_owned();
            let json_start = section.find('{')?;
            let json_end = section.rfind('}')?;

            let json = &section[json_start..=json_end];

            let success = serde_json::from_str(json)
                .ok()
                .and_then(parse_gmail)
                .map(BatchResponse::Success);
            let error = serde_json::from_str(json).ok().map(BatchResponse::Error);

            let response = success.or(error)?;

            Some(BatchSection {
                batch_name,
                response,
            })
        })
        .collect()
}

#[uniffi::export]
fn escape_text(text: String) -> String {
    html_escape::encode_text(&text).into_owned()
}
