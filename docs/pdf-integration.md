# 50 × 12 mm PDFs as an independent consumer

[Direct IPP](direct-ipp.md) is the officially recommended/default workflow:
submit the PDF to the application's PAPPL endpoint without a system CUPS queue
or scheduler. [CUPS integration](cups-experimental.md) is Experimental.
Both use the same server and Rust PDF pipeline; the service transport remains
mock by default. GUI application support for Direct IPP must be checked per
application.

The current system endpoint accepts PDF only. PWG/URF reception is disabled
until full-document validation is proven. qpdf preflight rejects damaged PDFs
and Poppler processes a complete private copy, not a live upload stream.
The PAPPL policy patch does not alter or split valid multipage PDFs; the
one-page product limit is applied later in Rust before any submission.

Inspected read-only:
`/home/cnachtigall/git/projects/homeops-letratag-200b/`.
There is no runtime dependency and no modification there.

Observed on 2026-10-18:
* `integration/.../LetraTag_200B.php` uses millimeters and suppresses PDF
  headers/footers and automatic page breaks. The content area is derived from
  profile values.
* `profiles/letratag-200b-v0.json` specifies 50 × 12 mm, 200 **preview** dpi,
  30 active rows, and 3 mm end margins. The profile explicitly describes the
  geometry as uncalibrated.
* `pdfinfo output/pdf/letratag-200b-preview.pdf` reported nine pages,
  141.732 × 34.0157 pt, corresponding to 50 × 12 mm. The provisional PDF is
  not evidence of hardware calibration. Its actual text contents were not copied.

## Single-job contract

One PDF page represents exactly one label. Page width is feed length; page
height is tape width. The portrait dimensions in the IPP media model are
reversed: `x-dimension=1200`, `y-dimension=5000` in 1/100 mm. The raster path
rotates the portrait page without changing the content's proportions.

Direct PDF input is rasterized in Rust at the current 160 dpi software layout
resolution (`geometry::RESOLUTION_DPI`). This is not a guarantee of physical
dot pitch. The centered 30 active cross-tape rows remain 30 pixels high.
White page margins outside the printhead frame are removed without shortening
the feed columns. Nonwhite margin content is rejected. The full 12 mm is not
shrunk to 30 pixels with a corresponding reduction in label length.
`print-scaling=none` is required, and the specific job length must match the
PDF page.

For a 50 × 12 mm PDF, the direct client command is:

```sh
lp -h 127.0.0.1:8631 -d letratag -o media=custom_default_12x50mm -o print-scaling=none label.pdf
```

Here `lp` is a client of PAPPL, not of a system CUPS queue. libcups/client
dependencies do not imply that a CUPS scheduler must be running.
For other lengths, send explicit IPP `media-col` dimensions:
`x-dimension=1200`, `y-dimension=<length in 1/100 mm>`, with zero margins,
matching the PDF and within the software range 1000..20000. For example,
63.5 mm requires `1200 × 6350`. An arbitrary `media=custom_...` keyword is
not proof that the intended size was selected. See the Direct IPP guide for
custom-length submission.

The current IPP service accepts one page and one copy. The nine-page reference
file is therefore rejected in full before anything is sent. A future consumer
must generate individual label PDFs or use an established PDF tool to split
the file into separate jobs in a controlled way. No Snipe-specific splitter
is part of this driver.

Neither the Poppler path nor an Experimental CUPS conversion may silently
enable `fit-to-page` or independent X/Y scaling. PAPPL's scaling-none default
cannot undo scaling already embedded by an upstream filter. The
[resolution investigation](evidence/resolution-investigation-2026-10-21.md)
demonstrated CUPS `pdftopdf` fitting an unresolved custom-media request into
the default 50-mm page. Its offline scaling-none control preserved the source
raster, but did not verify historical scheduler-forwarded attributes or fix
production CUPS submission. Experimental CUPS acceptance requires post-filter
geometry and actual forwarded media dimensions to be checked against a
mock-only destination before any separately authorized hardware use.

The older apparent feed-pitch discrepancy near 160 versus 316 dpi remains
unresolved. Direct submission avoids this CUPS filter stage; it does not prove
native hardware pitch. A synthetic PDF is used in the isolated IPP test; the
real template remains a separate integration and hardware test.
