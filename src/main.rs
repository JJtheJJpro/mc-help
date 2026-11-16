mod looper;

use serde::Deserialize;
use std::process::ExitCode;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct EnchantmentInfo {
    name: String,
    id: String,
    #[serde(deserialize_with = "deserialize_roman")]
    max_level: u8,
}
fn deserialize_roman<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let numeral = String::deserialize(deserializer)?;
    match numeral.as_str() {
        "I" => Ok(1),
        "II" => Ok(2),
        "III" => Ok(3),
        "IV" => Ok(4),
        "V" => Ok(5),
        _ => Err(serde::de::Error::custom(format!(
            "invalid Roman numeral: {numeral}"
        ))),
    }
}

fn main() -> ExitCode {
    const ENCH_LIST: &[u8] = include_bytes!("enchlist.csv");

    let local_data_dir = if let Some(local_data) = dirs::data_local_dir() {
        local_data.join("Minecraft Helper")
    } else {
        match dunce::canonicalize(".") {
            Ok(path) => path,
            Err(error) => {
                eprintln!("Could not find a data directory: {error}");
                return ExitCode::FAILURE;
            }
        }
    };
    if let Err(error) = std::fs::create_dir_all(&local_data_dir) {
        eprintln!("Could not create the data directory: {error}");
        return ExitCode::FAILURE;
    }

    let ench_list = match csv::Reader::from_reader(ENCH_LIST)
        .deserialize::<EnchantmentInfo>()
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(enchantments) => enchantments,
        Err(error) => {
            eprintln!("Could not load the enchantment list: {error}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = looper::looper(local_data_dir, ench_list) {
        eprintln!("Could not start Minecraft Helper: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
