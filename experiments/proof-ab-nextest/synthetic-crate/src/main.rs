fn main() {}

#[cfg(test)]
mod bin_tests {
    #[test]
    fn bin_smoke() {
        assert_eq!(2 + 2, 4);
    }
}
