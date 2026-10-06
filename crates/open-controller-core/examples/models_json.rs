//! Prints the model table as JSON, for the website's compatibility list.
//! `cargo run -p open-controller-core --example models_json > models.json`

use open_controller_core::extras;
use open_controller_core::models::MODELS;

fn main() {
    let rows: Vec<serde_json::Value> = MODELS
        .iter()
        .map(|&(vendor, product, family, name)| {
            serde_json::json!({
                "vendor": format!("{vendor:04x}"),
                "product": format!("{product:04x}"),
                "family": family,
                "name": name,
                "art": extras::art(family, vendor, product),
                "hint": extras::hint(vendor, product, family),
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
