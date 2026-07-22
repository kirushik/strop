//! Best-effort integration with the desktop-wide XBEL recent-file list.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

const BOOKMARK_NS: &str = "http://www.freedesktop.org/standards/desktop-bookmarks";
const MIME_NS: &str = "http://www.freedesktop.org/standards/shared-mime-info";

pub fn data_file() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map(|base| base.join("recently-used.xbel"))
}

pub fn add(path: &Path) {
    let Some(file) = data_file() else { return };
    if let Err(error) = add_at(&file, path)
        && cfg!(debug_assertions)
    {
        eprintln!("strop: could not update desktop recents: {error}");
    }
}

fn add_at(file: &Path, path: &Path) -> io::Result<()> {
    let uri = file_uri(path)?;
    let now = timestamp();
    let source = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => fresh().into_bytes(),
        Err(error) => return Err(error),
    };
    let output = update(&source, &uri, &now, path.extension().is_some_and(|ext| ext == "md"))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let Some(dir) = file.parent() else { return Err(io::Error::other("XBEL path has no parent")) };
    std::fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".recently-used.xbel.strop-{}-{}.tmp", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos()));
    let result = (|| {
        let mut handle = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
        handle.write_all(&output)?;
        handle.sync_all()?;
        std::fs::rename(&temp, file)?;
        if let Ok(directory) = std::fs::File::open(dir) { let _ = directory.sync_all(); }
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result
}

fn fresh() -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xbel version=\"1.0\" xmlns:bookmark=\"{BOOKMARK_NS}\" xmlns:mime=\"{MIME_NS}\">\n</xbel>\n")
}

fn update(source: &[u8], uri: &str, now: &str, markdown: bool) -> Result<Vec<u8>, quick_xml::Error> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::with_capacity(source.len() + 512));
    let mut found_root = false;
    let mut depth = 0usize;
    let mut root_closed = false;
    let mut matching = false;
    let mut found = false;
    let mut in_apps = false;
    let mut strop_app = false;
    loop {
        match reader.read_event()? {
            Event::Start(mut event) => {
                depth += 1;
                let local = event.local_name();
                if local.as_ref() == b"xbel" { found_root = true; }
                if local.as_ref() == b"bookmark" {
                    matching = attribute(&event, b"href").as_deref() == Some(uri);
                    found |= matching;
                    if matching { replace_attrs(&mut event, now, None); }
                } else if matching && local.as_ref() == b"applications" {
                    in_apps = true;
                } else if matching && in_apps && local.as_ref() == b"application"
                    && attribute(&event, b"name").as_deref() == Some("Strop")
                {
                    strop_app = true;
                    let count = attribute(&event, b"count").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) + 1;
                    replace_attrs(&mut event, now, Some(count));
                }
                writer.write_event(Event::Start(event))?;
            }
            Event::Empty(mut event) => {
                let local = event.local_name();
                if matching && in_apps && local.as_ref() == b"application"
                    && attribute(&event, b"name").as_deref() == Some("Strop")
                {
                    strop_app = true;
                    let count = attribute(&event, b"count").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) + 1;
                    replace_attrs(&mut event, now, Some(count));
                }
                writer.write_event(Event::Empty(event))?;
            }
            Event::End(event) => {
                if depth == 0 { return Err(quick_xml::Error::from(io::Error::new(io::ErrorKind::InvalidData, "unmatched XBEL end tag"))); }
                let local = event.local_name();
                if matching && local.as_ref() == b"applications" {
                    if !strop_app { write_application(&mut writer, now, 1)?; }
                    in_apps = false;
                }
                if local.as_ref() == b"bookmark" { matching = false; strop_app = false; }
                if local.as_ref() == b"xbel" {
                    if !found { write_bookmark(&mut writer, uri, now, markdown)?; }
                    root_closed = true;
                }
                depth -= 1;
                writer.write_event(Event::End(event))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }
    if !found_root || !root_closed || depth != 0 { return Err(quick_xml::Error::from(io::Error::new(io::ErrorKind::InvalidData, "XBEL root is missing"))); }
    Ok(writer.into_inner())
}

fn attribute(event: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    event.attributes().with_checks(true).find_map(|attr| {
        let attr = attr.ok()?;
        (attr.key.local_name().as_ref() == name).then(|| attr.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.into_owned())).flatten()
    })
}

fn replace_attrs(event: &mut BytesStart<'_>, now: &str, count: Option<u64>) {
    let attrs: Vec<(Vec<u8>, String)> = event.attributes().with_checks(false).filter_map(Result::ok)
        .filter(|a| !matches!(a.key.local_name().as_ref(), b"modified" | b"visited" | b"timestamp" | b"count"))
        .map(|a| (a.key.as_ref().to_vec(), a.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap_or_default().into_owned())).collect();
    event.clear_attributes();
    for (key, value) in &attrs { event.push_attribute((key.as_slice(), value.as_bytes())); }
    if count.is_some() { event.push_attribute(("timestamp", now)); } else {
        event.push_attribute(("modified", now)); event.push_attribute(("visited", now));
    }
    if let Some(count) = count { event.push_attribute(("count", count.to_string().as_str())); }
}

fn write_application(writer: &mut Writer<Vec<u8>>, now: &str, count: u64) -> Result<(), quick_xml::Error> {
    let mut app = BytesStart::new("bookmark:application");
    app.push_attribute(("name", "Strop")); app.push_attribute(("exec", "'strop %f'"));
    app.push_attribute(("timestamp", now)); app.push_attribute(("count", count.to_string().as_str()));
    writer.write_event(Event::Empty(app))?; Ok(())
}

fn write_bookmark(writer: &mut Writer<Vec<u8>>, uri: &str, now: &str, markdown: bool) -> Result<(), quick_xml::Error> {
    let mut bookmark = BytesStart::new("bookmark");
    bookmark.push_attribute(("href", uri)); bookmark.push_attribute(("added", now));
    bookmark.push_attribute(("modified", now)); bookmark.push_attribute(("visited", now));
    writer.write_event(Event::Start(bookmark))?;
    writer.write_event(Event::Start(BytesStart::new("info")))?;
    writer.write_event(Event::Start(BytesStart::new("metadata")))?;
    let mut mime = BytesStart::new("mime:mime-type");
    mime.push_attribute(("type", if markdown { "text/markdown" } else { "application/x-strop" }));
    writer.write_event(Event::Empty(mime))?;
    writer.write_event(Event::Start(BytesStart::new("bookmark:applications")))?;
    write_application(writer, now, 1)?;
    writer.write_event(Event::End(BytesEnd::new("bookmark:applications")))?;
    writer.write_event(Event::End(BytesEnd::new("metadata")))?;
    writer.write_event(Event::End(BytesEnd::new("info")))?;
    writer.write_event(Event::End(BytesEnd::new("bookmark")))?;
    Ok(())
}

fn file_uri(path: &Path) -> io::Result<String> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    #[cfg(unix)] let bytes = { use std::os::unix::ffi::OsStrExt; path.as_os_str().as_bytes() };
    #[cfg(not(unix))] let owned = path.to_string_lossy().into_owned();
    #[cfg(not(unix))] let bytes = owned.as_bytes();
    let mut uri = String::from("file://");
    for &byte in bytes { if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') { uri.push(byte as char); } else { uri.push_str(&format!("%{byte:02X}")); } }
    Ok(uri)
}

fn timestamp() -> String {
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let days = seconds.div_euclid(86_400); let day_seconds = seconds.rem_euclid(86_400);
    let z = days + 719_468; let era = z.div_euclid(146_097); let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153; let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 }; year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", day_seconds / 3600, day_seconds / 60 % 60, day_seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("strop-xbel-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fresh_file_roundtrips_through_xbel() {
        let dir = temp("fresh"); let file = dir.join("recently-used.xbel"); let doc = dir.join("Draft.strop");
        std::fs::write(&doc, b"").unwrap(); add_at(&file, &doc).unwrap();
        let xml = std::fs::read_to_string(&file).unwrap();
        assert!(xml.contains("application/x-strop")); assert!(xml.contains("name=\"Strop\""));
        assert!(Reader::from_str(&xml).read_event().is_ok());
    }

    #[test]
    fn foreign_entry_and_private_flag_survive() {
        let dir = temp("foreign"); let file = dir.join("recently-used.xbel"); let doc = dir.join("Draft.strop"); std::fs::write(&doc, b"").unwrap();
        let foreign = format!("<?xml version=\"1.0\"?><xbel version=\"1.0\" xmlns:bookmark=\"{BOOKMARK_NS}\"><bookmark href=\"file:///other\"><info><metadata><bookmark:private/><alien key=\"value\">payload</alien></metadata></info></bookmark></xbel>");
        std::fs::write(&file, foreign).unwrap(); add_at(&file, &doc).unwrap(); let xml = std::fs::read_to_string(file).unwrap();
        assert!(xml.contains("bookmark:private")); assert!(xml.contains("<alien key=\"value\">payload</alien>")); assert!(xml.contains("file:///other"));
    }

    #[test]
    fn dedupe_updates_instead_of_duplicating() {
        let dir = temp("dedupe"); let file = dir.join("recently-used.xbel"); let doc = dir.join("Draft.strop"); std::fs::write(&doc, b"").unwrap();
        add_at(&file, &doc).unwrap(); add_at(&file, &doc).unwrap(); let xml = std::fs::read_to_string(file).unwrap(); let uri = file_uri(&doc).unwrap();
        assert_eq!(xml.matches(&format!("href=\"{uri}\"")).count(), 1); assert!(xml.contains("count=\"2\""));
    }

    #[test]
    fn malformed_input_is_left_untouched() {
        let dir = temp("malformed"); let file = dir.join("recently-used.xbel"); let doc = dir.join("Draft.strop"); std::fs::write(&doc, b"").unwrap();
        let malformed = b"<xbel><bookmark>"; std::fs::write(&file, malformed).unwrap(); assert!(add_at(&file, &doc).is_err());
        assert_eq!(std::fs::read(file).unwrap(), malformed);
    }

    #[test]
    fn atomic_replace_path_is_exercised_without_temp_residue() {
        let dir = temp("atomic"); let file = dir.join("recently-used.xbel"); let doc = dir.join("Draft.strop"); std::fs::write(&doc, b"").unwrap();
        add_at(&file, &doc).unwrap(); assert!(file.exists());
        assert!(std::fs::read_dir(dir).unwrap().all(|entry| !entry.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
    }
}
