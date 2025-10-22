use serde::Serialize;

impl<T: Serialize> ToFormattedJson for T {
    fn to_formatted_json(&self) -> String {
        let mut content: Vec<u8> = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut ser = serde_json::Serializer::with_formatter(&mut content, formatter);
        self.serialize(&mut ser).unwrap();
        content.push(b'\n');

        let content = String::from_utf8(content).unwrap();

        content
    }
}

pub trait ToFormattedJson {
    fn to_formatted_json(&self) -> String;
}
