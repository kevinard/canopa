//! Recognize pure calculator input; Qalculate retains evaluation semantics.
const FUNCTIONS: &[&str] = &[
    "sqrt", "abs", "sin", "cos", "tan", "asin", "acos", "atan", "ln", "log10", "exp", "floor",
    "ceil", "round",
];

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
    depth: usize,
}
impl Parser<'_> {
    fn whitespace(&mut self) {
        while self
            .input
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        self.whitespace();
        if self.input.get(self.position) == Some(&byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }
    fn sum(&mut self) -> Option<()> {
        self.product()?;
        while self.take(b'+') || self.take(b'-') {
            self.product()?;
        }
        Some(())
    }
    fn product(&mut self) -> Option<()> {
        self.power()?;
        while self.take(b'*') || self.take(b'/') {
            self.power()?;
        }
        Some(())
    }
    fn power(&mut self) -> Option<()> {
        self.unary()?;
        while self.take(b'^') {
            self.unary()?;
        }
        Some(())
    }
    fn unary(&mut self) -> Option<()> {
        while self.take(b'+') || self.take(b'-') {}
        self.primary()?;
        self.take(b'%');
        Some(())
    }
    fn grouped(&mut self) -> Option<()> {
        if self.depth >= 16 {
            return None;
        }
        self.depth += 1;
        self.sum()?;
        if !self.take(b')') {
            return None;
        }
        self.depth -= 1;
        Some(())
    }
    fn primary(&mut self) -> Option<()> {
        self.whitespace();
        if self.take(b'(') {
            return self.grouped();
        }
        if self
            .input
            .get(self.position)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            let start = self.position;
            while self
                .input
                .get(self.position)
                .is_some_and(u8::is_ascii_alphanumeric)
            {
                self.position += 1;
            }
            let name = std::str::from_utf8(&self.input[start..self.position]).ok()?;
            if matches!(name, "pi" | "e") {
                return Some(());
            }
            if !FUNCTIONS.contains(&name) || !self.take(b'(') {
                return None;
            }
            return self.grouped();
        }
        let mut digits = 0;
        let mut dots = 0;
        while let Some(byte) = self.input.get(self.position) {
            match byte {
                b'0'..=b'9' => digits += 1,
                b'.' | b',' => dots += 1,
                _ => break,
            }
            self.position += 1;
        }
        (digits > 0 && dots <= 1).then_some(())
    }
}

pub(super) fn prefix(input: &str) -> Option<(String, usize)> {
    if input.len() > 256 {
        return None;
    }
    let mut parser = Parser {
        input: input.as_bytes(),
        position: 0,
        depth: 0,
    };
    parser.sum()?;
    parser.whitespace();
    Some((
        input[..parser.position].trim().replace(',', "."),
        parser.position,
    ))
}
pub(super) fn parse(input: &str) -> Option<String> {
    let (expression, position) = prefix(input)?;
    (position == input.len()).then_some(expression)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentages_functions_and_constants_use_a_closed_grammar() {
        for input in [
            "sqrt(144)",
            "abs(-5)",
            "200+10%",
            "sqrt(100*25%)",
            "sin(pi/2)",
            "round(1,5)",
            "2*pi",
        ] {
            assert!(parse(input).is_some(), "{input}");
        }
        for input in [
            "run(1)",
            "save(1)",
            "sqrt(1; 2)",
            "sqrt(",
            "1e9",
            "1 m",
            "1+",
            "10%%",
            "2(3)",
            "pi where x=1",
            "1;quit",
            "/save",
        ] {
            assert!(parse(input).is_none(), "{input}");
        }
        assert_eq!(prefix("sqrt(144) km").unwrap(), ("sqrt(144)".into(), 10));
    }
}
