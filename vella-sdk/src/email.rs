use std::{collections::HashMap, fmt::Display};

use chrono::{TimeZone, Utc};
use icalendar::{Calendar, Component, DatePerhapsTime, EventLike};
use lol_html::{element, HtmlRewriter, Settings};
use mail_parser::{Addr, HeaderName, MessageParser, MimeHeaders};
use rayon::prelude::*;
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

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
    calendar_events: Vec<CalendarEvent>,
    microdata_items: Vec<MicrodataItem>,

    unsubscribe: Unsubscribe,
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

    let calendar_events: Vec<CalendarEvent> = message
        .attachments()
        .par_bridge()
        .filter(|m| {
            m.content_type().is_some_and(|typ| {
                typ.ctype() == "text" && typ.subtype().is_some_and(|s| s == "calendar")
            })
        })
        .filter_map(|m| m.text_contents())
        .filter_map(parse_events)
        .flatten()
        .collect();

    let markups: Vec<String> = message
        .html_bodies()
        .par_bridge()
        .map(|x| x.to_string())
        .flat_map(|x| parse_json_lds(&x))
        .collect();

    let microdata_items: Vec<MicrodataItem> = message
        .html_bodies()
        .par_bridge()
        .map(|x| x.to_string())
        .flat_map(|x| extract_microdata(&x))
        .collect();

    let unsubscribe = extract_unsubscribe(&message);

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
        calendar_events,
        microdata_items,
        unsubscribe,
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

#[derive(uniffi::Record)]
struct CalendarEvent {
    uid: Option<String>,
    summary: Option<String>,
    status: Option<CalendarEventStatus>,
    url: Option<String>,
    google_conference_link: Option<String>,
    location: Option<String>,
    timestamp: Option<i64>,
    last_modified: Option<i64>,
    created: Option<i64>,
    start: Option<i64>,
    end: Option<i64>,
}

#[derive(uniffi::Enum)]
enum CalendarEventStatus {
    /// Indicates event is tentative.
    Tentative,
    /// Indicates event is definite.
    Confirmed,
    /// Indicates event was cancelled.
    Cancelled,
}

impl From<icalendar::EventStatus> for CalendarEventStatus {
    fn from(value: icalendar::EventStatus) -> Self {
        match value {
            icalendar::EventStatus::Tentative => Self::Tentative,
            icalendar::EventStatus::Confirmed => Self::Confirmed,
            icalendar::EventStatus::Cancelled => Self::Cancelled,
        }
    }
}

fn parse_events(body: &str) -> Option<Vec<CalendarEvent>> {
    let calendar: Calendar = body.parse().ok()?;

    Some(
        calendar
            .components
            .into_iter()
            .filter_map(parse_calendar_event)
            .collect(),
    )
}

fn parse_calendar_event(comp: icalendar::CalendarComponent) -> Option<CalendarEvent> {
    let event = comp.as_event()?;

    // Uses LAST_MODIFIED instead
    let last_modified1 = event.get_last_modified().map(|x| x.timestamp_millis());
    let last_modified2 = event
        .properties()
        .get("LAST-MODIFIED")
        .and_then(DatePerhapsTime::from_property)
        .and_then(get_timestamp);

    let last_modified = last_modified1.or(last_modified2);

    Some(CalendarEvent {
        uid: event.get_uid().map(|s| s.to_owned()),
        summary: event.get_summary().map(|s| s.to_owned()),
        status: event.get_status().map(|s| s.into()),
        url: event.get_url().map(|x| x.to_owned()),
        google_conference_link: event
            .property_value("X-GOOGLE-CONFERENCE")
            .map(|x| x.to_owned()),
        location: event.get_location().map(|x| x.to_string()),
        timestamp: event.get_timestamp().map(|x| x.timestamp_millis()),
        last_modified,
        created: event.get_created().map(|x| x.timestamp_millis()),
        start: event.get_start().and_then(get_timestamp),
        end: event.get_end().and_then(get_timestamp),
    })
}

fn get_timestamp(x: icalendar::DatePerhapsTime) -> Option<i64> {
    match x {
        DatePerhapsTime::DateTime(calendar_date_time) => match calendar_date_time {
            icalendar::CalendarDateTime::Floating(naive_date_time) => {
                let timestamp = naive_date_time.and_utc().timestamp_millis();
                Some(timestamp)
            }
            icalendar::CalendarDateTime::Utc(date_time) => {
                let timestamp = date_time.timestamp_millis();
                Some(timestamp)
            }
            icalendar::CalendarDateTime::WithTimezone { date_time, tzid } => {
                let tz: chrono_tz::Tz = tzid.parse().ok()?;
                let local_dt = tz.from_local_datetime(&date_time).single()?;
                let timestamp = local_dt.timestamp_millis();
                Some(timestamp)
            }
        },
        DatePerhapsTime::Date(naive_date) => {
            let datetime = naive_date.and_hms_opt(0, 0, 0)?;
            let timestamp = Utc.from_utc_datetime(&datetime).timestamp_millis();
            Some(timestamp)
        }
    }
}

#[derive(uniffi::Record)]
struct MicrodataItem {
    itemtype: Option<String>,
    properties: HashMap<String, String>,
    children: HashMap<String, MicrodataItem>,
}

fn extract_microdata_items(element: &scraper::ElementRef) -> MicrodataItem {
    let itemtype = element.value().attr("itemtype").map(String::from);
    let mut properties = HashMap::new();
    let mut children = HashMap::new();

    let prop_selector = Selector::parse("[itemprop]").unwrap();

    for prop in element.select(&prop_selector) {
        if prop.value().attr("itemscope").is_some() {
            if let Some(prop_name) = prop.value().attr("itemprop") {
                let child = extract_microdata_items(&prop);
                children.insert(prop_name.to_owned(), child);
            }
        } else if let Some(prop_name) = prop.value().attr("itemprop") {
            let value = prop.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let value = prop
                .attr("content")
                .or(prop.attr("href"))
                .map(|c| c.to_owned())
                .unwrap_or(value);

            properties.insert(prop_name.to_owned(), value);
        }
    }

    MicrodataItem {
        itemtype,
        properties,
        children,
    }
}

fn extract_microdata(html: &str) -> Vec<MicrodataItem> {
    let document = Html::parse_document(html);
    let root_selector = Selector::parse("[itemscope]").unwrap();

    document
        .select(&root_selector)
        // Skip nested scopes; only process top-level items here
        .filter(|el| {
            el.ancestors()
                .filter_map(scraper::ElementRef::wrap)
                .all(|e| e.value().attr("itemscope").is_none())
        })
        .map(|el| extract_microdata_items(&el))
        .collect()
}

#[derive(uniffi::Record)]
struct Unsubscribe {
    get: Option<String>,
    post: Option<UnsubscribePost>,
    email: Option<UnsubscribeEmail>,
}

#[derive(uniffi::Record)]
struct UnsubscribePost {
    url: String,
    body: String,
}

#[derive(uniffi::Record)]
struct UnsubscribeEmail {
    email: String,
    headers: Vec<Header>,
}

fn extract_unsubscribe(message: &mail_parser::Message<'_>) -> Unsubscribe {
    let list_unsubscribe = message
        .header_raw("list-unsubscribe")
        .unwrap_or_default()
        .trim();

    if list_unsubscribe.is_empty() {
        return Unsubscribe {
            get: None,
            post: None,
            email: None,
        };
    }

    let mut urls = list_unsubscribe
        .split(",")
        .map(|x| x.trim())
        .map(|x| x.trim_start_matches('<'))
        .map(|x| x.trim_end_matches('>'))
        .filter_map(|x| Url::parse(x).ok());

    let url = urls.find(|u| u.scheme() == "http" || u.scheme() == "https");
    let list_unsubscribe_post = message
        .header_raw("list-unsubscribe-post")
        .map(|x| x.trim());

    let get = match (&url, list_unsubscribe_post) {
        (Some(url), None) => Some(url.to_string()),
        _ => None,
    };

    let post = match (&url, list_unsubscribe_post) {
        (Some(url), Some(post)) => Some(UnsubscribePost {
            url: url.to_string(),
            body: post.to_owned(),
        }),
        _ => None,
    };

    let email = urls.find(|u| u.scheme() == "mailto").map(|url| {
        let headers = url
            .query_pairs()
            .into_iter()
            .map(|(name, value)| Header {
                name: name.into_owned(),
                value: value.into_owned(),
            })
            .collect();

        UnsubscribeEmail {
            email: url.path().to_owned(),
            headers,
        }
    });

    Unsubscribe { get, post, email }
}

#[cfg(test)]
mod test {
    use super::parse_batch_response;

    #[test]
    fn do_test() {
        let responses = std::fs::read_dir("responses/allspark")
            .unwrap()
            .into_iter()
            .filter_map(|x| x.ok())
            .map(|x| x.path());

        for path in responses {
            let file = std::fs::read_to_string(path).unwrap();
            parse_batch_response(file);
        }
    }
}
