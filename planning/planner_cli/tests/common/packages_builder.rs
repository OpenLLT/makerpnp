use csv::QuoteStyle;
use rust_decimal::Decimal;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct TestPackageRecord {
    pub name: String,
    pub size_x: Decimal,
    pub size_y: Decimal,
    pub size_z: Decimal,
    // There are more fields, but these are all we need for now
}

#[derive(Default)]
pub struct PackagesCSVBuilder<'a> {
    records: Option<&'a [TestPackageRecord]>,
}

impl<'a> PackagesCSVBuilder<'a> {
    pub fn as_string(&mut self) -> String {
        let content: Vec<u8> = vec![];

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_writer(content);

        if let Some(records) = self.records {
            for record in records.iter() {
                writer.serialize(record).unwrap();
            }
        }

        writer.flush().unwrap();

        String::from_utf8(writer.into_inner().unwrap()).unwrap()
    }
    pub fn with_items(mut self, records: &'a [TestPackageRecord]) -> Self {
        self.records = Some(records);
        self
    }

    pub fn new() -> Self {
        Default::default()
    }
}
