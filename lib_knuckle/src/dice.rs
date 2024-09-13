use rand::{rngs::StdRng, RngCore, SeedableRng};

pub struct Dice {
    next_dice: u8,
    rng: StdRng,
}

impl Dice {
    pub fn new(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let next_dice = (rng.next_u32() % 6) as u8 + 1;
        Dice { next_dice, rng }
    }

    pub fn roll(&mut self) -> usize {
        let num = self.next_dice;
        self.next_dice = (self.rng.next_u32() % 6) as u8 + 1;
        num as usize
    }
    pub fn peek(&self) -> usize {
        self.next_dice as usize
    }

    #[cfg(test)]
    pub(crate) fn set_next(&mut self, num: u8) {
        self.next_dice = num
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dice() {
        let mut dice = Dice::new(0);
        dice.set_next(1);
        assert_eq!(dice.peek(), 1);
        assert_eq!(dice.roll(), 1);
        dice.set_next(2);
        assert_eq!(dice.peek(), 2);
        assert_eq!(dice.roll(), 2);
    }
}
