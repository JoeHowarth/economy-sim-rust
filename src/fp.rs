#[derive(Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub struct Fp(pub i32);

pub const fn fp(x: i32) -> Fp {
    Fp(x * 100)
}

pub const fn dec(whole: i32, d: u8) -> Fp {
    Fp(whole * 100 + d as i32)
}

impl std::ops::Add for Fp {
    type Output = Fp;

    fn add(self, rhs: Self) -> Self::Output {
        Fp(self.0 + rhs.0)
    }
}

impl std::ops::Add for &Fp {
    type Output = Fp;

    fn add(self, rhs: Self) -> Self::Output {
        Fp(self.0 + rhs.0)
    }
}

impl std::ops::Mul for Fp {
    type Output = Fp;

    fn mul(self, rhs: Self) -> Self::Output {
        Fp((self.0 * rhs.0) / 100)
    }
}

impl std::ops::Div for Fp {
    type Output = Fp;

    fn div(self, rhs: Self) -> Self::Output {
        Fp((self.0 * 100) / rhs.0)
    }
}

impl std::ops::Sub for Fp {
    type Output = Fp;

    fn sub(self, rhs: Self) -> Self::Output {
        Fp(self.0 - rhs.0)
    }
}

impl std::ops::Neg for Fp {
    type Output = Fp;

    fn neg(self) -> Self::Output {
        Fp(-self.0)
    }
}

impl std::iter::Sum for Fp {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(fp(0), |acc, x| acc + x)
    }
}

impl Fp {
    pub fn abs(&self) -> Self {
        if self.0 < 0 { -*self } else { *self }
    }
}

impl std::fmt::Display for Fp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let whole = self.0 / 100;
        let decimal = self.0.abs() % 100;
        write!(f, "{}.{:02}", whole, decimal)
    }
}

impl std::fmt::Debug for Fp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
