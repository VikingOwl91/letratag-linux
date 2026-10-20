# 50 × 12 mm PDFs as an independent consumer

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

Direct PDF input is rasterized in Rust at **the same** provisional layout
resolution. The centered 30 active cross-tape rows remain 30 pixels high.
White page margins outside the printhead frame are removed without shortening
the feed columns. Nonwhite margin content is rejected. The full 12 mm is not
shrunk to 30 pixels with a corresponding reduction in label length.
`print-scaling=none` is required, and the specific job length must match the
PDF page.

The current IPP service accepts one page and one copy. The nine-page reference
file is therefore rejected in full before anything is sent. A future consumer
must generate individual label PDFs or use an established PDF tool to split
the file into separate jobs in a controlled way. No Snipe-specific splitter
is part of this driver.

Neither the Poppler path nor a CUPS conversion may silently enable `fit-to-page`
or independent X/Y scaling. A future CUPS queue also requires media size,
orientation, and print margins to be checked with a pixel/millimeter
calibration label. A synthetic PDF is used in the isolated IPP test; the real
template remains a separate integration and hardware test.
