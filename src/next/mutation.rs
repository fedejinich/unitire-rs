#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct MutationGeneration(u64);

impl MutationGeneration {
    pub fn current(self) -> u64 {
        self.0
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::MutationGeneration;

    #[test]
    fn mutation_generation_wraps() {
        let mut generation = MutationGeneration(u64::MAX);
        assert_eq!(generation.next(), 0);
    }
}
