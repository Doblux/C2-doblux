pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)] // esto solo compilalo cuando corra cargo test, no cuando compile el binario final
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
