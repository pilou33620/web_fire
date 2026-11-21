//! Le hasard du jeu : un générateur SplitMix64, reproductible à partir d'une graine, sans dépendance.
//! Il remplace `random.Random` de l'ancienne version Python : mêmes usages, pas les mêmes tirages
//! (une graine donne une autre journée qu'en Python, mais toujours la même en Rust).

#[derive(Clone, Debug)]
pub struct Hasard(u64);

impl Hasard {
    pub fn new(graine: i64) -> Hasard {
        Hasard((graine as u64) ^ 0x5DEE_CE66_D1CE_4E5B)
    }

    fn suivant(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Un nombre de [0, 1).
    pub fn random(&mut self) -> f64 {
        (self.suivant() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// Loi normale (Box-Muller).
    pub fn gauss(&mut self, mu: f64, sigma: f64) -> f64 {
        let u1 = 1.0 - self.random();
        let u2 = self.random();
        mu + sigma * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    pub fn expovariate(&mut self, lambd: f64) -> f64 {
        -(1.0 - self.random()).ln() / lambd
    }

    /// Un entier de [0, n) (n > 0).
    pub fn randrange(&mut self, n: usize) -> usize {
        ((self.random() * n as f64) as usize).min(n.saturating_sub(1))
    }

    pub fn choice<'a, T>(&mut self, l: &'a [T]) -> &'a T {
        &l[self.randrange(l.len())]
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.randrange(i + 1);
            v.swap(i, j);
        }
    }

    /// k éléments distincts tirés au hasard (moins s'il n'y en a pas assez).
    pub fn sample<T: Clone>(&mut self, v: &[T], k: usize) -> Vec<T> {
        let mut c = v.to_vec();
        let k = k.min(c.len());
        for i in 0..k {
            let j = i + self.randrange(c.len() - i);
            c.swap(i, j);
        }
        c.truncate(k);
        c
    }
}

#[cfg(test)]
mod essais {
    use super::Hasard;

    #[test]
    fn bornes_et_reproductible() {
        let (mut a, mut b) = (Hasard::new(7), Hasard::new(7));
        for _ in 0..1000 {
            let x = a.random();
            assert!((0.0..1.0).contains(&x));
            assert_eq!(x, b.random());
            assert!(a.randrange(5) < 5);
            b.randrange(5);
        }
        let s = a.sample(&[1, 2, 3, 4, 5], 3);
        assert_eq!(s.len(), 3);
        assert!(s[0] != s[1] && s[1] != s[2] && s[0] != s[2]);
    }
}
