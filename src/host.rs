use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::io::Read;
use std::time::Duration;
use url::Url;

#[derive(Default)]
struct Transport {
    input: Vec<u16>,
    output: Vec<u8>,
    fixture_origin: Option<Url>,
    client: Option<(String, reqwest::blocking::Client)>,
    #[cfg(test)]
    operations: Vec<String>,
}

thread_local! {
    static TRANSPORT: RefCell<Transport> = RefCell::new(Transport::default());
}

pub(super) fn reset() {
    TRANSPORT.with(|state| *state.borrow_mut() = Transport::default());
}

#[cfg(test)]
pub(super) fn fixture_origin(origin: Option<Url>) {
    TRANSPORT.with(|state| state.borrow_mut().fixture_origin = origin);
}

#[cfg(test)]
pub(super) fn operations() -> Vec<String> {
    TRANSPORT.with(|state| state.borrow().operations.clone())
}

#[unsafe(no_mangle)]
pub extern "C" fn youtubei_host_begin(length: i32) {
    TRANSPORT.with(|state| state.borrow_mut().input = vec![0; length.max(0) as usize]);
}

#[unsafe(no_mangle)]
pub extern "C" fn youtubei_host_write(index: i32, unit: i32) {
    TRANSPORT.with(|state| {
        if let Some(slot) = state.borrow_mut().input.get_mut(index as usize) {
            *slot = unit as u16;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn youtubei_host_call() -> i32 {
    let result = std::panic::catch_unwind(|| {
        let (input, origin) = TRANSPORT.with(|state| {
            let state = state.borrow();
            (
                String::from_utf16_lossy(&state.input),
                state.fixture_origin.clone(),
            )
        });
        let request: Value = serde_json::from_str(&input).map_err(|error| error.to_string())?;
        let op = string(&request, "op")?;
        #[cfg(test)]
        TRANSPORT.with(|state| state.borrow_mut().operations.push(op.to_owned()));
        dispatch(op, &request["args"], origin.as_ref())
    });
    let response = match result {
        Ok(Ok(value)) => json!({"value": value}),
        Ok(Err(error)) => json!({"error": error}),
        Err(_) => json!({"error": "Rust host operation panicked"}),
    };
    TRANSPORT.with(|state| {
        let mut state = state.borrow_mut();
        state.output = response.to_string().into_bytes();
        state.output.len() as i32
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn youtubei_host_data() -> *const u8 {
    // The generated host unit copies these bytes with _sh_asciiz_to_string
    // immediately, before another host call can replace the buffer. Explicit
    // length preserves embedded NULs; no Rust allocation is transferred.
    TRANSPORT.with(|state| state.borrow().output.as_ptr())
}

fn string<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args[key]
        .as_str()
        .ok_or_else(|| format!("Missing string: {key}"))
}

fn pairs(args: &Value) -> Result<Vec<(String, String)>, String> {
    serde_json::from_value(args["pairs"].clone()).map_err(|error| error.to_string())
}

fn header_pairs(headers: &HeaderMap) -> Value {
    json!(
        headers
            .iter()
            .map(|(name, value)| (
                name.as_str(),
                String::from_utf8_lossy(value.as_bytes()).into_owned()
            ))
            .collect::<Vec<_>>()
    )
}

fn headers(pairs: Vec<(String, String)>) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        headers.append(
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
            HeaderValue::from_str(value.trim()).map_err(|error| error.to_string())?,
        );
    }
    Ok(headers)
}

fn dispatch(op: &str, args: &Value, origin: Option<&Url>) -> Result<Value, String> {
    match op {
        "encode" => {
            let units: Vec<u16> =
                serde_json::from_value(args["units"].clone()).map_err(|error| error.to_string())?;
            Ok(json!(String::from_utf16_lossy(&units).into_bytes()))
        }
        "decode" => {
            let bytes: Vec<u8> =
                serde_json::from_value(args["bytes"].clone()).map_err(|error| error.to_string())?;
            let text = if args["fatal"].as_bool().unwrap_or(false) {
                String::from_utf8(bytes).map_err(|error| error.to_string())?
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            };
            Ok(json!(if args["ignoreBOM"].as_bool().unwrap_or(false) {
                &text
            } else {
                text.strip_prefix('\u{feff}').unwrap_or(&text)
            }))
        }
        "url" => {
            let input = string(args, "input")?;
            let mut url = if let Some(base) = args["base"].as_str() {
                Url::parse(base).and_then(|url| url.join(input))
            } else {
                Url::parse(input)
            }
            .map_err(|error| error.to_string())?;
            if let Some(query) = args["query"].as_str() {
                url.set_query(if query.is_empty() { None } else { Some(query) });
            }
            let host = match url.port() {
                Some(port) => format!("{}:{port}", url.host_str().unwrap_or("")),
                None => url.host_str().unwrap_or("").to_owned(),
            };
            Ok(
                json!({"href": url.as_str(), "origin": url.origin().ascii_serialization(),
                "hostname": url.host_str().unwrap_or(""), "host": host, "pathname": url.path(),
                "protocol": format!("{}:", url.scheme()),
                "search": url.query().map(|query| format!("?{query}")).unwrap_or_default(),
                "hash": url.fragment().map(|hash| format!("#{hash}")).unwrap_or_default()}),
            )
        }
        "query_parse" => Ok(json!(
            url::form_urlencoded::parse(string(args, "query")?.as_bytes())
                .into_owned()
                .collect::<Vec<_>>()
        )),
        "query_string" => Ok(json!(
            url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(pairs(args)?)
                .finish()
        )),
        "headers" => {
            let mut values = headers(pairs(args)?)?;
            let action = string(args, "action")?;
            if action == "normalize" {
                return Ok(header_pairs(&values));
            }
            let name = HeaderName::from_bytes(string(args, "name")?.as_bytes())
                .map_err(|error| error.to_string())?;
            match action {
                "has" => return Ok(json!(values.contains_key(name))),
                "get" => {
                    if !values.contains_key(&name) {
                        return Ok(Value::Null);
                    }
                    return Ok(json!(
                        values
                            .get_all(name)
                            .iter()
                            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                "delete" => {
                    values.remove(name);
                }
                "set" | "append" => {
                    let value = HeaderValue::from_str(string(args, "value")?.trim())
                        .map_err(|error| error.to_string())?;
                    if action == "set" {
                        values.insert(name, value);
                    } else {
                        values.append(name, value);
                    }
                }
                _ => return Err(format!("Unsupported header operation: {action}")),
            }
            Ok(header_pairs(&values))
        }
        "fetch" => {
            let original = Url::parse(string(args, "url")?).map_err(|error| error.to_string())?;
            let mut target = original.clone();
            if let Some(origin) = origin {
                if original.host_str() != Some("www.youtube.com") {
                    return Err("Fixture transport only accepts www.youtube.com".into());
                }
                target = origin
                    .join(&format!(
                        "{}?{}",
                        original.path(),
                        original.query().unwrap_or("")
                    ))
                    .map_err(|error| error.to_string())?;
            }
            let redirect_name = args["redirect"].as_str().unwrap_or("follow");
            let redirect = match redirect_name {
                "follow" => reqwest::redirect::Policy::limited(10),
                "manual" | "error" => reqwest::redirect::Policy::none(),
                _ => return Err("Invalid redirect policy".into()),
            };
            let client = TRANSPORT.with(|state| {
                let mut state = state.borrow_mut();
                if let Some((policy, client)) = &state.client
                    && policy == redirect_name
                {
                    return Ok(client.clone());
                }
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .redirect(redirect)
                    .build()
                    .map_err(|error| error.to_string())?;
                state.client = Some((redirect_name.to_owned(), client.clone()));
                Ok::<_, String>(client)
            })?;
            let method = string(args, "method")?
                .parse::<reqwest::Method>()
                .map_err(|error| error.to_string())?;
            let request_headers: Vec<(String, String)> =
                serde_json::from_value(args["headers"].clone())
                    .map_err(|error| error.to_string())?;
            let mut request = client
                .request(method, target)
                .headers(headers(request_headers)?);
            if let Some(body) = args["body"].as_str() {
                request = request.body(body.to_owned());
            }
            let response = request.send().map_err(|error| error.to_string())?;
            if args["redirect"] == "error" && response.status().is_redirection() {
                return Err("Unexpected HTTP redirect".into());
            }
            let status = response.status().as_u16();
            let final_url = if origin.is_some() {
                original.as_str()
            } else {
                response.url().as_str()
            }
            .to_owned();
            let headers = header_pairs(response.headers());
            let mut body = Vec::new();
            response
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut body)
                .map_err(|error| error.to_string())?;
            if body.len() > 8 * 1024 * 1024 {
                return Err("Probe response exceeds 8 MiB".into());
            }
            // Listing responses are JSON text. Passing byte arrays through JSON
            // multiplies the allocation and copying cost for every flat page.
            let body = String::from_utf8_lossy(&body);
            let body = body.strip_prefix('\u{feff}').unwrap_or(&body);
            Ok(json!({"status": status, "url": final_url, "headers": headers, "body": body}))
        }
        _ => Err(format!("Unsupported host operation: {op}")),
    }
}
