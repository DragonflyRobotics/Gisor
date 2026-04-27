pub fn prioritize(scored: &mut Vec<(usize, usize, usize, usize)>) {
    scored.sort_by_key(sort_key);
}

fn sort_key(item: &(usize, usize, usize, usize)) -> (usize, usize, usize, usize) {
    (item.2, item.3, item.0, item.1)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_0() {
        let mut v: Vec<(usize, usize, usize, usize)> = Vec::new();
        super::prioritize(&mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn test_1() {
        let mut v = vec![(0, 0, 2, 7), (0, 1, 0, 5), (1, 0, 1, 1), (1, 1, 0, 2)];
        super::prioritize(&mut v);

        assert_eq!(v[0], (1, 1, 0, 2));
        assert_eq!(v[1], (0, 1, 0, 5));

        assert_eq!(v[2], (1, 0, 1, 1));
        assert_eq!(v[3], (0, 0, 2, 7));
    }

    #[test]
    fn test_2() {
        let mut v = vec![(2, 3, 1, 4), (0, 1, 1, 4), (1, 0, 1, 4), (0, 0, 1, 4)];
        super::prioritize(&mut v);

        assert_eq!(v, vec![(0, 0, 1, 4), (0, 1, 1, 4), (1, 0, 1, 4), (2, 3, 1, 4)]);
    }

    #[test]
    fn test_3() {
        let mut v = vec![(0, 0, 0, 1), (0, 1, 0, 2), (1, 0, 1, 0), (1, 1, 2, 9)];
        let expected = v.clone();

        super::prioritize(&mut v);

        assert_eq!(v, expected);
    }
}
