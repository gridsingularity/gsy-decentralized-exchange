mod pay_as_bid;

pub trait PayAsBid {
    type Output;

    fn pay_as_bid(&mut self) -> Vec<Self::Output>;
}
