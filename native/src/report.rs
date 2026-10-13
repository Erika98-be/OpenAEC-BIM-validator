//! Small standalone PDF report writer (text pages, no external runtime).
use std::io::Write;
fn escape(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}
pub fn pdf(mut writer: impl Write, title: &str, lines: &[String]) -> Result<(), String> {
    let pages: Vec<_> = lines.chunks(43).collect();
    let pages = if pages.is_empty() {
        vec![&[][..]]
    } else {
        pages
    };
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        Vec::new(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];
    let mut ids = vec![];
    for (number, lines) in pages.iter().enumerate() {
        let id = objects.len() + 1;
        ids.push(id);
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",id+1).into_bytes());
        let mut stream = format!(
            "BT /F1 18 Tf 45 790 Td ({}) Tj /F1 10 Tf 0 -30 Td",
            escape(title)
        );
        for line in *lines {
            stream += &format!(" ({}) Tj 0 -16 Td", escape(line));
        }
        stream += &format!(
            " ET BT /F1 9 Tf 45 25 Td (OpenAEC BIM Validator - pagina {}) Tj ET",
            number + 1
        );
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{}\nendstream",
                stream.len(),
                stream
            )
            .into_bytes(),
        );
    }
    objects[1] = format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        ids.len(),
        ids.iter()
            .map(|id| format!("{id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ")
    )
    .into_bytes();
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        bytes.extend_from_slice(object);
        bytes.extend_from_slice(b"\nendobj\n");
    }
    let xref = bytes.len();
    bytes.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    writer.write_all(&bytes).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multipage_report_offsets_point_to_actual_objects() {
        let lines = (0..90)
            .map(|i| format!("Cluster {i}: 25 punten, 42.0 mm"))
            .collect::<Vec<_>>();
        let mut bytes = vec![];
        pdf(&mut bytes, "Afwijkingen (test)", &lines).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("/Count 3"));
        let xref = text
            .split("startxref\n")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert_eq!(&bytes[xref..xref + 4], b"xref");
        if let Ok(dir) = std::env::var("OPENAEC_BCF_TEST_OUTPUT") {
            std::fs::write(
                std::path::Path::new(&dir).join("deviation-report.pdf"),
                bytes,
            )
            .unwrap();
        }
    }
}
