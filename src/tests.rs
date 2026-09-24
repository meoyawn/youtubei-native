use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use super::host;
use super::{Error, Video, Youtube};

#[test]
fn native_playlist_and_lifetimes() {
    let mut youtube = Youtube::new().unwrap();
    assert!(matches!(Youtube::new(), Err(Error::Busy)));
    assert!(matches!(
        youtube.call("parseText", "{"),
        Err(Error::Javascript(_))
    ));
    assert_eq!(
        youtube
            .parse_text(&json!({"simpleText": "after error\u{0}🦀"}))
            .unwrap(),
        "after error\u{0}🦀"
    );
    assert_eq!(
        youtube
            .parse_text(&json!({"simpleText": "Hello from Rust 🦀"}))
            .unwrap(),
        "Hello from Rust 🦀"
    );
    assert_eq!(
        youtube
            .parse_text(&json!({"runs": [{"text": "Hello "}, {"text": "YouTube"}]}))
            .unwrap(),
        "Hello YouTube"
    );
    println!("Rust → AOT youtubei.js Text parser passed");
    let session = youtube
        .session_client_name()
        .expect("create offline Innertube session");
    assert_eq!(session, "WEB");
    println!("Rust → AOT youtubei.js async session: {session}");

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    host::fixture_origin(Some(format!("http://{address}").parse().unwrap()));
    let server = std::thread::spawn(move || {
        for page in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(10);
            let (mut socket, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "Missing fixture request");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("Fixture listener: {error}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            let header_end = loop {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break request.len();
                }
            };
            let headers = String::from_utf8_lossy(&request).to_ascii_lowercase();
            assert!(
                headers.starts_with("post /youtubei/v1/browse?prettyprint=false&alt=json http/1.1")
            );
            assert!(headers.contains("content-type: application/json"));
            assert!(headers.contains("x-youtube-client-name: 1"));
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            request.resize(header_end + length, 0);
            socket.read_exact(&mut request[header_end..]).unwrap();
            let body: Value = serde_json::from_slice(&request[header_end..]).unwrap();
            assert_eq!(body["context"]["client"]["clientName"], "WEB");
            if page == 0 {
                assert_eq!(body["browseId"], "VLfixture");
                assert_eq!(body["params"], "wgYCCAA=");
            } else {
                assert_eq!(body["continuation"], "fixture-next");
            }
            let video = json!({"playlistVideoRenderer": {
                "videoId": format!("fixture{page:04}"),
                "title": {"simpleText": if page == 0 { "First 🦀" } else { "Second" }, "accessibility": {"accessibilityData": {"label": "Fixture"}}},
                "shortBylineText": {"simpleText": "Fixture author"},
                "thumbnail": {"thumbnails": []}, "thumbnailOverlays": [],
                "navigationEndpoint": {"watchEndpoint": {"videoId": format!("fixture{page:04}")}},
                "isPlayable": true, "lengthSeconds": "60", "lengthText": {"simpleText": "1:00"},
                "videoInfo": {"simpleText": "2 days ago"}, "index": {"simpleText": (page + 1).to_string()}
            }});
            let response = if page == 0 {
                json!({"metadata": {"playlistMetadataRenderer": {"title": "Fixture"}},
                    "contents": {"playlistVideoListRenderer": {"playlistId": "fixture", "contents": [video,
                        {"continuationItemRenderer": {"trigger": "CONTINUATION_TRIGGER_ON_ITEM_SHOWN", "continuationEndpoint": {
                            "commandMetadata": {"webCommandMetadata": {"apiUrl": "/youtubei/v1/browse"}},
                            "continuationCommand": {"token": "fixture-next", "request": "CONTINUATION_REQUEST_TYPE_BROWSE"}
                        }}}
                    ]}}})
            } else {
                json!({"onResponseReceivedActions": [{"appendContinuationItemsAction": {"continuationItems": [
                    {"lockupViewModel": {"contentId": "hidden00000", "contentType": "LOCKUP_CONTENT_TYPE_VIDEO"}}
                ]}}]})
            }.to_string();
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        }
    });
    let pages = youtube
        .playlist("fixture")
        .expect("reqwest-backed Innertube playlist");
    assert_eq!(
        pages,
        vec![
            Video {
                id: "fixture0000".into(),
                title: "First 🦀".into(),
                duration: 60,
                published: "2 days ago".into(),
                available: true,
            },
            Video {
                id: "hidden00000".into(),
                title: "hidden00000".into(),
                duration: 0,
                published: String::new(),
                available: false,
            },
        ]
    );
    server.join().unwrap();
    host::fixture_origin(None);
    let operations = host::operations();
    assert_eq!(
        operations
            .iter()
            .filter(|op| op.as_str() == "fetch")
            .count(),
        2
    );
    for name in [
        "url",
        "query_parse",
        "query_string",
        "headers",
        "encode",
        "decode",
    ] {
        assert!(
            operations.iter().any(|op| op == name),
            "Rust adapter {name} was not exercised"
        );
    }
    println!("Rust → C youtubei.js → Rust reqwest: flat playlist + getContinuation passed");
    drop(youtube);
    let mut reopened = Youtube::new().unwrap();
    assert_eq!(reopened.session_client_name().unwrap(), "WEB");
}
