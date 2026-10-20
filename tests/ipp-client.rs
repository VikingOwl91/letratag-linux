use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

fn attribute(out: &mut Vec<u8>, tag: u8, name: &str, value: &str) {
    out.push(tag);
    out.extend_from_slice(&(name.len() as u16).to_be_bytes());
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(value.as_bytes());
}

// Deliberately incomplete test client, not a server or production IPP library.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args[1].parse().unwrap();
    let pdf = std::fs::read(&args[2]).unwrap();
    let mut ipp = vec![2, 0, 0, 2, 0, 0, 0, 1, 1];
    attribute(&mut ipp, 0x47, "attributes-charset", "utf-8");
    attribute(&mut ipp, 0x48, "attributes-natural-language", "en");
    attribute(
        &mut ipp,
        0x45,
        "printer-uri",
        &format!("ipp://127.0.0.1:{port}/ipp/print"),
    );
    attribute(&mut ipp, 0x42, "requesting-user-name", "test");
    attribute(&mut ipp, 0x49, "document-format", "application/pdf");
    ipp.push(2);
    attribute(&mut ipp, 0x44, "media", "custom_default_12x50mm");
    attribute(&mut ipp, 0x44, "print-scaling", "none");
    ipp.push(3);
    ipp.extend_from_slice(&pdf);
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let header = if args[3] == "short" {
        format!(
            "POST /ipp/print HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Type: application/ipp\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            ipp.len() + 100
        )
    } else {
        format!(
            "POST /ipp/print HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Type: application/ipp\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
            ipp.len()
        )
    };
    stream.write_all(header.as_bytes()).unwrap();
    stream.write_all(&ipp).unwrap();
    // Missing declared bytes or the terminating zero-length chunk.
    stream.shutdown(Shutdown::Write).unwrap();
    let mut reply = Vec::new();
    let _ = stream.take(65536).read_to_end(&mut reply);
    println!("incomplete-upload test response bytes: {}", reply.len());
}
