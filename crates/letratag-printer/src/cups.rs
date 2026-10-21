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
         \x20 The serve path defaults to the mock transport; real hardware is\n\
         \x20 an explicit per-invocation choice (build with --features\n\
         \x20 pappl,ble, then serve --transport bluez --address <MAC>).\n\
         \x20 There is no implicit fallback. The hardware path is\n\
         \x20 operator-in-the-loop: every job leaves an inflight journal\n\
         \x20 until the operator resolves it (docs/security.md,\n\
         \x20 docs/hardware-acceptance.md).\n\
         \n\
         Dedicated CUPS queue (validated 2026-10-21; create only with explicit\n\
         \x20 operator approval; loopback only; never touch an existing queue):\n\
         \n\
         \x20 Preflight:\n\
         \x20 - Start the local endpoint first and verify it answers\n\
         \x20   (ipptool ipp://127.0.0.1:{port}/ipp/print).\n\
         \x20 - Hardware runs: printer powered on, blueman-applet stopped (it\n\
         \x20   kills BLE discovery), fresh private journal, validated caps in\n\
         \x20   <state>/printer-caps.conf. For a no-hardware dry run, start the\n\
         \x20   endpoint without --transport bluez (mock) and expect a completed\n\
         \x20   mock job.\n\
         \n\
         \x20 Create the queue (driverless IPP Everywhere, no retries):\n\
         \x20 lpadmin -p letratag-e2e -E -v ipp://127.0.0.1:{port}/ipp/print -m everywhere\n\
         \x20 lpadmin -p letratag-e2e -o printer-error-policy=abort-job\n\
         \n\
         \x20 Print one job through CUPS:\n\
         \x20 lp -d letratag-e2e -o media=12x50mm.Borderless label.pdf\n\
         \n\
         \x20 Check the queue and the job:\n\
         \x20 lpstat -p letratag-e2e\n\
         \x20 lpoptions -p letratag-e2e -l\n\
         \x20 lpstat -W all -o letratag-e2e\n\
         \n\
         \x20 Remove the queue again (returns the system to its prior state):\n\
         \x20 lpadmin -x letratag-e2e\n\
         \n\
         \x20 Media through CUPS is named 12x50mm.Borderless (1200 x 5000 in\n\
         \x20 1/100 mm); no print-scaling option is needed (the driver defaults\n\
         \x20 to no-scaling when the attribute is absent). abort-job prevents\n\
         \x20 scheduler retries: the hardware path never reprints an uncertain\n\
         \x20 job automatically.\n\
         \n\
         \x20 Expected states: the endpoint reports the job aborted (uncertain\n\
         \x20 outcome, inflight journal); CUPS may report completed. Neither is\n\
         \x20 physical proof; inspect the label and record the journal decision.\n\
         \x20 See docs/hardware-acceptance.md section 7.\n\
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
        assert!(report.contains("12 x 50 mm"));
        assert!(report.contains("PDF only"));
        assert!(report.contains("operator-in-the-loop"));
        assert!(report.contains("docs/security.md"));
    }

    #[test]
    fn settings_report_documents_the_validated_cups_queue_flow() {
        let report = settings_report(8631);
        // Driverless queue creation with the validated queue name and the
        // no-retry error policy.
        assert!(report.contains(
            "lpadmin -p letratag-e2e -E -v ipp://127.0.0.1:8631/ipp/print -m everywhere"
        ));
        assert!(report.contains("lpadmin -p letratag-e2e -o printer-error-policy=abort-job"));
        // Print through CUPS with the CUPS-side media name, and cleanup.
        assert!(report.contains("lp -d letratag-e2e -o media=12x50mm.Borderless label.pdf"));
        assert!(report.contains("lpadmin -x letratag-e2e"));
        // Preflight, expected states and the software-vs-physical distinction.
        assert!(report.contains("blueman-applet stopped"));
        assert!(report.contains("printer-caps.conf"));
        assert!(report.contains("no print-scaling option is needed"));
        assert!(report.contains("the endpoint reports the job aborted"));
        assert!(report.contains("CUPS may report completed"));
        assert!(report.contains("Neither is"));
        assert!(report.contains("docs/hardware-acceptance.md section 7"));
    }

    #[test]
    fn settings_report_uses_the_configured_port() {
        assert!(settings_report(38631).contains("127.0.0.1:38631"));
        assert!(!settings_report(38631).contains("127.0.0.1:8631"));
    }
}
