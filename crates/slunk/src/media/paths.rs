//! Node's `path.posix` functions that slunk's media context used, ported
//! lexically: nothing here touches the disk (design §10). Paths are strings
//! with forward slashes, so they match what slunk stored to the character.

/// An absolute path, like `path.resolve(base, path)`: `path` if it's
/// absolute, else `path` from `base`, with `.`, `..`, repeated slashes and a
/// trailing slash resolved. `..` stops at `/`. `base` must be absolute.
pub fn resolve(base: &str, path: &str) -> String {
    let joined = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{base}/{path}")
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }
    format!("/{}", segments.join("/"))
}

/// The relative path from `from` to `to`, both absolute and resolved, like
/// `path.relative()`: `""` when they're the same, and `..` for each folder up.
pub fn relative(from: &str, to: &str) -> String {
    let from: Vec<&str> = segments(from).collect();
    let to: Vec<&str> = segments(to).collect();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(from, to)| from == to)
        .count();
    let ups = std::iter::repeat_n("..", from.len() - common);
    ups.chain(to[common..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}

/// The folder an absolute, resolved path is in, like `path.dirname()`: `/`
/// for `/` itself.
pub fn dirname(path: &str) -> String {
    match path.trim_end_matches('/').rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(slash) => path[..slash].to_string(),
    }
}

/// A name inside an absolute, resolved folder.
pub fn join(folder: &str, name: &str) -> String {
    resolve(folder, name)
}

/// The last part of a path, like `path.basename()`.
pub fn basename(path: &str) -> &str {
    let path = path.trim_end_matches('/');
    path.rsplit('/').next().unwrap_or(path)
}

/// The extension, with its dot, like `path.extname()`: from the last dot of
/// the last part, unless that dot starts the name. `""` when there's none.
pub fn extname(path: &str) -> &str {
    let name = basename(path);
    if name == ".." {
        return "";
    }
    match name.rfind('.') {
        Some(0) | None => "",
        Some(dot) => {
            // A dot after only dots, like "..flac", still starts the extension
            &name[dot..]
        }
    }
}

fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|segment| !segment.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each case's expected values are what Node 26's path.posix returned.

    #[test]
    fn paths_resolve_and_relativize_as_nodes_do() {
        let root = "/Users/me/Music";
        for (path, normalized, absolute) in [
            ("", "", "/Users/me/Music"),
            (".", "", "/Users/me/Music"),
            ("a/b.flac", "a/b.flac", "/Users/me/Music/a/b.flac"),
            ("./a//b/", "a/b", "/Users/me/Music/a/b"),
            ("a/../b", "b", "/Users/me/Music/b"),
            ("../x.flac", "../x.flac", "/Users/me/x.flac"),
            ("../../..", "../../..", "/"),
            ("../../../../../..", "../../..", "/"),
            ("/Users/me/Music/a", "a", "/Users/me/Music/a"),
            ("/Users/me/Other/x", "../Other/x", "/Users/me/Other/x"),
            ("/", "../../..", "/"),
            ("/Users/me/Music", "", "/Users/me/Music"),
            ("a/./../../Music/a", "a", "/Users/me/Music/a"),
            ("..", "..", "/Users/me"),
            ("a\\b", "a\\b", "/Users/me/Music/a\\b"),
            (" a ", " a ", "/Users/me/Music/ a "),
            ("a/b/", "a/b", "/Users/me/Music/a/b"),
        ] {
            let resolved = resolve(root, path);
            assert_eq!(resolved, absolute, "resolve {path:?}");
            assert_eq!(relative(root, &resolved), normalized, "relative {path:?}");
        }
        for (from, to, expected) in [
            ("/", "/a", "a"),
            ("/a", "/", ".."),
            ("/a/b", "/a/bc", "../bc"),
            ("/a/bc", "/a/b", "../b"),
            ("/a/b", "/a/b/c/d", "c/d"),
            ("/a/b/c", "/a", "../.."),
            ("/x", "/y", "../y"),
        ] {
            assert_eq!(relative(from, to), expected, "{from} → {to}");
        }
    }

    #[test]
    fn names_folders_and_extensions_split_as_nodes_do() {
        for (path, expected) in [("/", "/"), ("/a", "/"), ("/a/b", "/a"), ("/a/b/", "/a")] {
            assert_eq!(dirname(path), expected, "{path}");
        }
        for (path, expected) in [
            ("a/b.flac", "b.flac"),
            ("b.flac", "b.flac"),
            ("../x", "x"),
            ("..", ".."),
            ("a/b/", "b"),
        ] {
            assert_eq!(basename(path), expected, "{path}");
        }
        for (path, expected) in [
            ("a.flac", ".flac"),
            (".flac", ""),
            ("..flac", ".flac"),
            ("...", "."),
            ("a.", "."),
            ("a.b.c", ".c"),
            ("a/.b", ""),
            ("a.b/c", ""),
            ("..", ""),
            ("a/b.FLAC", ".FLAC"),
        ] {
            assert_eq!(extname(path), expected, "{path}");
        }
    }
}
