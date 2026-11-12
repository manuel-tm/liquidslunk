//! `slunk://localhost/files/:id` (backend.md, "Serving files"): a registered
//! file's bytes, read-only, for `<img>` and `<audio>`, as slunk's
//! `GET /raw/files/:id` served them. An unknown id, or a file missing on disk,
//! gets a 404. Range requests get a 206, so audio can seek.
//!
//! `slunk://localhost/audio/:id` is what the app's audio plays: a FLAC file
//! as an MP4 that holds its frames, which WebKit seeks exactly, and any other
//! file as `/files/:id` serves it (backend.md, "Serving FLAC as MP4").

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use slunk::media::{AudioSource, FlacMp4};

use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::library::Libraries;

/// The scheme, registered by `lib.rs`.
pub const SCHEME: &str = "slunk";

/// How much a range with no end gets. A protocol response is a whole body in
/// memory, so `bytes=0-` gets this much, and the media element asks for the
/// rest as it plays.
const OPEN_RANGE: u64 = 1024 * 1024;

/// Answers on the blocking pool, so reading a file never blocks the main
/// thread.
pub fn handle<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let libraries = app.state::<Libraries>();
        responder.respond(respond(&libraries, &request));
    });
}

/// What a request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// A file's bytes
    File(i64),
    /// A file as the app's audio plays it
    Audio(i64),
}

fn route(path: &str) -> Option<Route> {
    let id = |id: &str| id.parse::<i64>().ok();
    if let Some(rest) = path.strip_prefix("/files/") {
        id(rest).map(Route::File)
    } else if let Some(rest) = path.strip_prefix("/audio/") {
        id(rest).map(Route::Audio)
    } else {
        None
    }
}

fn respond(libraries: &Libraries, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(route) = route(request.uri().path()) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Ok(library) = libraries.library() else {
        return status(StatusCode::NOT_FOUND);
    };
    let range = request.headers().get(header::RANGE);
    // Only the lookup holds the connection; the file is read after
    let (id, found) = match route {
        Route::File(id) => (
            id,
            library
                .media
                .file_path(id)
                .map(|path| path.map(AudioSource::File)),
        ),
        Route::Audio(id) => (
            id,
            library.media.audio_source(id, |path, error| {
                eprintln!(
                    "Can't serve {} as MP4, so it's served as it is: {error}",
                    path.display()
                );
            }),
        ),
    };
    match found {
        Ok(Some(AudioSource::File(path))) => serve(&path, range),
        Ok(Some(AudioSource::Mp4(mp4))) => serve_mp4(&mp4, range),
        Ok(None) => status(StatusCode::NOT_FOUND),
        Err(error) => {
            eprintln!("Can't look up file {id}: {error}");
            status(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn serve(path: &Path, range: Option<&header::HeaderValue>) -> Response<Vec<u8>> {
    let Ok(mut file) = File::open(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return status(StatusCode::NOT_FOUND);
    };
    ranged(length, &content_type(path), range, |start, end| {
        let mut body = vec![0; (end - start + 1) as usize];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut body)?;
        Ok(body)
    })
}

/// A FLAC file as an MP4: its header, then its frames, read from the file.
fn serve_mp4(mp4: &FlacMp4, range: Option<&header::HeaderValue>) -> Response<Vec<u8>> {
    ranged(mp4.len(), "audio/mp4", range, |start, end| {
        mp4.read(start, end)
    })
}

/// A body of `length` bytes, or the range of it asked for. `read` reads
/// from `start` to `end`, both included.
fn ranged(
    length: u64,
    content_type: &str,
    range: Option<&header::HeaderValue>,
    read: impl FnOnce(u64, u64) -> io::Result<Vec<u8>>,
) -> Response<Vec<u8>> {
    let range = range.and_then(|value| value.to_str().ok());
    let Some(range) = range else {
        let body = if length == 0 {
            Ok(Vec::new())
        } else {
            read(0, length - 1)
        };
        let Ok(body) = body else {
            return status(StatusCode::INTERNAL_SERVER_ERROR);
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, body.len())
            .header(header::ACCEPT_RANGES, "bytes")
            .body(body)
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR));
    };

    let Some((start, end)) = parse_range(range, length) else {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{length}"))
            .body(Vec::new())
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR));
    };
    let Ok(body) = read(start, end) else {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    };
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, body.len())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{length}"),
        )
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

/// The first range of a `Range: bytes=…` header, as inclusive offsets, or
/// `None` when it can't be satisfied.
fn parse_range(header: &str, length: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    let first = spec.split(',').next()?.trim();
    let (start, end) = first.split_once('-')?;
    if length == 0 {
        return None;
    }
    let last = length - 1;
    let (start, end) = match (start.trim(), end.trim()) {
        // The last n bytes
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            if n == 0 {
                return None;
            }
            (length.saturating_sub(n), last)
        }
        (start, "") => {
            let start: u64 = start.parse().ok()?;
            (start, last.min(start.saturating_add(OPEN_RANGE - 1)))
        }
        (start, end) => (start.parse().ok()?, last.min(end.parse().ok()?)),
    };
    (start <= end && start <= last).then_some((start, end))
}

/// The content type, from the extension.
fn content_type(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        // mime_guess says audio/m4a, which isn't a registered type
        Some("m4a") => "audio/mp4".into(),
        _ => mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string(),
    }
}

fn status(status: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = status;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_clamped_and_open_ones_get_a_megabyte() {
        let mb = OPEN_RANGE;
        assert_eq!(parse_range("bytes=0-1", 100), Some((0, 1)));
        assert_eq!(parse_range("bytes=10-", 100), Some((10, 99)));
        assert_eq!(parse_range("bytes=0-", 10 * mb), Some((0, mb - 1)));
        assert_eq!(parse_range("bytes=50-500", 100), Some((50, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-500", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=0-1, 5-6", 100), Some((0, 1)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=5-1", 100), None);
        assert_eq!(parse_range("bytes=x-1", 100), None);
        assert_eq!(parse_range("items=0-1", 100), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
    }

    #[test]
    fn routes_are_a_file_or_its_audio_by_id() {
        assert_eq!(route("/files/12"), Some(Route::File(12)));
        assert_eq!(route("/audio/12"), Some(Route::Audio(12)));
        assert_eq!(route("/audio/"), None);
        assert_eq!(route("/audio/x"), None);
        assert_eq!(route("/covers/12"), None);
        assert_eq!(route("/files/12/x"), None);
    }

    #[test]
    fn ranges_of_a_body_are_read_as_asked() {
        let bytes: Vec<u8> = (0..=255).collect();
        let read = |start: u64, end: u64| Ok(bytes[start as usize..=end as usize].to_vec());
        let whole = ranged(256, "audio/mp4", None, read);
        assert_eq!(whole.status(), StatusCode::OK);
        assert_eq!(whole.body(), &bytes);
        let value = header::HeaderValue::from_static("bytes=10-19");
        let part = ranged(256, "audio/mp4", Some(&value), read);
        assert_eq!(part.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(part.body(), &bytes[10..20]);
        assert_eq!(part.headers()[header::CONTENT_RANGE], "bytes 10-19/256");
        assert_eq!(part.headers()[header::CONTENT_TYPE], "audio/mp4");
        let outside = header::HeaderValue::from_static("bytes=300-");
        let refused = ranged(256, "audio/mp4", Some(&outside), read);
        assert_eq!(refused.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    }

    #[test]
    fn content_types_come_from_the_extension() {
        for (file, expected) in [
            ("a.flac", "audio/flac"),
            ("a.mp3", "audio/mpeg"),
            ("a.M4A", "audio/mp4"),
            ("a.jpg", "image/jpeg"),
            ("a.png", "image/png"),
            ("a.log", "text/plain"),
            ("a.unknown", "application/octet-stream"),
        ] {
            assert_eq!(content_type(Path::new(file)), expected, "{file}");
        }
    }

    #[test]
    fn serves_whole_files_and_ranges() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.flac");
        std::fs::write(&path, b"0123456789").unwrap();

        let whole = serve(&path, None);
        assert_eq!(whole.status(), StatusCode::OK);
        assert_eq!(whole.body(), b"0123456789");
        assert_eq!(whole.headers()[header::CONTENT_TYPE], "audio/flac");

        let value = header::HeaderValue::from_static("bytes=2-4");
        let part = serve(&path, Some(&value));
        assert_eq!(part.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(part.body(), b"234");
        assert_eq!(part.headers()[header::CONTENT_RANGE], "bytes 2-4/10");

        let value = header::HeaderValue::from_static("bytes=20-");
        assert_eq!(
            serve(&path, Some(&value)).status(),
            StatusCode::RANGE_NOT_SATISFIABLE
        );
        assert_eq!(
            serve(&dir.path().join("gone.flac"), None).status(),
            StatusCode::NOT_FOUND
        );
    }
}
