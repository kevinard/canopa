//! A constrained arithmetic/conversion language, independent of qalc commands.

type Dimension = [i8; 6]; // length, mass, time, temperature, information, angle
const LENGTH: Dimension = [1, 0, 0, 0, 0, 0];
const MASS: Dimension = [0, 1, 0, 0, 0, 0];
const TIME: Dimension = [0, 0, 1, 0, 0, 0];
const TEMPERATURE: Dimension = [0, 0, 0, 1, 0, 0];
const DATA: Dimension = [0, 0, 0, 0, 1, 0];
const ANGLE: Dimension = [0, 0, 0, 0, 0, 1];
const VOLUME: Dimension = [3, 0, 0, 0, 0, 0];
const AREA: Dimension = [2, 0, 0, 0, 0, 0];
const PRESSURE: Dimension = [-1, 1, -2, 0, 0, 0];
const ENERGY: Dimension = [2, 1, -2, 0, 0, 0];
const POWER: Dimension = [2, 1, -3, 0, 0, 0];
const SPEED: Dimension = [1, 0, -1, 0, 0, 0];

struct UnitDefinition {
    code: &'static str,
    label: &'static str,
    dimension: Dimension,
    symbols: &'static str,
    names: &'static str,
}

macro_rules! unit {
    ($code:literal, $label:literal, $dimension:ident, $symbols:literal, $names:literal) => {
        UnitDefinition {
            code: $code,
            label: $label,
            dimension: $dimension,
            symbols: $symbols,
            names: $names,
        }
    };
}

// Symbols remain case sensitive (MB is bytes; Mb is bits). English names may
// vary in case. Conversion factors and temperature offsets belong to Qalculate.
const UNITS: &[UnitDefinition] = &[
    unit!("m", "m", LENGTH, "", "meter|meters|metre|metres"),
    unit!(
        "km",
        "km",
        LENGTH,
        "",
        "kilometer|kilometers|kilometre|kilometres"
    ),
    unit!(
        "cm",
        "cm",
        LENGTH,
        "",
        "centimeter|centimeters|centimetre|centimetres"
    ),
    unit!(
        "mm",
        "mm",
        LENGTH,
        "",
        "millimeter|millimeters|millimetre|millimetres"
    ),
    unit!(
        "um",
        "µm",
        LENGTH,
        "µm|μm",
        "micrometer|micrometers|micrometre|micrometres"
    ),
    unit!(
        "nm",
        "nm",
        LENGTH,
        "",
        "nanometer|nanometers|nanometre|nanometres"
    ),
    unit!("in", "in", LENGTH, "", "inch|inches"),
    unit!("ft", "ft", LENGTH, "", "foot|feet"),
    unit!("yd", "yd", LENGTH, "", "yard|yards"),
    unit!("mi", "mi", LENGTH, "", "mile|miles"),
    unit!("kg", "kg", MASS, "", "kilogram|kilograms"),
    unit!("g", "g", MASS, "", "gram|grams"),
    unit!("mg", "mg", MASS, "", "milligram|milligrams"),
    unit!("ug", "µg", MASS, "µg|μg", "microgram|micrograms"),
    unit!("lb", "lb", MASS, "", "pound|pounds|lbs"),
    unit!("oz", "oz", MASS, "", "ounce|ounces"),
    unit!("t", "t", MASS, "", "tonne|tonnes|metric ton|metric tons"),
    unit!("s", "s", TIME, "", "second|seconds|sec"),
    unit!("ms", "ms", TIME, "", "millisecond|milliseconds"),
    unit!("us", "µs", TIME, "µs|μs", "microsecond|microseconds"),
    unit!("min", "min", TIME, "", "minute|minutes"),
    unit!("h", "h", TIME, "", "hour|hours|hr|hrs"),
    unit!("d", "d", TIME, "", "day|days"),
    unit!("week", "week", TIME, "wk", "weeks"),
    unit!("oC", "°C", TEMPERATURE, "C|c|°C|°c|degC", "celsius"),
    unit!("oF", "°F", TEMPERATURE, "F|f|°F|°f|degF", "fahrenheit"),
    unit!("K", "K", TEMPERATURE, "", "kelvin|kelvins"),
    unit!("bit", "bit", DATA, "b", "bits"),
    unit!("kbit", "kbit", DATA, "kb", "kilobit|kilobits"),
    unit!("Mbit", "Mbit", DATA, "Mb", "megabit|megabits"),
    unit!("Gbit", "Gbit", DATA, "Gb", "gigabit|gigabits"),
    unit!("Tbit", "Tbit", DATA, "Tb", "terabit|terabits"),
    unit!("B", "B", DATA, "", "byte|bytes"),
    unit!("kB", "kB", DATA, "KB", "kilobyte|kilobytes"),
    unit!("MB", "MB", DATA, "", "megabyte|megabytes"),
    unit!("GB", "GB", DATA, "", "gigabyte|gigabytes"),
    unit!("TB", "TB", DATA, "", "terabyte|terabytes"),
    unit!("KiB", "KiB", DATA, "", "kibibyte|kibibytes"),
    unit!("MiB", "MiB", DATA, "", "mebibyte|mebibytes"),
    unit!("GiB", "GiB", DATA, "", "gibibyte|gibibytes"),
    unit!("TiB", "TiB", DATA, "", "tebibyte|tebibytes"),
    unit!("L", "L", VOLUME, "l", "liter|liters|litre|litres"),
    unit!(
        "mL",
        "mL",
        VOLUME,
        "ml",
        "milliliter|milliliters|millilitre|millilitres"
    ),
    unit!(
        "gal",
        "US gal",
        VOLUME,
        "",
        "gallon|gallons|US gal|US gallon|US gallons"
    ),
    unit!("ha", "ha", AREA, "", "hectare|hectares"),
    unit!("acre", "acre", AREA, "ac", "acres"),
    unit!("mph", "mph", SPEED, "", "miles per hour"),
    unit!("Pa", "Pa", PRESSURE, "", "pascal|pascals"),
    unit!("kPa", "kPa", PRESSURE, "", "kilopascal|kilopascals"),
    unit!("bar", "bar", PRESSURE, "", "bars"),
    unit!("atm", "atm", PRESSURE, "", "atmosphere|atmospheres"),
    unit!("psi", "psi", PRESSURE, "", ""),
    unit!("J", "J", ENERGY, "", "joule|joules"),
    unit!("kJ", "kJ", ENERGY, "", "kilojoule|kilojoules"),
    unit!("cal", "cal", ENERGY, "cal_th", "calorie|calories"),
    unit!(
        "kcal",
        "kcal",
        ENERGY,
        "kcal_th",
        "kilocalorie|kilocalories"
    ),
    unit!("Wh", "Wh", ENERGY, "W*h", "watt hour|watt hours"),
    unit!("kWh", "kWh", ENERGY, "kW*h", "kilowatt hour|kilowatt hours"),
    unit!("W", "W", POWER, "", "watt|watts"),
    unit!("kW", "kW", POWER, "", "kilowatt|kilowatts"),
    unit!("rad", "rad", ANGLE, "", "radian|radians"),
    unit!("deg", "°", ANGLE, "°", "degree|degrees"),
];

#[derive(Debug, Clone)]
struct Unit {
    code: String,
    label: String,
    dimension: Dimension,
}

fn atomic_unit(input: &str) -> Option<Unit> {
    UNITS
        .iter()
        .find(|unit| {
            unit.code == input
                || unit
                    .symbols
                    .split('|')
                    .any(|symbol| !symbol.is_empty() && symbol == input)
                || unit
                    .names
                    .split('|')
                    .any(|name| !name.is_empty() && name.eq_ignore_ascii_case(input))
        })
        .map(|unit| Unit {
            code: unit.code.into(),
            label: unit.label.into(),
            dimension: unit.dimension,
        })
}

fn factor(input: &str) -> Option<Unit> {
    let input = input.trim();
    if let Some(unit) = atomic_unit(input) {
        return Some(unit);
    }
    let lowercase = input.to_ascii_lowercase();
    let (base, power) = if let Some(base) = input.strip_suffix("^2") {
        (base, 2)
    } else if let Some(base) = input.strip_suffix("^3") {
        (base, 3)
    } else if lowercase.starts_with("square ") {
        (&input[7..], 2)
    } else if lowercase.starts_with("cubic ") {
        (&input[6..], 3)
    } else {
        return None;
    };
    let unit = atomic_unit(base.trim())?;
    if unit.dimension == TEMPERATURE {
        return None;
    }
    Some(Unit {
        code: format!("{}^{power}", unit.code),
        label: format!("{}{}", unit.label, if power == 2 { '²' } else { '³' }),
        dimension: unit.dimension.map(|dimension| dimension * power),
    })
}

fn parse_unit(input: &str) -> Option<Unit> {
    if input.is_empty() || input.len() > 64 {
        return None;
    }
    let input = input.replace('²', "^2").replace('³', "^3");
    if let Some(unit) = factor(&input) {
        return Some(unit);
    }
    let (numerator, denominator) = input.split_once('/').or_else(|| {
        let position = input.to_ascii_lowercase().find(" per ")?;
        Some((&input[..position], &input[position + 5..]))
    })?;
    let numerator = factor(numerator)?;
    let denominator = factor(denominator)?;
    if numerator.dimension == TEMPERATURE || denominator.dimension == TEMPERATURE {
        return None;
    }
    let mut dimension = numerator.dimension;
    for (value, denominator) in dimension.iter_mut().zip(denominator.dimension) {
        *value -= denominator;
    }
    Some(Unit {
        code: format!("{}/{}", numerator.code, denominator.code),
        label: format!("{}/{}", numerator.label, denominator.label),
        dimension,
    })
}

#[derive(Debug)]
pub(super) struct Expression {
    pub source: String,
    pub automatic: bool,
    target: Option<Unit>,
}

impl Expression {
    pub fn parse(query: &str) -> Result<Self, &'static str> {
        if query.len() > 256 {
            return Err("Calculations are limited to 256 bytes");
        }
        let input = query
            .replace("->", " to ")
            .replace('→', " to ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let lower = input.to_ascii_lowercase();
        let separator = lower.rfind(" to ").or_else(|| lower.rfind(" in "));
        if let Some(position) = separator {
            let (quantity, target) = input.split_at(position);
            let target = parse_unit(target[4..].trim()).ok_or("Choose a supported target unit")?;
            let (amount, end) =
                super::math::prefix(quantity).ok_or("Enter a complete numerical amount")?;
            let unit =
                parse_unit(quantity[end..].trim()).ok_or("Choose a supported source unit")?;
            if unit.dimension != target.dimension {
                return Err("These units measure different quantities");
            }
            Ok(Self {
                source: format!("({amount}) {} to -{}", unit.code, target.code),
                automatic: true,
                target: Some(target),
            })
        } else {
            let source = arithmetic(&input)
                .ok_or("Enter a complete arithmetic expression or unit conversion")?;
            Ok(Self {
                automatic: source.contains(['+', '-', '*', '/', '^', '%'])
                    || (source.contains('(') && source.chars().any(|ch| ch.is_ascii_alphabetic())),
                source,
                target: None,
            })
        }
    }

    pub fn units_enabled(&self) -> bool {
        self.target.is_some()
    }

    pub fn result(&self, output: &str) -> Option<String> {
        let output = output.trim().trim_start_matches(['=', '≈', ' ']);
        if output.len() > 512 {
            return None;
        }
        match &self.target {
            None => numeric_result(output).then(|| output.to_string()),
            Some(target) => {
                let (number, unit) = output.split_once(char::is_whitespace)?;
                let actual = parse_unit(unit.trim())?;
                (numeric_result(number) && actual.code == target.code)
                    .then(|| format!("{number} {}", target.label))
            }
        }
    }
}

fn numeric_result(value: &str) -> bool {
    let value = value.strip_prefix(['+', '-']).unwrap_or(value);
    let (number, exponent) = value
        .split_once(['e', 'E'])
        .map_or((value, None), |(number, exponent)| (number, Some(exponent)));
    let mut digits = 0;
    let mut dots = 0;
    for ch in number.chars() {
        match ch {
            '0'..='9' => digits += 1,
            '.' => dots += 1,
            _ => return false,
        }
    }
    digits > 0
        && dots <= 1
        && exponent.is_none_or(|exponent| {
            let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            !exponent.is_empty() && exponent.chars().all(|ch| ch.is_ascii_digit())
        })
}

/// Recognizes pure arithmetic without exposing qalc's command language.
fn arithmetic(input: &str) -> Option<String> {
    super::math::parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_is_complete_bounded_and_has_no_command_language() {
        for expression in ["12 * (8 + 3)", "-2 * 3", "0.1+0.2", "1,5 + 2", "2^10"] {
            assert!(arithmetic(expression).is_some(), "{expression}");
        }
        for expression in [
            "/save", "exrates", "2; quit", "2 +", "(2+3", "2 3", "2..3", "2(3)", "()", "1e9",
        ] {
            assert!(arithmetic(expression).is_none(), "{expression}");
        }
    }

    #[test]
    fn conversion_aliases_compounds_powers_and_dimensions() {
        for expression in [
            "10cm to meters",
            "1,5 kilometres in m",
            "32 F -> C",
            "0 °C → °F",
            "1 GiB to MiB",
            "1 MB/s to Mb/s",
            "1 m² to square feet",
            "2 cubic meters to litres",
            "(5+3) ft to m",
            "60 km/h to m/s",
            "12 in in cm",
        ] {
            let parsed = Expression::parse(expression)
                .unwrap_or_else(|error| panic!("{expression}: {error}"));
            assert!(parsed.units_enabled());
            assert!(parsed.automatic);
        }
        for expression in [
            "1 m to kg",
            "1 MB to m",
            "1 m^2 to m^3",
            "1 m/s to m",
            "1 °C/s to K/s",
            "1 m to USD",
            "1 m to exrates",
            "1 m to hex",
            "1 m to cm; save",
            "1 m to",
            "go to documents",
        ] {
            assert!(Expression::parse(expression).is_err(), "{expression}");
        }
        assert_ne!(
            parse_unit("MB").unwrap().code,
            parse_unit("Mb").unwrap().code
        );
    }

    #[test]
    fn result_requires_a_complete_number_and_the_requested_unit() {
        let expression = Expression::parse("1 m to cm").unwrap();
        assert_eq!(expression.result("100 cm\n"), Some("100 cm".into()));
        for invalid in [
            "1 m",
            "100 kg",
            "100",
            "Infinity cm",
            "1 / 0 cm",
            "100 cm\nwarning",
            "1E+ cm",
            "1.2.3 cm",
        ] {
            assert!(expression.result(invalid).is_none(), "{invalid}");
        }
        assert_eq!(
            Expression::parse("32 F to C").unwrap().result("0 oC"),
            Some("0 °C".into())
        );
        for number in ["0", "-1.25", "1E+3000", ".5", "+1e-2"] {
            assert!(numeric_result(number), "{number}");
        }
        for number in ["", "NaN", "inf", "1e", "1+2", "1e2e3", "++1", "."] {
            assert!(!numeric_result(number), "{number}");
        }
    }

    #[tokio::test]
    async fn qalc_converts_supported_units_and_rejects_incompatible_quantities() {
        if std::process::Command::new("qalc")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        for (query, expected) in [
            ("sqrt(144)", "12"),
            ("abs(-5)", "5"),
            ("200+10%", "220"),
            ("100*25%", "25"),
            ("sin(pi/2)", "1"),
            ("log10(1000)", "3"),
            ("round(1,5)", "2"),
            ("sqrt(144) m to cm", "1200 cm"),
            ("10 cm to m", "0.1 m"),
            ("1 mile in kilometers", "1.609344 km"),
            ("1 kg to lb", "2.204622622 lb"),
            ("32 F to C", "0 °C"),
            ("0 °C -> °F", "32 °F"),
            ("-40 celsius to fahrenheit", "-40 °F"),
            ("273,15 K to C", "0 °C"),
            ("36 km/h to m/s", "10 m/s"),
            ("1 GiB to MiB", "1024 MiB"),
            ("1 GB to MB", "1000 MB"),
            ("1 MB to Mb", "8 Mbit"),
            ("1 MB/s to Mbit/s", "8 Mbit/s"),
            ("1 m² to cm^2", "10000 cm²"),
            ("2 cubic meters to liters", "2000 L"),
            ("1 hour to minutes", "60 min"),
            ("(5+3) ft to m", "2.4384 m"),
            ("1 bar to kPa", "100 kPa"),
            ("1 kWh to J", "3600000 J"),
            ("1 kcal to J", "4184 J"),
            ("1 kW to W", "1000 W"),
            ("180 degrees to rad", "3.141592654 rad"),
            ("1 gallon to liters", "3.785411784 L"),
            ("1 week to hours", "168 h"),
        ] {
            let batch = super::super::process::calculate(query, false).await;
            assert_eq!(
                batch.results.first().map(|result| result.label.as_str()),
                Some(expected),
                "{query}: {}",
                batch.status
            );
            assert!(
                matches!(&batch.results[0].actions[0].action, super::super::Action::Copy { text } if text == expected)
            );
        }
        let invalid = super::super::process::calculate("1 m to kg", true).await;
        assert!(invalid.results.is_empty());
        assert_eq!(invalid.status, "These units measure different quantities");
        assert!(
            super::super::process::calculate("go to documents", false)
                .await
                .status
                .is_empty()
        );
    }

    #[tokio::test]
    async fn every_unit_has_a_recognized_qalc_definition_and_result_symbol() {
        if std::process::Command::new("qalc")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        for unit in UNITS {
            let query = format!("1 {} to {}", unit.code, unit.code);
            let batch = super::super::process::calculate(&query, true).await;
            assert_eq!(
                batch.results.first().map(|result| result.label.clone()),
                Some(format!("1 {}", unit.label)),
                "{query}: {}",
                batch.status
            );
        }
    }
}
