use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Default)]
pub struct MavenMeta {
    pub release: Option<String>,
    pub versions: Vec<String>,
}

pub fn parse_maven_metadata(xml: &str) -> MavenMeta {
    let mut reader = Reader::from_str(xml);
    let mut meta = MavenMeta::default();
    let mut in_release = false;
    let mut in_version = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.local_name();
                if name.as_ref() == b"release" {
                    in_release = true;
                } else if name.as_ref() == b"version" {
                    in_version = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_release || in_version {
                    if let Ok(t) = e.unescape() {
                        let text = t.trim().to_string();
                        if !text.is_empty() {
                            if in_release {
                                meta.release = Some(text.clone());
                            }
                            if in_version && !meta.versions.contains(&text) {
                                meta.versions.push(text);
                            }
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                if name.as_ref() == b"release" {
                    in_release = false;
                } else if name.as_ref() == b"version" {
                    in_version = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    meta
}
