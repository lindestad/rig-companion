//! Offline estimates: stable between sessions, independent of lap/track speed records.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct Car {
    pub name: String,
    pub aliases: Vec<String>,
    pub top_speed_kmh: u16,
    pub basis: String,
}
#[derive(Deserialize)]
struct Catalogue {
    cars: Vec<Car>,
}
pub fn catalogue() -> &'static [Car] {
    static CARS: OnceLock<Catalogue> = OnceLock::new();
    &CARS
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/wind-car-speeds.json"))
                .expect("checked-in car catalogue")
        })
        .cars
}
fn normalized(s: &str) -> String {
    s.to_lowercase()
        .replace("[legacy]", "")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}
pub fn is_iracing(game: &str) -> bool {
    normalized(game) == "iracing"
}

#[derive(Debug, Clone)]
pub struct Estimate {
    pub car: String,
    pub top_speed_kmh: u16,
    pub basis: String,
}
impl Estimate {
    pub fn full_speed_kmh(&self) -> u16 {
        self.top_speed_kmh.saturating_sub(15).max(10)
    }
}
pub fn estimate(game: &str, id: &str, model: &str, class: &str, fallback: u16) -> Estimate {
    let fallback_estimate = || Estimate {
        car: if model.is_empty() {
            "No identified car".into()
        } else {
            model.into()
        },
        top_speed_kmh: fallback,
        basis: "Fallback estimate · adjust below if needed".into(),
    };
    if !is_iracing(game) {
        return fallback_estimate();
    }
    let keys = [normalized(id), normalized(model)];
    // Prefer exact identifiers/names. Never interpret numeric SimHub IDs as another game's IDs.
    let exact = catalogue().iter().find(|c| {
        c.aliases.iter().any(|a| {
            let a = normalized(a);
            !a.is_empty() && keys.iter().any(|k| !k.is_empty() && *k == a)
        })
    });
    // SimHub may append model/year labels. Accept only an unambiguous longest alias.
    let mut partial: Vec<_> = catalogue()
        .iter()
        .filter_map(|c| {
            c.aliases
                .iter()
                .map(|a| normalized(a))
                .filter(|a| a.len() >= 8 && keys.iter().any(|k| k.contains(a)))
                .map(|a| a.len())
                .max()
                .map(|n| (n, c))
        })
        .collect();
    partial.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    let matched = exact.or_else(|| match partial.as_slice() {
        [(n, c), rest @ ..] if rest.first().is_none_or(|(next, _)| next < n) => Some(*c),
        _ => None,
    });
    if let Some(c) = matched {
        return Estimate {
            car: c.name.clone(),
            top_speed_kmh: c.top_speed_kmh,
            basis: c.basis.clone(),
        };
    }
    let label = normalized(&format!("{model} {class}"));
    let groups = [
        ("formulavee", 165),
        ("gt3cup", 290),
        ("porschecup", 290),
        ("gt3", 285),
        ("gt4", 265),
        ("gte", 300),
        ("gtp", 335),
        ("lmp2", 325),
        ("lmp3", 290),
        ("tcr", 265),
        ("formula4", 240),
        ("nascartruck", 300),
        ("xfinity", 310),
        ("nextgen", 315),
    ];
    if let Some((_, speed)) = groups.iter().find(|(key, _)| label.contains(key)) {
        return Estimate {
            car: model.into(),
            top_speed_kmh: *speed,
            basis: "Class estimate · car not in catalogue".into(),
        };
    }
    fallback_estimate()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_official_car_and_alias_resolves() {
        let identities: serde_json::Value =
            serde_json::from_str(include_str!("../data/iracing-car-identities.json")).unwrap();
        for car in identities["cars"].as_array().unwrap() {
            let e = estimate(
                "IRacing",
                car["path"].as_str().unwrap(),
                car["name"].as_str().unwrap(),
                "",
                250,
            );
            assert!(!e.basis.contains("Fallback"), "{car}");
        }
        for car in catalogue() {
            assert!((100..=400).contains(&car.top_speed_kmh));
            for alias in &car.aliases {
                assert_eq!(
                    estimate("iRacing", alias, "", "", 250).top_speed_kmh,
                    car.top_speed_kmh,
                    "{alias}"
                );
            }
        }
    }
    #[test]
    fn separates_cup_gt3_new_cars_and_other_games() {
        assert_eq!(
            estimate("iRacing", "porsche992cup", "", "", 250).full_speed_kmh(),
            275
        );
        assert_eq!(
            estimate("iRacing", "porsche992rgt3", "", "", 250).full_speed_kmh(),
            270
        );
        assert_eq!(
            estimate("iRacing", "", "BMW M2 Racing (G87)", "", 250).full_speed_kmh(),
            255
        );
        assert_eq!(
            estimate("iRacing", "", "Formula Vee - Conqueror", "", 250).full_speed_kmh(),
            150
        );
        assert_eq!(
            estimate("iRacing", "9999", "New GT3", "GT3", 250).full_speed_kmh(),
            270
        );
        assert_eq!(
            estimate("iRacing", "9999", "Unknown", "", 250).full_speed_kmh(),
            235
        );
        assert_eq!(
            estimate("Other game", "porsche992rgt3", "", "", 250).full_speed_kmh(),
            235
        );
    }
}
