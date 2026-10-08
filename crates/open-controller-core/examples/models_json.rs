//! Prints the model table as JSON, for the website's compatibility list.
//! `cargo run -p open-controller-core --example models_json > models.json`

use open_controller_core::extras;
use open_controller_core::models::{self, MODELS};
use open_controller_core::rating;

fn main() {
    let mut rows: Vec<serde_json::Value> = MODELS
        .iter()
        .map(|&(vendor, product, family, name)| {
            let hint = extras::hint(vendor, product, family);
            serde_json::json!({
                "vendor": format!("{vendor:04x}"),
                "product": format!("{product:04x}"),
                "family": family,
                "name": name,
                "art": extras::art(family, vendor, product),
                "hint": hint,
                "rating": rating::rating(vendor, product, family, hint),
                "drawing": models::drawing(vendor, product),
            })
        })
        .collect();
    // Models that copy another's ids: listed under their own name, drawn as themselves, and
    // otherwise what the model they copy is.
    for &(vendor, product, _, brand, name, drawing) in models::ALIASES {
        let Some(&(_, _, family, original)) = models::lookup(vendor, product) else { continue };
        let hint = extras::hint(vendor, product, family);
        rows.push(serde_json::json!({
            "vendor": format!("{vendor:04x}"),
            "product": format!("{product:04x}"),
            "family": family,
            "name": name,
            "art": extras::art(family, vendor, product),
            "hint": hint,
            "rating": rating::rating(vendor, product, family, hint),
            "drawing": drawing,
            "brand": brand,
            "alias_of": original,
        }));
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
