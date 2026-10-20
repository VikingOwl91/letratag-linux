//! CUPS integration settings and setup instructions (informational only;
//! nothing is installed or executed here).

/// Render the CUPS settings report for the local PAPPL endpoint on `port`.
pub fn settings_report(port: u16) -> String {
    format!(
        "CUPS integration settings (letratag-linux)\n\
         \n\
         Local PAPPL endpoint (mock transport by default):\n\
         \x20 IPP URI:      ipp://127.0.0.1:{port}/ipp/print\n\
         \x20 Printer name: letratag\n\
         \x20 Bind:         127.0.0.1 only (loopback, no mDNS advertisement)\n\
         \n\
         Submit a job:\n\
         \x20 lp -h 127.0.0.1:{port} -d letratag \\\n\
         \x20   -o media=custom_default_12x50mm -o print-scaling=none label.pdf\n\
         \x20 lpr -H 127.0.0.1:{port} -P letratag label.pdf\n\
         \n\
         Media profile (PDF only):\n\
         \x20 12 mm tape width, 10..200 mm length, default 12 x 50 mm.\n\
         \x20 Other lengths need an explicit IPP media collection; unsupported\n\
         \x20 media or options are rejected (no silent substitution).\n\
         \n\
         Document policy:\n\
         \x20 PDF only; PWG Raster and URF are rejected before submission.\n\
         \x20 One page and one copy per job; no automatic retries.\n\
         \n\
         Experimental BLE transport:\n\
         \x20 The serve path is mock-only today. BLE wiring (serve\n\
         \x20 --transport bluez --address <MAC>) is in preparation. The\n\
         \x20 hardware path is operator-in-the-loop: every job leaves an\n\
         \x20 inflight journal until the operator resolves it\n\
         \x20 (docs/security.md, docs/hardware-acceptance.md).\n\
         \n\
         System-wide queue (requires separate operator approval; NOT installed):\n\
         \x20 1. Start the local endpoint, e.g. via the systemd template in\n\
         \x20    packaging/systemd/letratag-linux.service (not installed).\n\
         \x20 2. Register a NEW IPP destination (do not touch existing queues):\n\
         \x20    lpadmin -p letratag -E -v ipp://127.0.0.1:{port}/ipp/print\n\
         \x20 3. Verify:\n\
         \x20    lpstat -p letratag -v\n\
         \n\
         Service-user permissions (before any hardware-backed queue):\n\
         \x20 Check the non-root service user's BlueZ/D-Bus authorization\n\
         \x20 (docs/security.md). This project installs no global Bluetooth\n\
         \x20 policy changes.\n\
         \n\
         Test suite (offline, no hardware):\n\
         \x20 cargo test --locked --workspace\n\
         \x20 bash scripts/test-ipp.sh\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_report_contains_uri_commands_and_policy() {
        let report = settings_report(8631);
        assert!(report.contains("ipp://127.0.0.1:8631/ipp/print"));
        assert!(report.contains("lp -h 127.0.0.1:8631 -d letratag"));
        assert!(report.contains("lpr -H 127.0.0.1:8631 -P letratag"));
        assert!(report.contains("lpadmin -p letratag -E -v ipp://127.0.0.1:8631/ipp/print"));
        assert!(report.contains("12 x 50 mm"));
        assert!(report.contains("PDF only"));
        assert!(report.contains("operator-in-the-loop"));
        assert!(report.contains("docs/security.md"));
    }

    #[test]
    fn settings_report_uses_the_configured_port() {
        assert!(settings_report(38631).contains("127.0.0.1:38631"));
        assert!(!settings_report(38631).contains("127.0.0.1:8631"));
    }
}
