//! Direct IPP guidance and Experimental CUPS settings (informational only).

/// Render connection guidance without installing or executing anything.
pub fn settings_report(port: u16) -> String {
    format!(
        "Direct IPP (recommended/default workflow)\n\
         \n\
         PAPPL is the shared IPP server; serve defaults to mock transport.\n\
         IPP URI: ipp://127.0.0.1:{port}/ipp/print\n\
         Printer: letratag; loopback only, no mDNS advertisement.\n\
         No system CUPS queue or scheduler is needed for Direct IPP.\n\
         The lp/lpr/ipptool client utilities may come from CUPS packages.\n\
         \n\
         Submit one 50 x 12 mm PDF directly (no system queue; choose one client):\n\
         \x20 lp -h 127.0.0.1:{port} -d letratag \\\n\
         \x20   -o media=custom_default_12x50mm -o print-scaling=none label.pdf\n\
         \x20 lpr -H 127.0.0.1:{port} -P letratag \\\n\
         \x20   -o media=custom_default_12x50mm -o print-scaling=none label.pdf\n\
         \n\
         PDF only, one page and one copy. Software media range: 12 x 10..200 mm.\n\
         Other lengths need explicit media-col dimensions matching the PDF.\n\
         See docs/direct-ipp.md for checked setup, media and status commands.\n\
         Desktop application support is not generally verified.\n\
         \n\
         Real hardware requires an explicit operator choice:\n\
         \x20 build with --features pappl,ble (established BlueZ backend) or\n\
         \x20 pappl,btleplug (alternative btleplug backend);\n\
         \x20 serve --transport bluez|btleplug --address <MAC>.\n\
         There is no implicit fallback. Hardware completion is operator-in-the-loop:\n\
         every transmission leaves an inflight journal; do not blindly resubmit.\n\
         See docs/security.md and docs/hardware-acceptance.md.\n\
         \n\
         CUPS integration: Experimental (optional, not required)\n\
         \x20 pdftopdf can fit artwork to incorrectly resolved media before PAPPL.\n\
         \x20 A submitted media keyword does not guarantee the intended dimensions.\n\
         \x20 print-scaling=none removed fitting in an offline control, but does not\n\
         \x20 establish correct forwarded media-col or physical dimensional accuracy.\n\
         \x20 The readable 50-mm CUPS acceptance label was not a dimensional calibration.\n\
         \x20 The historical 160-versus-316-dpi feed discrepancy remains unresolved.\n\
         \n\
         Optional dedicated queue, only with operator approval:\n\
         \x20 lpadmin -p letratag-e2e -E -v ipp://127.0.0.1:{port}/ipp/print -m everywhere\n\
         \x20 lpadmin -p letratag-e2e -o printer-error-policy=abort-job\n\
         \x20 lp -d letratag-e2e -o media=12x50mm.Borderless label.pdf\n\
         \x20 lpstat -W all -o letratag-e2e\n\
         \x20 lpadmin -x letratag-e2e\n\
         Never modify an existing queue. abort-job is not a guarantee against all resends.\n\
         CUPS may report completed while the endpoint reports the job aborted.\n\
         Neither is physical proof; retain the journal and inspect the output.\n\
         See docs/cups-experimental.md and\n\
         docs/evidence/resolution-investigation-2026-10-21.md.\n\
         \n\
         Offline checks (no hardware):\n\
         \x20 cargo test --locked --workspace\n\
         \x20 bash scripts/test-direct-ipp.sh\n\
         \x20 bash scripts/test-ipp.sh\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_report_recommends_direct_ipp_with_explicit_clients() {
        let report = settings_report(8631);
        assert!(report.starts_with("Direct IPP (recommended/default workflow)"));
        assert!(report.contains("ipp://127.0.0.1:8631/ipp/print"));
        assert!(report.contains("lp -h 127.0.0.1:8631 -d letratag"));
        assert!(report.contains("lpr -H 127.0.0.1:8631 -P letratag"));
        assert!(report.contains("choose one client"));
        assert!(report.contains("PDF only, one page and one copy"));
        assert!(report.contains("operator-in-the-loop"));
        assert!(report.contains("docs/direct-ipp.md"));
        assert!(report.contains("docs/security.md"));
        assert!(report.contains("--transport bluez|btleplug"));
        assert!(report.contains("pappl,btleplug"));
    }

    #[test]
    fn settings_report_keeps_cups_available_but_marks_it_experimental() {
        let report = settings_report(8631);
        assert!(report.contains("CUPS integration: Experimental"));
        assert!(report.contains(
            "lpadmin -p letratag-e2e -E -v ipp://127.0.0.1:8631/ipp/print -m everywhere"
        ));
        assert!(report.contains("lpadmin -p letratag-e2e -o printer-error-policy=abort-job"));
        assert!(report.contains("lp -d letratag-e2e -o media=12x50mm.Borderless label.pdf"));
        assert!(report.contains("lpadmin -x letratag-e2e"));
        assert!(report.contains("pdftopdf"));
        assert!(report.contains("not a guarantee against all resends"));
        assert!(!report.contains("no print-scaling option is needed"));
        assert!(report.contains("CUPS may report completed"));
        assert!(report.contains("docs/cups-experimental.md"));
    }

    #[test]
    fn settings_report_uses_the_configured_port() {
        assert!(settings_report(38631).contains("127.0.0.1:38631"));
        assert!(!settings_report(38631).contains("127.0.0.1:8631"));
    }
}
