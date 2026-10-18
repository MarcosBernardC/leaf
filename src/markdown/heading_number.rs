#[derive(Default)]
pub(crate) struct HeadingNumbering {
    counters: Vec<usize>,
}

impl HeadingNumbering {
    pub(crate) fn next(&mut self, level: u8) -> String {
        let level = usize::from(level.clamp(1, 6));

        if self.counters.len() < level {
            self.counters.resize(level, 1);
        } else {
            self.counters.truncate(level);
            self.counters[level - 1] += 1;
        }

        let decimal = self
            .counters
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(".");

        if level == 1 {
            format!("{}.", to_roman(self.counters[0]))
        } else {
            format!("{}.", decimal)
        }
    }
}

fn to_roman(mut number: usize) -> String {
    const ROMAN: &[(usize, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut result = String::new();

    for &(value, symbol) in ROMAN {
        while number >= value {
            result.push_str(symbol);
            number -= value;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::HeadingNumbering;

    #[test]
    fn numbers_nested_headings_and_resets_descendants() {
        let mut numbering = HeadingNumbering::default();

        let numbers: Vec<String> = [1, 2, 3, 2, 3, 1, 2]
            .into_iter()
            .map(|level| numbering.next(level))
            .collect();

        assert_eq!(
            numbers,
            ["I.", "1.1.", "1.1.1.", "1.2.", "1.2.1.", "II.", "2.1.",]
        );
    }

    #[test]
    fn fills_missing_levels_without_zeroes() {
        let mut numbering = HeadingNumbering::default();

        assert_eq!(numbering.next(3), "1.1.1.");
        assert_eq!(numbering.next(4), "1.1.1.1.");
        assert_eq!(numbering.next(2), "1.2.");
        assert_eq!(numbering.next(3), "1.2.1.");
    }

    #[test]
    fn converts_large_h1_numbers_to_roman_numerals() {
        let mut numbering = HeadingNumbering::default();

        for _ in 0..9 {
            numbering.next(1);
        }

        assert_eq!(numbering.next(1), "X.");
        assert_eq!(numbering.next(1), "XI.");
    }
}
