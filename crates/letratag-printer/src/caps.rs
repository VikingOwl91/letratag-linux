//! Hardware-validated printer write capabilities, persisted per device
//! address in `<state>/printer-caps.conf`.
//!
//! Only operator-confirmed limits are recorded. An acknowledged ATT write
//! does not prove the printer processed or physically printed the job, so
//! nothing is promoted automatically. The file is optional: without an entry,
//! `auto` chunk mode falls back to the conservative 20-byte limit.
//!
//! Format: one `<address> <bytes>` pair per line, `#` comments and blank
//! lines ignored. Example:
//!
//! ```text
//! # Hardware-validated GATT write-value caps (operator-confirmed prints).
//! 90:E5:B1:49:DA:AE 244
//! ```

use std::collections::HashMap;
use std::path::Path;

pub const FILE_NAME: &str = "printer-caps.conf";

/// Protocol bounds for one GATT write value: 500 body bytes plus the 1-byte
/// chunk index and the 2-byte final magic.
const MIN_CAP: usize = 9;
const MAX_CAP: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: malformed line {line}", FILE_NAME)]
    Malformed { line: usize },
    #[error("{FILE_NAME}: capability {cap} is outside the protocol bounds {MIN_CAP}..={MAX_CAP}")]
    OutOfRange { cap: usize },
    #[error("capabilities file I/O failed")]
    Io(#[from] std::io::Error),
}

fn normalize(address: &str) -> String {
    address.trim().to_ascii_uppercase()
}

fn is_address(text: &str) -> bool {
    let groups: Vec<_> = text.split(':').collect();
    groups.len() == 6
        && groups
            .iter()
            .all(|group| group.len() == 2 && group.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Load validated caps keyed by normalized address. A missing file is an
/// empty map; a malformed file is an error, never silently ignored.
pub fn load(directory: &Path) -> Result<HashMap<String, usize>, Error> {
    let text = match std::fs::read_to_string(directory.join(FILE_NAME)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(e.into()),
    };
    let mut caps = HashMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        let malformed = || Error::Malformed { line: index + 1 };
        let [address, value] = fields[..] else {
            return Err(malformed());
        };
        if !is_address(address) {
            return Err(malformed());
        }
        let cap: usize = value.parse().map_err(|_| malformed())?;
        if !(MIN_CAP..=MAX_CAP).contains(&cap) {
            return Err(Error::OutOfRange { cap });
        }
        caps.insert(normalize(address), cap);
    }
    Ok(caps)
}

/// The validated write capability for one device address, if recorded.
pub fn lookup(directory: &Path, address: &str) -> Result<Option<usize>, Error> {
    Ok(load(directory)?.get(&normalize(address)).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn missing_file_is_an_empty_map() {
        let dir = dir();
        assert_eq!(load(dir.path()).unwrap(), HashMap::new());
        assert_eq!(lookup(dir.path(), "90:E5:B1:49:DA:AE").unwrap(), None);
    }

    #[test]
    fn entries_comments_and_case_are_normalized() {
        let dir = dir();
        std::fs::write(
            dir.path().join(FILE_NAME),
            "# validated caps\n\n90:e5:b1:49:da:ae 244\nAA:BB:CC:DD:EE:FF 20\n",
        )
        .unwrap();
        let caps = load(dir.path()).unwrap();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps["90:E5:B1:49:DA:AE"], 244);
        assert_eq!(lookup(dir.path(), "90:E5:B1:49:DA:AE").unwrap(), Some(244));
        assert_eq!(lookup(dir.path(), "aa:bb:cc:dd:ee:ff").unwrap(), Some(20));
        assert_eq!(lookup(dir.path(), "11:22:33:44:55:66").unwrap(), None);
    }

    #[test]
    fn malformed_lines_and_out_of_range_caps_are_errors() {
        let dir = dir();
        for content in [
            "not-an-address 244\n",
            "90:E5:B1:49:DA:AE\n",
            "90:E5:B1:49:DA:AE 244 extra\n",
            "90:E5:B1:49:DA:AE abc\n",
            "90:E5:B1:49:DA:AE 8\n",
            "90:E5:B1:49:DA:AE 513\n",
        ] {
            std::fs::write(dir.path().join(FILE_NAME), content).unwrap();
            assert!(load(dir.path()).is_err(), "accepted: {content:?}");
        }
    }

    #[test]
    fn boundary_caps_are_accepted() {
        let dir = dir();
        std::fs::write(
            dir.path().join(FILE_NAME),
            "90:E5:B1:49:DA:AE 9\nAA:BB:CC:DD:EE:FF 512\n",
        )
        .unwrap();
        let caps = load(dir.path()).unwrap();
        assert_eq!(caps["90:E5:B1:49:DA:AE"], 9);
        assert_eq!(caps["AA:BB:CC:DD:EE:FF"], 512);
    }
}
