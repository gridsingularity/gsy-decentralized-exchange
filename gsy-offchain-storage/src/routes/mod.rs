mod grid_topology;
mod health_check;
mod market;
mod orders;
mod profiles;
mod trades;

pub use grid_topology::*;
pub use health_check::*;
pub use market::*;
pub use orders::*;
pub use profiles::*;
pub use trades::*;

use actix_web::HttpResponse;

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
