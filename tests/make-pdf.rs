use std::io::Write;
fn main() {
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let stream = "0 g 20 15 100 4 re f\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 141.732283 34.015748] /Resources << >> /Contents 4 0 R >>"
            .to_string(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
    ];
    let mut offsets = vec![0];
    for (i, o) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        write!(pdf, "{} 0 obj\n{}\nendobj\n", i + 1, o).unwrap();
    }
    let xref = pdf.len();
    write!(pdf, "xref\n0 5\n0000000000 65535 f \n").unwrap();
    for o in &offsets[1..] {
        writeln!(pdf, "{o:010} 00000 n ").unwrap();
    }
    write!(
        pdf,
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )
    .unwrap();
    std::io::stdout().write_all(&pdf).unwrap();
}
