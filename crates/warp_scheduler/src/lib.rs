// priority helper
pub fn prioritize(scored: &mut Vec<(usize, usize, usize)>) {
    // warps with more active threads have higher priority
    // coudl cause starvation of processes with more warp divergence
    scored.sort_by(|a, b| a.2.cmp(&b.2));
}

#[cfg(test)]
mod tests {
    #[test]
    fn prioritize_orders_low_divergence_first() {
        let mut v = vec![(0, 0, 2), (0, 1, 5), (1, 0, 1)];
        super::prioritize(&mut v);
        assert_eq!(v[0].2, 1);
        assert_eq!(v[1].2, 2);
        assert_eq!(v[2].2, 5);
    }
}
