mod grid_topology;
mod health_check;
mod ids;
mod market;
mod orders;
mod profiles;
mod trades;

pub use grid_topology::*;
pub use health_check::*;
pub use ids::*;
pub use market::*;
pub use orders::*;
pub use profiles::*;
pub use trades::*;

use actix_web::HttpResponse;
use primitives::utils::opt_rfc3339_to_epoch;

/// Parses optional RFC 3339 `start_time`/`end_time` query params into epoch seconds.
pub fn parse_time_range(
    start_time: Option<&str>,
    end_time: Option<&str>,
) -> Result<(Option<u64>, Option<u64>), HttpResponse> {
    let parse = |value| {
        opt_rfc3339_to_epoch(value).map_err(|e| HttpResponse::BadRequest().body(e.to_string()))
    };
    let (start, end) = (parse(start_time)?, parse(end_time)?);
    validate_start_end_time(start, end)?;
    Ok((start, end))
}

pub fn validate_start_end_time<T: PartialOrd>(
    start_time: Option<T>,
    end_time: Option<T>,
) -> Result<(), HttpResponse> {
    let (start, end) = match (start_time, end_time) {
        (Some(start), Some(end)) => (start, end),
        _ => return Ok(()),
    };

    if end < start {
        return Err(HttpResponse::BadRequest().body("end_time must be after start_time"));
    }

    Ok(())
}
