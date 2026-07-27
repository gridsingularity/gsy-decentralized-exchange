pub trait PayAsBid {
    type Output;

    fn pay_as_bid(&mut self) -> Vec<Self::Output>;
}

/// Matches the accepted merit-order volume at one uniform clearing price.
pub trait PayAsClear {
    type Output;

    fn pay_as_clear(&mut self) -> Vec<Self::Output>;
}
